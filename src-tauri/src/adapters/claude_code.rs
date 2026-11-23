use super::{
    DetectResult, RunOutput, VersionCompatibility, detect_with, epoch_seconds, json_u64,
    limit_signal, parse_version, run, sum_u64,
};
use crate::{roles::Permissions, supervisor::RunUpdate};
use serde_json::Value;
use std::{
    collections::HashMap,
    io::{self, BufRead, BufReader, Read, Write},
    path::Path,
    process::{Command, Output, Stdio},
    thread,
};

fn logged_in(output: &Output) -> Result<bool, String> {
    let value: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|_| "无法检查 Claude Code 登录状态：auth status 未返回有效 JSON".to_owned())?;
    match value.get("loggedIn").and_then(|value| value.as_bool()) {
        Some(false) => Ok(false),
        Some(true) if output.status.success() => Ok(true),
        _ => Err("无法检查 Claude Code 登录状态：auth status 返回未知结果".to_owned()),
    }
}

fn compatible(version: &str) -> VersionCompatibility {
    let Some(version) = parse_version(version) else {
        return VersionCompatibility::Unknown;
    };
    if ((2, 1, 259)..(2, 1, 331)).contains(&version) {
        VersionCompatibility::Compatible
    } else {
        VersionCompatibility::Unverified
    }
}

pub fn detect() -> DetectResult {
    detect_with(
        "claude-code",
        "claude",
        &["auth", "status", "--json"],
        logged_in,
        compatible,
        run,
    )
}

pub(crate) struct StartOptions<'a> {
    pub cwd: &'a Path,
    pub model: &'a str,
    pub system_prompt: &'a str,
    pub prompt: &'a str,
    pub permissions: &'a Permissions,
    pub resume: Option<&'a str>,
    pub persist_session: bool,
}

fn command(options: &StartOptions<'_>) -> Result<Command, String> {
    let Permissions { allow, deny } = options.permissions;
    let mut tools = Vec::new();
    for rule in allow {
        let tool = rule.split_once('(').map_or(rule.as_str(), |(tool, _)| tool);
        if tool.is_empty()
            || tool == "default"
            || !tool
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        {
            return Err(format!("无效的 Claude Code 工具允许规则：{rule}"));
        }
        if !tools.contains(&tool) {
            tools.push(tool);
        }
    }
    if deny
        .iter()
        .any(|rule| rule.trim().is_empty() || rule.starts_with('-'))
    {
        return Err("存在无效的 Claude Code 工具禁止规则".into());
    }

    let mut command = Command::new("claude");
    command
        .current_dir(options.cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .args([
            "-p",
            "--output-format",
            "stream-json",
            "--verbose",
            "--include-partial-messages",
            "--permission-mode",
            "dontAsk",
            "--permission-prompts",
            "none",
            "--setting-sources",
            "user",
            "--strict-mcp-config",
            "--tools",
        ])
        .arg(tools.join(","))
        .arg(format!("--append-system-prompt={}", options.system_prompt));
    let model = options.model.trim();
    if !model.is_empty() && model != "auto" {
        command.arg(format!("--model={model}"));
    }
    if let Some(session) = options.resume {
        command.arg(format!("--resume={session}"));
    }
    if !allow.is_empty() {
        command.arg("--allowedTools").args(allow);
    }
    command.arg("--disallowedTools").args(deny).args([
        "mcp__*",
        "Bash(git commit *)",
        "Bash(git merge *)",
        "Bash(git push *)",
    ]);
    Ok(command)
}

fn usage_metrics(usage: &Value) -> Option<(u64, u64, Option<u64>)> {
    let input = json_u64(&usage["input_tokens"])?;
    let output = json_u64(&usage["output_tokens"])?;
    let cache = sum_u64(usage, &["cache_read_input_tokens", "cache_creation_input_tokens"]);
    Some((input, output, cache))
}

fn stream(
    input: impl BufRead,
    emit: &impl Fn(RunUpdate) -> Result<(), String>,
) -> Result<Option<Result<RunOutput, String>>, String> {
    let mut result = None;
    let mut session_id = None;
    let mut emitted_text = false;
    let mut partials = HashMap::<usize, String>::new();
    let mut message_index = 0;
    let mut last_block = None;
    let mut emit_text = |text: &str, block| {
        if !text.is_empty() {
            let text = if last_block.is_some() && last_block != Some(block) {
                format!("\n{text}")
            } else {
                text.into()
            };
            emit(RunUpdate::Text { text })?;
            last_block = Some(block);
            emitted_text = true;
        }
        Ok::<(), String>(())
    };
    for line in input.lines() {
        let line = line.map_err(|error| format!("无法读取 Claude Code 输出：{error}"))?;
        let Ok(message) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        if !message["parent_tool_use_id"].is_null() {
            continue;
        }
        if let Some(id) = message.get("session_id").and_then(Value::as_str) {
            session_id = Some(id.to_owned());
        }
        match message["type"].as_str() {
            Some("stream_event") => {
                let event = &message["event"];
                if event["type"] == "message_start" {
                    partials.clear();
                    message_index += 1;
                }
                if event["type"] == "content_block_delta"
                    && event["delta"]["type"] == "text_delta"
                    && let Some(text) = event["delta"]["text"].as_str()
                {
                    let index = event["index"].as_u64().unwrap_or_default() as usize;
                    emit_text(text, (message_index, index))?;
                    partials.entry(index).or_default().push_str(text);
                }
            }
            Some("rate_limit_event") => {
                let info = &message["rate_limit_info"];
                let rejected = matches!(info["status"].as_str(), Some("rejected"))
                    || matches!(info["overageStatus"].as_str(), Some("rejected"));
                if rejected {
                    let resets_at = json_u64(&info["resetsAt"])
                        .or_else(|| {
                            info["rateLimitType"]
                                .as_str()
                                .and_then(|kind| json_u64(&info["unifiedWindows"][kind]["resetsAt"]))
                        })
                        .or_else(|| json_u64(&info["overageResetsAt"]))
                        .map(epoch_seconds);
                    emit(RunUpdate::UsageLimited { resets_at })?;
                }
            }
            Some("assistant") => {
                if let Some(blocks) = message["message"]["content"].as_array() {
                    for (index, block) in blocks.iter().enumerate() {
                        match block["type"].as_str() {
                            Some("text") => {
                                if let Some(text) = block["text"].as_str() {
                                    let partial = partials.get(&index).map_or("", String::as_str);
                                    emit_text(
                                        text.strip_prefix(partial).unwrap_or_default(),
                                        (message_index, index),
                                    )?;
                                }
                            }
                            Some("tool_use") => {
                                if let Some(name) = block["name"].as_str() {
                                    emit(RunUpdate::Tool { name: name.into() })?;
                                }
                            }
                            _ => {}
                        }
                    }
                }
                partials.clear();
                message_index += 1;
            }
            Some("result") => {
                if let Some((input, output, cache)) = usage_metrics(&message["usage"]) {
                    emit(RunUpdate::TurnMetrics {
                        input_tokens: input,
                        output_tokens: output,
                        cache_tokens: cache,
                    })?;
                }
                result = Some(message);
            }
            _ => {}
        }
    }
    let Some(result) = result else {
        return Ok(None);
    };
    let text = result["result"].as_str().unwrap_or_default();
    if result["subtype"] == "success" && result["is_error"] == false {
        if !emitted_text && !text.is_empty() {
            emit(RunUpdate::Text { text: text.into() })?;
        }
        return Ok(Some(Ok(RunOutput {
            text: text.into(),
            session_id,
        })));
    }
    let reason = result["errors"]
        .as_array()
        .and_then(|errors| errors.iter().find_map(Value::as_str))
        .or(Some(text).filter(|text| !text.is_empty()))
        .or_else(|| result["subtype"].as_str())
        .unwrap_or("未知错误");
    if limit_signal(reason)
        || limit_signal(result["api_error"].as_str().unwrap_or_default())
        || result["api_error_code"]
            .as_str()
            .is_some_and(limit_signal)
    {
        emit(RunUpdate::UsageLimited { resets_at: None })?;
    }
    Ok(Some(Err(format!("Claude Code 运行失败：{reason}"))))
}

pub(crate) fn start(
    options: StartOptions<'_>,
    control: &crate::supervisor::RunSlot,
    emit: &impl Fn(RunUpdate) -> Result<(), String>,
) -> Result<RunOutput, String> {
    let mut command = command(&options)?;
    let mut child = crate::process_tree::spawn(&mut command).map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            "无法启动 Claude Code：未找到 claude 命令".into()
        } else {
            format!("无法启动 Claude Code：{error}")
        }
    })?;
    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    control.attach(child)?;
    thread::scope(|scope| {
        let writer = scope.spawn(move || stdin.write_all(options.prompt.as_bytes()));
        let diagnostics = scope.spawn(move || {
            let mut prefix = Vec::new();
            let _ = stderr.by_ref().take(2048).read_to_end(&mut prefix);
            let _ = io::copy(&mut stderr, &mut io::sink());
            prefix
        });
        let result = emit(RunUpdate::Started).and_then(|()| stream(BufReader::new(stdout), emit));
        if result.is_err() {
            control.terminate();
        }
        let status = control
            .wait()
            .map_err(|error| format!("无法等待 Claude Code 退出：{error}"));
        let written = writer.join().unwrap();
        let diagnostics = diagnostics.join().unwrap();
        let status = status?;
        match result? {
            Some(Err(message)) => Err(message),
            _ if !status.success() => {
                let stderr = String::from_utf8_lossy(&diagnostics);
                let reason = stderr.trim();
                if reason.is_empty() {
                    Err(format!("Claude Code 运行失败：{status}"))
                } else {
                    Err(format!("Claude Code 运行失败：{status}：{reason}"))
                }
            }
            result => {
                written.map_err(|error| format!("无法向 Claude Code 发送任务：{error}"))?;
                result.ok_or_else(|| "Claude Code 已退出但未返回运行结果".to_owned())?
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[test]
    fn detection_fixtures() {
        super::super::tests::fixtures(
            "claude-code",
            "claude",
            &["auth", "status", "--json"],
            super::logged_in,
            super::compatible,
            include_str!("../../../tests/adapters/fixtures/claude-detect.json"),
        );
    }

    #[test]
    fn incremental_text_and_tools_from_stream_json() {
        let updates = RefCell::new(Vec::new());
        let outcome = stream(
            include_bytes!("../../../tests/adapters/fixtures/claude-start.jsonl").as_slice(),
            &|update| {
                updates.borrow_mut().push(update);
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(
            outcome,
            Some(Ok(RunOutput {
                text: "The file is empty.".into(),
                session_id: Some("claude-fixture".into()),
            }))
        );
        assert_eq!(
            serde_json::to_value(updates.into_inner()).unwrap(),
            serde_json::json!([
                {"type": "text", "text": "Reading "},
                {"type": "text", "text": "the file."},
                {"type": "tool", "name": "Read"},
                {"type": "text", "text": "\nThe file is empty."}
            ])
        );
    }

    #[test]
    fn usage_limit_and_metrics_from_stream_json() {
        let updates = RefCell::new(Vec::new());
        let outcome = stream(
            include_bytes!("../../../tests/adapters/fixtures/claude-usage.jsonl").as_slice(),
            &|update| {
                updates.borrow_mut().push(update);
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(
            outcome,
            Some(Ok(RunOutput {
                text: "Done.".into(),
                session_id: Some("usage-fixture".into()),
            }))
        );
        assert_eq!(
            serde_json::to_value(updates.into_inner()).unwrap(),
            serde_json::json!([
                {"type":"usage_limited","resets_at":1700000060},
                {"type":"text","text":"Done."},
                {"type":"turn_metrics","input_tokens":12,"output_tokens":4,"cache_tokens":5}
            ])
        );

        let updates = RefCell::new(Vec::new());
        let outcome = stream(
            include_bytes!("../../../tests/adapters/fixtures/claude-usage-error.jsonl").as_slice(),
            &|update| {
                updates.borrow_mut().push(update);
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(
            outcome,
            Some(Err("Claude Code 运行失败：Usage limit reached".into()))
        );
        assert_eq!(
            serde_json::to_value(updates.into_inner()).unwrap(),
            serde_json::json!([{"type":"usage_limited","resets_at":null}])
        );
    }

    #[test]
    fn separate_blocks_keep_their_streamed_deltas_together() {
        let updates = RefCell::new(String::new());
        stream(concat!(
            "{\"type\":\"stream_event\",\"event\":{\"type\":\"message_start\"}}\n",
            "{\"type\":\"stream_event\",\"event\":{\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"First \"}}}\n",
            "{\"type\":\"stream_event\",\"event\":{\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"block\"}}}\n",
            "{\"type\":\"stream_event\",\"event\":{\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"text_delta\",\"text\":\"Second block\"}}}\n",
            "{\"type\":\"assistant\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"First block\"},{\"type\":\"text\",\"text\":\"Second block\"}]}}\n",
            "{\"type\":\"assistant\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"Third block\"},{\"type\":\"text\",\"text\":\"Fourth block\"}]}}\n",
        ).as_bytes(), &|update| {
            if let RunUpdate::Text { text } = update { updates.borrow_mut().push_str(&text); }
            Ok(())
        }).unwrap();
        assert_eq!(
            *updates.borrow(),
            "First block\nSecond block\nThird block\nFourth block"
        );
    }

    #[test]
    fn cli_failure_and_missing_result() {
        let ignore = |_| Ok(());
        assert_eq!(
            stream(
                include_bytes!("../../../tests/adapters/fixtures/claude-start-error.jsonl")
                    .as_slice(),
                &ignore
            )
            .unwrap(),
            Some(Err(
                "Claude Code 运行失败：Not logged in · Please run /login".into()
            ))
        );
        assert_eq!(stream(b"not JSON\n".as_slice(), &ignore).unwrap(), None);
    }

    #[test]
    fn reviewer_has_only_read_tools() {
        let config = crate::roles::tests::reviewer_config();
        let cmd = command(&StartOptions {
            cwd: Path::new("camp"),
            model: &config.model,
            system_prompt: &config.system_prompt,
            prompt: "review",
            permissions: &config.permissions,
            resume: None,
            persist_session: false,
        })
        .unwrap();
        let args = cmd
            .get_args()
            .map(|arg| arg.to_str().unwrap())
            .collect::<Vec<_>>();
        assert!(
            args.windows(2)
                .any(|pair| pair == ["--tools", "Read,Grep,Glob"])
        );
        let denied = &args[args
            .iter()
            .position(|arg| *arg == "--disallowedTools")
            .unwrap()
            + 1..];
        for tool in ["Write", "Edit", "Bash", "NotebookEdit"] {
            assert!(denied.contains(&tool));
        }
    }

    #[test]
    fn command_uses_role_tools_and_keeps_prompt_off_argv() {
        let cwd = Path::new("a camp");
        let permissions = Permissions {
            allow: vec!["Read".into(), "Bash(git status *)".into()],
            deny: vec!["Bash(rm -rf *)".into()],
        };
        let mut options = StartOptions {
            cwd,
            model: "sonnet",
            system_prompt: "-role personality\n\nskill",
            prompt: "--dangerously-skip-permissions",
            permissions: &permissions,
            resume: None,
            persist_session: false,
        };
        let cmd = command(&options).unwrap();
        assert_eq!(cmd.get_program(), "claude");
        assert_eq!(cmd.get_current_dir(), Some(cwd));
        let args = cmd.get_args().collect::<Vec<_>>();
        assert!(args.windows(2).any(|pair| pair == ["--tools", "Read,Bash"]));
        assert!(
            args.windows(3)
                .any(|args| args == ["--allowedTools", "Read", "Bash(git status *)"])
        );
        assert!(
            args.windows(2)
                .any(|pair| pair == ["--disallowedTools", "Bash(rm -rf *)"])
        );
        for arg in [
            "-p",
            "--verbose",
            "--include-partial-messages",
            "--model=sonnet",
            "--append-system-prompt=-role personality\n\nskill",
        ] {
            assert!(args.contains(&std::ffi::OsStr::new(arg)));
        }
        assert!(!args.contains(&std::ffi::OsStr::new(options.prompt)));
        let no_tools = Permissions {
            allow: vec![],
            deny: vec![],
        };
        options.permissions = &no_tools;
        options.model = "auto";
        let cmd = command(&options).unwrap();
        let args = cmd.get_args().collect::<Vec<_>>();
        assert!(args.windows(2).any(|pair| pair == ["--tools", ""]));
        assert!(
            !args
                .iter()
                .any(|arg| arg.to_string_lossy().starts_with("--model"))
        );
        assert!(!args.contains(&std::ffi::OsStr::new("--allowedTools")));
    }

    #[test]
    fn resume_uses_session_argument() {
        let permissions = Permissions {
            allow: vec![],
            deny: vec![],
        };
        let resumed = command(&StartOptions {
            cwd: Path::new("camp"),
            model: "auto",
            system_prompt: "",
            prompt: "rework",
            permissions: &permissions,
            resume: Some("session-1"),
            persist_session: true,
        })
        .unwrap();
        let args = resumed
            .get_args()
            .map(|arg| arg.to_str().unwrap())
            .collect::<Vec<_>>();
        assert!(args.contains(&"--resume=session-1"));
        let fresh = command(&StartOptions {
            cwd: Path::new("camp"),
            model: "auto",
            system_prompt: "",
            prompt: "rework",
            permissions: &permissions,
            resume: None,
            persist_session: true,
        })
        .unwrap();
        assert!(
            !fresh
                .get_args()
                .any(|arg| arg.to_string_lossy().starts_with("--resume"))
        );
    }
}

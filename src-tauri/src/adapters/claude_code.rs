use super::{DetectResult, detect_with, run};
use crate::{roles::Permissions, supervisor::RunUpdate};
use serde_json::Value;
use std::{
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

pub fn detect() -> DetectResult {
    detect_with(
        "claude-code",
        "claude",
        &["auth", "status", "--json"],
        logged_in,
        run,
    )
}

pub(crate) struct StartOptions<'a> {
    pub cwd: &'a Path,
    pub model: &'a str,
    pub system_prompt: &'a str,
    pub prompt: &'a str,
    pub permissions: &'a Permissions,
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

fn stream(
    input: impl BufRead,
    emit: &impl Fn(RunUpdate) -> Result<(), String>,
) -> Result<Option<Result<(), String>>, String> {
    let mut result = None;
    let mut emitted_text = false;
    let mut partial = String::new();
    let mut emit_text = |text: &str| {
        if !text.is_empty() {
            emit(RunUpdate::Text { text: text.into() })?;
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
        match message["type"].as_str() {
            Some("stream_event") => {
                let event = &message["event"];
                if event["type"] == "message_start" {
                    partial.clear();
                }
                if event["type"] == "content_block_delta"
                    && event["delta"]["type"] == "text_delta"
                    && let Some(text) = event["delta"]["text"].as_str()
                {
                    emit_text(text)?;
                    partial.push_str(text);
                }
            }
            Some("assistant") => {
                if let Some(blocks) = message["message"]["content"].as_array() {
                    for block in blocks {
                        match block["type"].as_str() {
                            Some("text") => {
                                if let Some(text) = block["text"].as_str() {
                                    emit_text(text.strip_prefix(&partial).unwrap_or_default())?;
                                }
                                partial.clear();
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
            }
            Some("result") => {
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
        return Ok(Some(Ok(())));
    }
    let reason = result["errors"]
        .as_array()
        .and_then(|errors| errors.iter().find_map(Value::as_str))
        .or(Some(text).filter(|text| !text.is_empty()))
        .or_else(|| result["subtype"].as_str())
        .unwrap_or("未知错误");
    Ok(Some(Err(format!("Claude Code 运行失败：{reason}"))))
}

pub(crate) fn start(
    options: StartOptions<'_>,
    control: &crate::supervisor::RunSlot,
    emit: &impl Fn(RunUpdate) -> Result<(), String>,
) -> Result<(), String> {
    let mut child = command(&options)?.spawn().map_err(|error| {
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
        assert_eq!(outcome, Some(Ok(())));
        assert_eq!(
            serde_json::to_value(updates.into_inner()).unwrap(),
            serde_json::json!([
                {"type": "text", "text": "Reading "},
                {"type": "text", "text": "the file."},
                {"type": "tool", "name": "Read"},
                {"type": "text", "text": "The file is empty."}
            ])
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
}

use super::claude_code::StartOptions;
use super::{
    DetectResult, RunOutput, VersionCompatibility, detect_with, epoch_seconds, json_u64,
    limit_signal, parse_version, run, sum_u64,
};
use crate::supervisor::RunUpdate;
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    io::{self, BufRead, BufReader, Read, Write},
    process::{Command, Output, Stdio},
    thread,
};

fn logged_in(output: &Output) -> Result<bool, String> {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let lines = || stdout.lines().chain(stderr.lines()).map(str::trim);
    if output.status.success() && lines().any(|line| line.starts_with("Logged in using ")) {
        Ok(true)
    } else if !output.status.success() && lines().any(|line| line == "Not logged in") {
        Ok(false)
    } else {
        Err("无法检查 Codex 登录状态：login status 返回未知结果".to_owned())
    }
}

fn compatible(version: &str) -> VersionCompatibility {
    let Some(version) = parse_version(version) else {
        return VersionCompatibility::Unknown;
    };
    if ((0, 171, 0)..(0, 172, 0)).contains(&version) {
        VersionCompatibility::Compatible
    } else {
        VersionCompatibility::Unverified
    }
}

pub fn detect() -> DetectResult {
    detect_with(
        "codex",
        "codex",
        &["login", "status"],
        logged_in,
        compatible,
        run,
    )
}

fn command(options: &StartOptions<'_>) -> Command {
    let permissions = options.permissions;
    let writable = ["Write", "Edit"].iter().all(|tool| {
        permissions.allow.iter().any(|rule| rule == tool)
            && !permissions
                .deny
                .iter()
                .any(|rule| rule.split('(').next() == Some(tool))
    });
    let sandbox = if writable {
        "workspace-write"
    } else {
        "read-only"
    };
    let mut command = Command::new("codex");
    command
        .current_dir(options.cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .args(["--ask-for-approval", "never", "exec"]);
    if let Some(session) = options.resume {
        command
            .arg("resume")
            .arg(session)
            .args([
                "--json",
                "--ignore-user-config",
                "--ignore-rules",
                "--config",
            ])
            .arg(format!("sandbox_mode={sandbox}"));
    } else {
        command.args([
            "--json",
            "--sandbox",
            sandbox,
            "--ignore-user-config",
            "--ignore-rules",
        ]);
        if !options.persist_session {
            command.arg("--ephemeral");
        }
    }
    command.arg("--skip-git-repo-check");
    command
        .args([
            "--config",
            "sandbox_workspace_write.network_access=false",
            "--config",
            "sandbox_workspace_write.exclude_tmpdir_env_var=true",
            "--config",
            "sandbox_workspace_write.exclude_slash_tmp=true",
        ])
        .arg("--config")
        .arg(format!(
            "developer_instructions={}",
            serde_json::to_string(options.system_prompt).unwrap()
        ));
    let model = options.model.trim();
    if !model.is_empty() && model != "auto" {
        command.arg(format!("--model={model}"));
    }
    command.arg("-");
    command
}

fn usage_metrics(usage: &Value) -> Option<(u64, u64, Option<u64>)> {
    let input = json_u64(&usage["input_tokens"])?;
    let output = json_u64(&usage["output_tokens"])?;
    let cache = sum_u64(usage, &["cached_input_tokens", "cache_write_input_tokens"]);
    Some((input, output, cache))
}

fn reset_at(message: &Value) -> Option<u64> {
    ["resets_at", "reset_at"]
        .iter()
        .find_map(|key| {
            json_u64(&message[*key])
                .or_else(|| json_u64(&message["error"][*key]))
                .map(epoch_seconds)
        })
}

fn stream(
    input: impl BufRead,
    emit: &impl Fn(RunUpdate) -> Result<(), String>,
) -> Result<Option<Result<RunOutput, String>>, String> {
    let mut result = None;
    let mut session_id = None;
    let mut final_text = String::new();
    let mut texts = HashMap::<String, String>::new();
    let mut emitted_text = false;
    let mut tools = HashSet::new();
    for line in input.lines() {
        let line = line.map_err(|error| format!("无法读取 Codex 输出：{error}"))?;
        let Ok(message) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        match message["type"].as_str() {
            Some("thread.started") => {
                if let Some(id) = message["thread_id"].as_str() {
                    session_id = Some(id.to_owned());
                }
            }
            Some("item.started" | "item.updated" | "item.completed") => {
                let item = &message["item"];
                let Some(id) = item["id"].as_str() else {
                    continue;
                };
                match item["type"].as_str() {
                    Some("agent_message") => {
                        if let Some(text) = item["text"].as_str() {
                            if message["type"] == "item.completed" {
                                final_text = text.into();
                            }
                            let previous = texts.entry(id.into()).or_default();
                            let delta = text.strip_prefix(previous.as_str()).unwrap_or(text);
                            if !delta.is_empty() {
                                let text = if previous.is_empty() && emitted_text {
                                    format!("\n{delta}")
                                } else {
                                    delta.into()
                                };
                                emit(RunUpdate::Text { text })?;
                                emitted_text = true;
                            }
                            *previous = text.into();
                        }
                    }
                    Some(
                        kind @ ("command_execution" | "file_change" | "mcp_tool_call"
                        | "web_search" | "collab_tool_call"),
                    ) if tools.insert(id.to_owned()) => {
                        let name = if kind == "mcp_tool_call" {
                            format!(
                                "{}::{}",
                                item["server"].as_str().unwrap_or("mcp"),
                                item["tool"].as_str().unwrap_or("tool")
                            )
                        } else {
                            kind.into()
                        };
                        emit(RunUpdate::Tool { name })?;
                    }
                    _ => {}
                }
            }
            Some("turn.completed") => {
                if result.is_none() {
                    if let Some((input, output, cache)) = usage_metrics(&message["usage"]) {
                        emit(RunUpdate::TurnMetrics {
                            input_tokens: input,
                            output_tokens: output,
                            cache_tokens: cache,
                        })?;
                    }
                    result = Some(Ok(RunOutput {
                        text: std::mem::take(&mut final_text),
                        session_id: session_id.take(),
                    }));
                }
            }
            Some("turn.failed" | "error") => {
                let reason = message["error"]["message"]
                    .as_str()
                    .or_else(|| message["message"].as_str())
                    .unwrap_or("未知错误");
                if limit_signal(reason) {
                    emit(RunUpdate::UsageLimited {
                        resets_at: reset_at(&message),
                    })?;
                }
                result = Some(Err(format!("Codex 运行失败：{reason}")));
            }
            _ => {}
        }
    }
    Ok(result)
}

pub(crate) fn start(
    options: StartOptions<'_>,
    control: &crate::supervisor::RunSlot,
    emit: &impl Fn(RunUpdate) -> Result<(), String>,
) -> Result<RunOutput, String> {
    let mut command = command(&options);
    let mut child = crate::process_tree::spawn(&mut command).map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            "无法启动 Codex：未找到 codex 命令".into()
        } else {
            format!("无法启动 Codex：{error}")
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
            .map_err(|error| format!("无法等待 Codex 退出：{error}"));
        let written = writer.join().unwrap();
        let diagnostics = diagnostics.join().unwrap();
        let status = status?;
        match result? {
            Some(Err(message)) => Err(message),
            _ if !status.success() => {
                let stderr = String::from_utf8_lossy(&diagnostics);
                Err(format!("Codex 运行失败：{status}：{}", stderr.trim()))
            }
            result => {
                written.map_err(|error| format!("无法向 Codex 发送任务：{error}"))?;
                result.ok_or_else(|| "Codex 已退出但未返回运行结果".to_owned())?
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::roles::Permissions;
    use std::{cell::RefCell, path::Path};

    #[test]
    fn jsonl_fixtures() {
        let updates = RefCell::new(Vec::new());
        assert_eq!(
            stream(
                include_bytes!("../../../tests/adapters/fixtures/codex-start.jsonl").as_slice(),
                &|update| {
                    updates.borrow_mut().push(update);
                    Ok(())
                }
            )
            .unwrap(),
            Some(Ok(RunOutput {
                text: "Done.".into(),
                session_id: Some("thread-fixture".into()),
            }))
        );
        assert_eq!(
            serde_json::to_value(updates.into_inner()).unwrap(),
            serde_json::json!([
                {"type":"text", "text":"Reading "},
                {"type":"text", "text":"the file."},
                {"type":"tool", "name":"command_execution"},
                {"type":"tool", "name":"file_change"},
                {"type":"tool", "name":"docs::search"},
                {"type":"tool", "name":"web_search"},
                {"type":"text", "text":"\nDone."},
                {"type":"turn_metrics", "input_tokens":10, "output_tokens":5, "cache_tokens":0}
            ])
        );
        assert_eq!(
            stream(
                include_bytes!("../../../tests/adapters/fixtures/codex-start-error.jsonl")
                    .as_slice(),
                &|_| Ok(())
            )
            .unwrap(),
            Some(Err("Codex 运行失败：Not logged in".into()))
        );
    }

    #[test]
    fn usage_metrics_and_limit_from_fixtures() {
        let updates = RefCell::new(Vec::new());
        let outcome = stream(
            include_bytes!("../../../tests/adapters/fixtures/codex-usage.jsonl").as_slice(),
            &|update| {
                updates.borrow_mut().push(update);
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(outcome, Some(Ok(RunOutput::from(""))));
        assert_eq!(
            serde_json::to_value(updates.into_inner()).unwrap(),
            serde_json::json!([
                {"type":"turn_metrics","input_tokens":10,"output_tokens":5,"cache_tokens":5}
            ])
        );

        let updates = RefCell::new(Vec::new());
        let outcome = stream(
            include_bytes!("../../../tests/adapters/fixtures/codex-usage-limit.jsonl").as_slice(),
            &|update| {
                updates.borrow_mut().push(update);
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(
            outcome,
            Some(Err("Codex 运行失败：Usage limit reached".into()))
        );
        assert_eq!(
            serde_json::to_value(updates.into_inner()).unwrap(),
            serde_json::json!([{"type":"usage_limited","resets_at":1700000060}])
        );
    }

    #[test]
    fn empty_invalid_error_and_event_failure() {
        for input in ["", "not JSON\n", "{\"type\":\"thread.started\"}\n"] {
            assert_eq!(stream(input.as_bytes(), &|_| Ok(())).unwrap(), None);
        }
        assert_eq!(
            stream(
                b"{\"type\":\"error\",\"message\":\"fatal\"}\n{\"type\":\"turn.completed\"}\n"
                    .as_slice(),
                &|_| Ok(())
            )
            .unwrap(),
            Some(Err("Codex 运行失败：fatal".into()))
        );
        assert_eq!(
            stream(
                include_bytes!("../../../tests/adapters/fixtures/codex-start.jsonl").as_slice(),
                &|_| Err("event failed".into())
            )
            .unwrap_err(),
            "event failed"
        );
        let input = BufReader::new(&b"\xff\n"[..]);
        assert!(
            stream(input, &|_| Ok(()))
                .unwrap_err()
                .contains("无法读取 Codex 输出")
        );
    }

    #[test]
    fn resume_pins_sandbox_mode_without_sandbox_or_ephemeral() {
        let permissions = Permissions {
            allow: vec!["Write".into(), "Edit".into()],
            deny: vec![],
        };
        let options = StartOptions {
            cwd: Path::new("camp"),
            model: "auto",
            system_prompt: "",
            prompt: "rework",
            permissions: &permissions,
            resume: Some("thread-1"),
            persist_session: true,
        };
        let cmd = command(&options);
        let args = cmd
            .get_args()
            .map(|arg| arg.to_str().unwrap())
            .collect::<Vec<_>>();
        assert!(
            args.windows(4)
                .any(|window| window == ["exec", "resume", "thread-1", "--json"])
        );
        assert!(!args.contains(&"--sandbox"));
        assert!(!args.contains(&"--ephemeral"));
        assert!(
            args.windows(2)
                .any(|pair| pair == ["--config", "sandbox_mode=workspace-write"])
        );

        let fresh = command(&StartOptions {
            cwd: Path::new("camp"),
            model: "auto",
            system_prompt: "",
            prompt: "rework",
            permissions: &permissions,
            resume: None,
            persist_session: true,
        });
        let args = fresh
            .get_args()
            .map(|arg| arg.to_str().unwrap())
            .collect::<Vec<_>>();
        assert!(
            args.windows(2)
                .any(|pair| pair == ["--sandbox", "workspace-write"])
        );
        assert!(!args.contains(&"--ephemeral"));

        let ephemeral = command(&StartOptions {
            cwd: Path::new("camp"),
            model: "auto",
            system_prompt: "",
            prompt: "rework",
            permissions: &permissions,
            resume: None,
            persist_session: false,
        });
        assert!(ephemeral.get_args().any(|arg| arg == "--ephemeral"));
    }

    #[test]
    fn reviewer_uses_read_only_sandbox() {
        let config = crate::roles::tests::reviewer_config();
        let cmd = command(&StartOptions {
            cwd: Path::new("camp"),
            model: &config.model,
            system_prompt: &config.system_prompt,
            prompt: "review",
            permissions: &config.permissions,
            resume: None,
            persist_session: false,
        });
        let args = cmd
            .get_args()
            .map(|arg| arg.to_str().unwrap())
            .collect::<Vec<_>>();
        assert!(
            args.windows(2)
                .any(|pair| pair == ["--sandbox", "read-only"])
        );
    }

    #[test]
    fn command_keeps_task_off_argv_and_uses_conservative_sandbox() {
        let cwd = Path::new("a camp");
        for (allow, deny, sandbox) in [
            (vec![], vec![], "read-only"),
            (vec!["Read"], vec!["Write", "Edit", "Bash"], "read-only"),
            (vec!["Write"], vec![], "read-only"),
            (vec!["Write", "Edit", "Bash"], vec![], "workspace-write"),
            (vec!["Write", "Edit"], vec!["Edit(file)"], "read-only"),
        ] {
            let permissions = Permissions {
                allow: allow.into_iter().map(str::to_owned).collect(),
                deny: deny.into_iter().map(str::to_owned).collect(),
            };
            let options = StartOptions {
                cwd,
                model: "a model",
                system_prompt: "Persona \"quoted\"\nSkills",
                prompt: "--dangerously-bypass-approvals-and-sandbox",
                permissions: &permissions,
                resume: None,
                persist_session: false,
            };
            let cmd = command(&options);
            assert_eq!(cmd.get_program(), "codex");
            assert_eq!(cmd.get_current_dir(), Some(cwd));
            let args = cmd
                .get_args()
                .map(|arg| arg.to_str().unwrap())
                .collect::<Vec<_>>();
            assert!(args.windows(2).any(|pair| pair == ["--sandbox", sandbox]));
            assert!(
                args.windows(2)
                    .any(|pair| pair == ["--ask-for-approval", "never"])
            );
            assert!(args.contains(&"--model=a model"));
            assert!(
                !args
                    .iter()
                    .any(|arg| arg.starts_with("features.shell_tool="))
            );
            for restriction in [
                "sandbox_workspace_write.network_access=false",
                "sandbox_workspace_write.exclude_tmpdir_env_var=true",
                "sandbox_workspace_write.exclude_slash_tmp=true",
            ] {
                assert!(
                    args.windows(2)
                        .any(|pair| pair == ["--config", restriction])
                );
            }
            assert!(args.contains(&"--ignore-user-config"));
            assert!(args.contains(&"--json"));
            assert_eq!(args.last(), Some(&"-"));
            assert!(!args.contains(&options.prompt));
            let developer = args
                .iter()
                .find_map(|arg| arg.strip_prefix("developer_instructions="))
                .unwrap();
            assert_eq!(
                serde_json::from_str::<String>(developer).unwrap(),
                options.system_prompt
            );
            let cmd = command(&StartOptions {
                model: "auto",
                ..options
            });
            assert!(
                !cmd.get_args()
                    .any(|arg| arg.to_string_lossy().starts_with("--model"))
            );
        }
    }

    #[test]
    fn detection_fixtures() {
        super::super::tests::fixtures(
            "codex",
            "codex",
            &["login", "status"],
            super::logged_in,
            super::compatible,
            include_str!("../../../tests/adapters/fixtures/codex-detect.json"),
        );
    }
}

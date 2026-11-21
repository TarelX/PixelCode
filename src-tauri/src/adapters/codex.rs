use super::claude_code::StartOptions;
use super::{DetectResult, detect_with, run};
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

pub fn detect() -> DetectResult {
    detect_with("codex", "codex", &["login", "status"], logged_in, run)
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
    let mut command = Command::new("codex");
    command
        .current_dir(options.cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .args([
            "--ask-for-approval",
            "never",
            "exec",
            "--json",
            "--sandbox",
            if writable {
                "workspace-write"
            } else {
                "read-only"
            },
            "--ignore-user-config",
            "--ignore-rules",
            "--ephemeral",
            "--skip-git-repo-check",
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

fn stream(
    input: impl BufRead,
    emit: &impl Fn(RunUpdate) -> Result<(), String>,
) -> Result<Option<Result<(), String>>, String> {
    let mut result = None;
    let mut texts = HashMap::<String, String>::new();
    let mut emitted_text = false;
    let mut tools = HashSet::new();
    for line in input.lines() {
        let line = line.map_err(|error| format!("无法读取 Codex 输出：{error}"))?;
        let Ok(message) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        match message["type"].as_str() {
            Some("item.started" | "item.updated" | "item.completed") => {
                let item = &message["item"];
                let Some(id) = item["id"].as_str() else {
                    continue;
                };
                match item["type"].as_str() {
                    Some("agent_message") => {
                        if let Some(text) = item["text"].as_str() {
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
            Some("turn.completed") if result.is_none() => result = Some(Ok(())),
            Some("turn.failed" | "error") => {
                let reason = message["error"]["message"]
                    .as_str()
                    .or_else(|| message["message"].as_str())
                    .unwrap_or("未知错误");
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
) -> Result<(), String> {
    let mut child = command(&options).spawn().map_err(|error| {
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
            Some(Ok(()))
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
                {"type":"text", "text":"\nDone."}
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
            include_str!("../../../tests/adapters/fixtures/codex-detect.json"),
        );
    }
}

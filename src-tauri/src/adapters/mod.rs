pub mod claude_code;
pub mod codex;

use serde::Serialize;
use std::{
    io,
    process::{Command, Output, Stdio},
    thread,
    time::{Duration, Instant},
};

#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DetectStatus {
    NotFound,
    Found,
    FoundNeedsLogin,
}

#[derive(Debug, Serialize)]
pub struct DetectResult {
    pub id: &'static str,
    pub status: DetectStatus,
    pub version: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, PartialEq)]
pub(crate) struct RunOutput {
    pub text: String,
    pub session_id: Option<String>,
}

impl From<String> for RunOutput {
    fn from(text: String) -> Self {
        Self {
            text,
            session_id: None,
        }
    }
}

impl From<&str> for RunOutput {
    fn from(text: &str) -> Self {
        text.to_owned().into()
    }
}

pub(super) fn run(binary: &str, args: &[&str]) -> io::Result<Output> {
    let mut command = Command::new(binary);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = crate::process_tree::spawn(&mut command)?;
    let start = Instant::now();
    loop {
        if child.try_wait()?.is_some() {
            return child.wait_with_output();
        }
        if start.elapsed() >= Duration::from_secs(5) {
            crate::process_tree::terminate(&[child.id()]);
            child.wait()?;
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "CLI detection timed out",
            ));
        }
        thread::sleep(Duration::from_millis(20));
    }
}

pub(super) fn detect_with(
    id: &'static str,
    binary: &str,
    auth_args: &[&str],
    parse: fn(&Output) -> Result<bool, String>,
    mut run: impl FnMut(&str, &[&str]) -> io::Result<Output>,
) -> DetectResult {
    let mut result = DetectResult {
        id,
        status: DetectStatus::FoundNeedsLogin,
        version: None,
        error: None,
    };
    let version = match run(binary, &["--version"]) {
        Ok(output)
            if output.status.success()
                && !String::from_utf8_lossy(&output.stdout).trim().is_empty() =>
        {
            output
        }
        Ok(_) => {
            result.error = Some(format!(
                "无法检测 {binary} 版本：--version 返回失败或空输出"
            ));
            return result;
        }
        Err(error) => {
            if error.kind() == io::ErrorKind::NotFound {
                result.status = DetectStatus::NotFound;
            }
            result.error = Some(format!("无法启动 {binary}：{error}"));
            return result;
        }
    };
    result.version = Some(String::from_utf8_lossy(&version.stdout).trim().to_owned());
    match run(binary, auth_args)
        .map_err(|error| format!("无法检查 {binary} 登录状态：{error}"))
        .and_then(|output| parse(&output))
    {
        Ok(true) => result.status = DetectStatus::Found,
        Ok(false) => {}
        Err(error) => result.error = Some(error),
    }
    result
}

#[tauri::command]
pub async fn detect_clis() -> Result<Vec<DetectResult>, String> {
    tauri::async_runtime::spawn_blocking(|| vec![claude_code::detect(), codex::detect()])
        .await
        .map_err(|error| format!("CLI 探测失败：{error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runner_reports_missing_binary() {
        assert_eq!(
            run(
                "pixelcode-nonexistent-cli-for-detection-test",
                &["--version"]
            )
            .unwrap_err()
            .kind(),
            io::ErrorKind::NotFound
        );
    }

    fn output(success: bool, stdout: &str, stderr: &str) -> Output {
        #[cfg(unix)]
        use std::os::unix::process::ExitStatusExt;
        #[cfg(windows)]
        use std::os::windows::process::ExitStatusExt;
        Output {
            status: std::process::ExitStatus::from_raw(if success { 0 } else { 256 }),
            stdout: stdout.as_bytes().to_vec(),
            stderr: stderr.as_bytes().to_vec(),
        }
    }

    pub(super) fn fixtures(
        id: &'static str,
        binary: &str,
        args: &[&str],
        parse: fn(&Output) -> Result<bool, String>,
        fixtures: &str,
    ) {
        let cases: serde_json::Value = serde_json::from_str(fixtures).unwrap();
        for case in cases.as_array().unwrap() {
            let mut calls = 0;
            let result = detect_with(id, binary, args, parse, |command, actual_args| {
                assert_eq!(command, binary);
                calls += 1;
                if calls == 1 {
                    assert_eq!(actual_args, ["--version"]);
                    Ok(output(true, "1.2.3\n", ""))
                } else {
                    assert_eq!(actual_args, args);
                    Ok(output(
                        case["success"].as_bool().unwrap(),
                        case["stdout"].as_str().unwrap(),
                        case["stderr"].as_str().unwrap(),
                    ))
                }
            });
            assert_eq!(calls, 2);
            assert_eq!(result.version.as_deref(), Some("1.2.3"));
            assert_eq!(
                serde_json::to_value(&result.status).unwrap(),
                case["status"]
            );
            assert_eq!(result.error.is_some(), case["error"].as_bool().unwrap());
        }
    }

    #[test]
    fn empty_or_failed_version_and_failed_auth() {
        for version in [output(true, "", ""), output(false, "", "failed")] {
            let mut version = Some(version);
            let result = detect_with(
                "codex",
                "codex",
                &[],
                |_| Ok(true),
                |_, _| Ok(version.take().unwrap()),
            );
            assert!(result.error.is_some());
            assert_eq!(result.status, DetectStatus::FoundNeedsLogin);
        }
        let result = detect_with(
            "codex",
            "codex",
            &["login", "status"],
            |_| Ok(true),
            |_, args| {
                if args == ["--version"] {
                    Ok(output(true, "1.2.3", ""))
                } else {
                    Err(io::ErrorKind::TimedOut.into())
                }
            },
        );
        assert!(result.error.is_some());
        assert_eq!(result.status, DetectStatus::FoundNeedsLogin);
    }

    #[test]
    fn missing_and_unlaunchable_binaries() {
        for (kind, expected) in [
            (io::ErrorKind::NotFound, DetectStatus::NotFound),
            (
                io::ErrorKind::PermissionDenied,
                DetectStatus::FoundNeedsLogin,
            ),
            (io::ErrorKind::TimedOut, DetectStatus::FoundNeedsLogin),
        ] {
            let result = detect_with("codex", "codex", &[], |_| Ok(true), |_, _| Err(kind.into()));
            assert_eq!(result.status, expected);
            assert!(result.error.is_some());
            assert!(result.version.is_none());
        }
    }

    struct EnvGuard {
        key: &'static str,
        old: Option<std::ffi::OsString>,
    }
    impl EnvGuard {
        fn set(key: &'static str, value: std::ffi::OsString) -> Self {
            let old = std::env::var_os(key);
            unsafe { std::env::set_var(key, value) };
            Self { key, old }
        }
    }
    impl Drop for EnvGuard {
        fn drop(&mut self) {
            unsafe {
                match &self.old {
                    Some(value) => std::env::set_var(self.key, value),
                    None => std::env::remove_var(self.key),
                }
            }
        }
    }

    struct FakeCli {
        _path: EnvGuard,
        _pid_file: EnvGuard,
        _args_file: EnvGuard,
        root: std::path::PathBuf,
    }
    impl FakeCli {
        fn new() -> Self {
            let root =
                std::env::temp_dir().join(format!("pixelcode-fake-cli-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&root);
            std::fs::create_dir_all(&root).unwrap();
            let source = root.join("fake_cli.rs");
            std::fs::write(&source, include_str!("../../../tests/adapters/fake_cli.rs")).unwrap();
            let binary = root.join(if cfg!(windows) {
                "fake-cli.exe"
            } else {
                "fake-cli"
            });
            let status = Command::new("rustc")
                .arg("--edition=2021")
                .arg("-O")
                .arg(&source)
                .arg("-o")
                .arg(&binary)
                .status()
                .unwrap();
            assert!(status.success());
            for name in ["claude", "codex"] {
                let target = root.join(if cfg!(windows) {
                    format!("{name}.exe")
                } else {
                    name.into()
                });
                std::fs::copy(&binary, &target).unwrap();
            }
            let old_path = std::env::var_os("PATH").unwrap_or_default();
            let mut paths = vec![root.clone().into_os_string()];
            paths.extend(std::env::split_paths(&old_path).map(|path| path.into_os_string()));
            let path = std::env::join_paths(paths).unwrap();
            let path_guard = EnvGuard::set("PATH", path);
            let pid_guard = EnvGuard::set(
                "PIXELCODE_FAKE_PID_FILE",
                root.join("grandchild.pid").into_os_string(),
            );
            let args_guard = EnvGuard::set(
                "PIXELCODE_FAKE_ARGS_FILE",
                root.join("args.log").into_os_string(),
            );
            Self {
                _path: path_guard,
                _pid_file: pid_guard,
                _args_file: args_guard,
                root,
            }
        }
    }
    impl Drop for FakeCli {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    fn fake_options<'a>(
        cwd: &'a std::path::Path,
        prompt: &'a str,
        permissions: &'a crate::roles::Permissions,
    ) -> claude_code::StartOptions<'a> {
        claude_code::StartOptions {
            cwd,
            model: "auto",
            system_prompt: "",
            prompt,
            permissions,
            resume: None,
            persist_session: false,
        }
    }

    fn wait_for_file(path: &std::path::Path) -> String {
        for _ in 0..100 {
            if let Ok(value) = std::fs::read_to_string(path)
                && !value.trim().is_empty()
            {
                return value;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        panic!("fake CLI did not write {}", path.display());
    }

    fn wait_for_exit(pid: u32) {
        for _ in 0..60 {
            if !crate::process_tree::process_exists(pid) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        assert!(!crate::process_tree::process_exists(pid));
    }

    #[test]
    fn fake_cli_real_spawn_streams_and_cleans_process_tree() {
        let fake = FakeCli::new();
        let cwd = fake.root.join("cwd");
        std::fs::create_dir_all(&cwd).unwrap();
        let permissions = crate::roles::Permissions {
            allow: vec![],
            deny: vec![],
        };
        let slot = crate::supervisor::RunSlot::default();
        let updates = std::cell::RefCell::new(Vec::new());
        let result = claude_code::start(
            fake_options(&cwd, "say hello", &permissions),
            &slot,
            &|update| {
                updates.borrow_mut().push(update);
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(result.text, "Hello world");
        assert_eq!(
            serde_json::to_value(updates.into_inner()).unwrap(),
            serde_json::json!([
                {"type":"started"},
                {"type":"text","text":"Hello "},
                {"type":"text","text":"world"}
            ])
        );
        assert_eq!(result.session_id.as_deref(), Some("fake-claude-session"));

        let updates = std::cell::RefCell::new(Vec::new());
        let result = codex::start(
            fake_options(&cwd, "say hello", &permissions),
            &slot,
            &|update| {
                updates.borrow_mut().push(update);
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(result.text, "Hello world");
        assert_eq!(
            serde_json::to_value(updates.into_inner()).unwrap(),
            serde_json::json!([
                {"type":"started"},
                {"type":"text","text":"Hello "},
                {"type":"text","text":"world"}
            ])
        );
        assert_eq!(result.session_id.as_deref(), Some("fake-codex-thread"));

        let mut options = fake_options(&cwd, "continue", &permissions);
        options.resume = Some("claude-session-123");
        options.persist_session = true;
        let result = claude_code::start(options, &slot, &|_| Ok(())).unwrap();
        assert_eq!(result.text, "Hello world");

        let mut options = fake_options(&cwd, "continue", &permissions);
        options.resume = Some("thread-123");
        options.persist_session = true;
        let result = codex::start(options, &slot, &|_| Ok(())).unwrap();
        assert_eq!(result.text, "Hello world");

        let args_log = std::fs::read_to_string(fake.root.join("args.log")).unwrap();
        let claude_resume = args_log
            .lines()
            .find(|line| line.contains("--resume=claude-session-123"))
            .unwrap();
        assert!(claude_resume.contains("--output-format stream-json"));
        let codex_resume = args_log
            .lines()
            .find(|line| line.contains("exec resume thread-123"))
            .unwrap();
        assert!(codex_resume.contains("--json"));
        assert!(!codex_resume.contains("--sandbox"));
        assert!(!codex_resume.contains("--ephemeral"));

        let slot = crate::supervisor::RunSlot::default();
        let error =
            claude_code::start(
                fake_options(&cwd, "abort", &permissions),
                &slot,
                &|_| Ok(()),
            )
            .unwrap_err();
        assert!(error.contains("fake cli abort"), "{error}");
        let slot = crate::supervisor::RunSlot::default();
        let error = codex::start(
            fake_options(&cwd, "abort", &permissions),
            &slot,
            &|_| Ok(()),
        )
        .unwrap_err();
        assert!(error.contains("fake cli abort"), "{error}");

        let slot = crate::supervisor::RunSlot::default();
        let _active = slot.acquire().unwrap();
        slot.stop().unwrap();
        let error = claude_code::start(
            fake_options(&cwd, "say hello", &permissions),
            &slot,
            &|_| Ok(()),
        )
        .unwrap_err();
        assert!(error.contains("Claude Code"), "{error}");
        assert!(slot.child.lock().unwrap().is_empty());
        drop(_active);

        let pid_file = fake.root.join("grandchild.pid");
        let slot = crate::supervisor::RunSlot::default();
        let options = fake_options(&cwd, "tree", &permissions);
        let ignore = |_| Ok(());
        let (result, grandchild) = std::thread::scope(|scope| {
            let running = scope.spawn(|| claude_code::start(options, &slot, &ignore));
            let pid = wait_for_file(&pid_file);
            slot.stop().unwrap();
            (running.join().unwrap(), pid)
        });
        assert!(result.is_err());
        wait_for_exit(grandchild.trim().parse().unwrap());
    }
}

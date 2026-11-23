pub mod claude_code;
pub mod codex;

use serde::Serialize;
use serde_json::Value;
use std::{
    io,
    process::{Command, Output, Stdio},
    thread,
    time::{Duration, Instant},
};

#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VersionCompatibility {
    Compatible,
    Unverified,
    Unknown,
}

#[derive(Debug, Serialize)]
pub struct DetectResult {
    pub id: &'static str,
    pub installed: bool,
    pub signed_in: bool,
    pub version: Option<String>,
    pub version_compatibility: VersionCompatibility,
    pub error: Option<String>,
}

pub(super) fn parse_version(raw: &str) -> Option<(u64, u64, u64)> {
    let start = raw.find(|c: char| c.is_ascii_digit())?;
    let end = raw[start..]
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .map_or(raw.len(), |index| start + index);
    let parts: Vec<&str> = raw[start..end].split('.').collect();
    if parts.len() < 3 {
        return None;
    }
    Some((
        parts[0].parse().ok()?,
        parts[1].parse().ok()?,
        parts[2].parse().ok()?,
    ))
}

pub(super) fn json_u64(value: &Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_i64().and_then(|value| u64::try_from(value).ok()))
}

pub(super) fn sum_u64(value: &Value, keys: &[&str]) -> Option<u64> {
    let mut total: u64 = 0;
    let mut found = false;
    for key in keys {
        if let Some(value) = value.get(*key).and_then(json_u64) {
            total = total.saturating_add(value);
            found = true;
        }
    }
    found.then_some(total)
}

pub(super) fn epoch_seconds(value: u64) -> u64 {
    if value >= 100_000_000_000 {
        value / 1000
    } else {
        value
    }
}

pub(super) fn limit_signal(text: &str) -> bool {
    let text = text.to_lowercase();
    [
        "rate limit",
        "rate_limit",
        "ratelimit",
        "usage limit",
        "usage_limit",
        "quota",
        "too many requests",
        "429",
        "credits_required",
        "out_of_credits",
        "usage credits",
        "token budget",
        "session limit",
    ]
    .iter()
    .any(|needle| text.contains(needle))
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
    compatible: fn(&str) -> VersionCompatibility,
    mut run: impl FnMut(&str, &[&str]) -> io::Result<Output>,
) -> DetectResult {
    let mut result = DetectResult {
        id,
        installed: false,
        signed_in: false,
        version: None,
        version_compatibility: VersionCompatibility::Unknown,
        error: None,
    };
    let version = match run(binary, &["--version"]) {
        Ok(output)
            if output.status.success()
                && !String::from_utf8_lossy(&output.stdout).trim().is_empty() =>
        {
            result.installed = true;
            Some(output)
        }
        Ok(_) => {
            result.installed = true;
            result.error = Some(format!(
                "无法检测 {binary} 版本：--version 返回失败或空输出"
            ));
            None
        }
        Err(error) => {
            if error.kind() != io::ErrorKind::NotFound {
                result.installed = true;
            }
            result.error = Some(format!("无法启动 {binary}：{error}"));
            if !result.installed {
                return result;
            }
            None
        }
    };
    if let Some(output) = version {
        let raw = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        result.version_compatibility = compatible(&raw);
        result.version = Some(raw);
    }
    match run(binary, auth_args)
        .map_err(|error| format!("无法检查 {binary} 登录状态：{error}"))
        .and_then(|output| parse(&output))
    {
        Ok(true) => result.signed_in = true,
        Ok(false) => {}
        Err(error) => {
            if result.error.is_none() {
                result.error = Some(error);
            }
        }
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
        compatible: fn(&str) -> VersionCompatibility,
        fixtures: &str,
    ) {
        let cases: serde_json::Value = serde_json::from_str(fixtures).unwrap();
        for case in cases.as_array().unwrap() {
            let mut calls = 0;
            let result = detect_with(id, binary, args, parse, compatible, |command, actual_args| {
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
            assert!(result.installed);
            assert_eq!(
                result.signed_in,
                case["status"].as_str().unwrap() == "found"
            );
            assert_eq!(result.version_compatibility, VersionCompatibility::Unverified);
            assert_eq!(result.error.is_some(), case["error"].as_bool().unwrap());
        }
    }

    #[test]
    fn version_failure_still_checks_auth() {
        for version in [output(true, "", ""), output(false, "", "failed")] {
            let mut version = Some(version);
            let result = detect_with(
                "codex",
                "codex",
                &["login", "status"],
                |_| Ok(true),
                |_| VersionCompatibility::Unknown,
                |_, args| {
                    if args == ["--version"] {
                        Ok(version.take().unwrap())
                    } else {
                        Ok(output(true, "", ""))
                    }
                },
            );
            assert!(result.installed);
            assert!(result.signed_in);
            assert!(result.error.is_some());
            assert!(result.version.is_none());
            assert_eq!(result.version_compatibility, VersionCompatibility::Unknown);
        }
        let result = detect_with(
            "codex",
            "codex",
            &["login", "status"],
            |_| Ok(true),
            |_| VersionCompatibility::Unknown,
            |_, args| {
                if args == ["--version"] {
                    Ok(output(true, "1.2.3", ""))
                } else {
                    Err(io::ErrorKind::TimedOut.into())
                }
            },
        );
        assert!(result.installed);
        assert!(!result.signed_in);
        assert!(result.error.is_some());
    }

    #[test]
    fn missing_and_unlaunchable_binaries() {
        for (kind, installed) in [
            (io::ErrorKind::NotFound, false),
            (io::ErrorKind::PermissionDenied, true),
            (io::ErrorKind::TimedOut, true),
        ] {
            let result = detect_with(
                "codex",
                "codex",
                &[],
                |_| Ok(true),
                |_| VersionCompatibility::Unknown,
                |_, _| Err(kind.into()),
            );
            assert_eq!(result.installed, installed);
            assert!(!result.signed_in);
            assert!(result.error.is_some());
            assert!(result.version.is_none());
            assert_eq!(result.version_compatibility, VersionCompatibility::Unknown);
        }
    }

    #[test]
    fn version_usage_and_limit_helpers() {
        assert_eq!(parse_version("2.1.330 (Claude Code)"), Some((2, 1, 330)));
        assert_eq!(parse_version("codex-cli 0.171.0"), Some((0, 171, 0)));
        assert_eq!(parse_version("not a version"), None);
        assert_eq!(epoch_seconds(1_700_000_000_000), 1_700_000_000);
        assert_eq!(epoch_seconds(1_700_000_000), 1_700_000_000);
        assert!(limit_signal("429 Too Many Requests"));
        assert!(limit_signal("shared rollout token budget exhausted"));
        assert!(!limit_signal("Done."));
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

        let write_permissions = crate::roles::Permissions {
            allow: vec!["Write".into(), "Edit".into()],
            deny: vec![],
        };
        let mut options = fake_options(&cwd, "continue", &write_permissions);
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
        assert!(codex_resume.contains("--config sandbox_mode=workspace-write"));
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

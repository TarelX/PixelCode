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

pub(super) fn run(binary: &str, args: &[&str]) -> io::Result<Output> {
    let mut child = Command::new(binary)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let start = Instant::now();
    loop {
        if child.try_wait()?.is_some() {
            return child.wait_with_output();
        }
        if start.elapsed() >= Duration::from_secs(5) {
            child.kill()?;
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
}

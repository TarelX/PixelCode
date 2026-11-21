use super::{DetectResult, detect_with, run};
use std::process::Output;

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

#[cfg(test)]
mod tests {
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

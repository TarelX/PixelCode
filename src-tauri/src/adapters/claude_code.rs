use super::{DetectResult, detect_with, run};
use std::process::Output;

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

#[cfg(test)]
mod tests {
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
}

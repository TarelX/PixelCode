use rusqlite::{Connection, params};
use tauri::WebviewWindow;
use tauri_plugin_notification::NotificationExt;

use crate::settings::{self, Locale};

#[derive(Clone, Copy)]
pub(crate) enum Kind {
    NeedsAnswer,
    Blocked,
    ReadyToDeliver,
    Failed,
}

fn db_error(error: rusqlite::Error) -> String {
    format!("无法记录系统通知：{error}")
}

fn now_ms() -> Result<i64, String> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .map_err(|error| format!("无法读取系统时间：{error}"))
}

fn copy(locale: Locale, kind: Kind) -> (&'static str, &'static str) {
    match (locale, kind) {
        (Locale::ZhCn, Kind::NeedsAnswer) => ("需要回答", "PixelCode 有队长澄清问题等待回答。"),
        (Locale::ZhCn, Kind::Blocked) => ("任务被阻塞", "PixelCode 计划卡执行遇到阻塞。"),
        (Locale::ZhCn, Kind::ReadyToDeliver) => ("计划待交付", "PixelCode 计划卡已可交付确认。"),
        (Locale::ZhCn, Kind::Failed) => ("运行失败", "PixelCode 运行失败，请返回基地查看。"),
        (Locale::En, Kind::NeedsAnswer) => ("Answer needed", "PixelCode is waiting for your answer to a captain clarification."),
        (Locale::En, Kind::Blocked) => ("Task blocked", "A PixelCode task is blocked."),
        (Locale::En, Kind::ReadyToDeliver) => ("Ready to deliver", "A PixelCode plan is ready for delivery confirmation."),
        (Locale::En, Kind::Failed) => ("Run failed", "A PixelCode run failed. Return to the base for details."),
    }
}

pub(crate) fn notify(
    window: &WebviewWindow,
    conn: &Connection,
    event_id: &str,
    kind: Kind,
    camp_id: &str,
    plan_id: Option<&str>,
    task_id: Option<&str>,
) -> Result<(), String> {
    if !settings::notifications_enabled(conn)? {
        return Ok(());
    }
    if window.is_focused().unwrap_or(true) {
        return Ok(());
    }
    let inserted = conn
        .execute(
            "INSERT OR IGNORE INTO notification_log (event_id, created_at) VALUES (?1, ?2)",
            params![event_id, now_ms()?],
        )
        .map_err(db_error)?;
    if inserted == 0 {
        return Ok(());
    }
    let locale = settings::read_locale(conn).unwrap_or_default();
    let (title, body) = copy(locale, kind);
    window
        .notification()
        .builder()
        .title(title)
        .body(body)
        .extra("campId", camp_id)
        .extra("planId", plan_id.unwrap_or(""))
        .extra("taskId", task_id.unwrap_or(""))
        .show()
        .map_err(|error| format!("无法发送系统通知：{error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notification_copy_has_no_prompt_or_code_content() {
        for locale in [Locale::ZhCn, Locale::En] {
            for kind in [Kind::NeedsAnswer, Kind::Blocked, Kind::ReadyToDeliver, Kind::Failed] {
                let (title, body) = copy(locale, kind);
                assert!(!title.is_empty() && !body.is_empty());
                assert!(!body.contains("```"));
            }
        }
    }
}

use std::{collections::BTreeMap, path::Path, sync::Mutex};

use rusqlite::{Connection, params};

fn statuses(
    conn: &Connection,
    camp: &str,
    start: i64,
    end: i64,
) -> Result<BTreeMap<String, i64>, String> {
    if end <= start || end - start > 90_000 {
        return Err("战报日期范围无效".into());
    }
    let mut query = conn
        .prepare(
            "SELECT status, count(*) FROM (
         SELECT status FROM tasks WHERE camp_id=?1
         AND coalesce(updated_at, created_at)>=?2 AND coalesce(updated_at, created_at)<?3
         UNION ALL
         SELECT 'pending' FROM plans, json_each(plans.plan_json, '$.tasks')
         WHERE plans.camp_id=?1 AND plans.status='draft'
         AND plans.created_at>=?2 AND plans.created_at<?3
         ) GROUP BY status",
        )
        .map_err(|error| format!("无法读取今日进度：{error}"))?;
    query
        .query_map(params![camp, start, end], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .and_then(|rows| rows.collect())
        .map_err(|error| format!("无法读取今日进度：{error}"))
}

#[tauri::command]
pub(crate) fn battle_report_counts(
    conn: tauri::State<'_, Mutex<Connection>>,
    camp_id: String,
    start: i64,
    end: i64,
) -> Result<BTreeMap<String, i64>, String> {
    let conn = conn.lock().map_err(|error| error.to_string())?;
    statuses(&conn, &camp_id, start, end)
}

#[tauri::command]
pub(crate) fn export_battle_report(path: String, bytes: Vec<u8>) -> Result<(), String> {
    let path = Path::new(&path);
    if !path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("png"))
    {
        return Err("战报文件必须使用 .png 扩展名".into());
    }
    if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err("战报数据不是 PNG 图片".into());
    }
    std::fs::write(path, bytes).map_err(|error| format!("无法保存战报：{error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_and_saves_png() {
        let dir = std::env::temp_dir().join(format!("pixelcode-report-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("report.PNG");
        let name = path.to_string_lossy().into_owned();
        let bytes = b"\x89PNG\r\n\x1a\n".to_vec();
        assert!(
            export_battle_report(
                dir.join("report.jpg").to_string_lossy().into(),
                bytes.clone()
            )
            .is_err()
        );
        assert!(export_battle_report(name.clone(), vec![]).is_err());
        assert!(export_battle_report(name.clone(), b"not png".to_vec()).is_err());
        export_battle_report(name.clone(), bytes.clone()).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        assert!(
            export_battle_report(
                dir.join("missing/report.png").to_string_lossy().into(),
                bytes
            )
            .is_err()
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn counts_current_status_for_tasks_updated_today_in_this_camp() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE tasks (camp_id TEXT, status TEXT, created_at INTEGER, updated_at INTEGER);
            CREATE TABLE plans (camp_id TEXT, status TEXT, created_at INTEGER, plan_json TEXT);
            INSERT INTO plans VALUES ('a','draft',100,'{\"tasks\":[{},{}]}'),
            ('a','draft',99,'{\"tasks\":[{}]}'), ('b','draft',100,'{\"tasks\":[{}]}'),
            ('a','executing',100,'{\"tasks\":[{}]}');
            INSERT INTO tasks VALUES ('a','done',1,100), ('a','running',100,NULL),
            ('a','failed',1,99), ('a','pending',200,200), ('b','done',100,100);").unwrap();
        assert_eq!(
            statuses(&conn, "a", 100, 200).unwrap(),
            BTreeMap::from([
                ("done".into(), 1),
                ("running".into(), 1),
                ("pending".into(), 2)
            ])
        );
        assert!(statuses(&conn, "empty", 100, 200).unwrap().is_empty());
        assert!(statuses(&conn, "a", 200, 100).is_err());
        conn.execute_batch("DROP TABLE tasks").unwrap();
        assert!(statuses(&conn, "a", 100, 200).is_err());
    }
}

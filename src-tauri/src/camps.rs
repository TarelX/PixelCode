use std::{
    path::{Path, PathBuf},
    sync::Mutex,
};

use rusqlite::{Connection, params};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Camp {
    id: String,
    name: Option<String>,
    root_path: String,
    created_at: Option<i64>,
    last_opened_at: Option<i64>,
}

fn read_camp(row: &rusqlite::Row<'_>) -> rusqlite::Result<Camp> {
    Ok(Camp {
        id: row.get(0)?,
        name: row.get(1)?,
        root_path: row.get(2)?,
        created_at: row.get(3)?,
        last_opened_at: row.get(4)?,
    })
}

fn database_error(error: rusqlite::Error) -> String {
    format!("无法读取或保存基地：{error}")
}

fn save_camp(connection: &Connection, path: &Path) -> Result<Camp, String> {
    let path = path
        .canonicalize()
        .map_err(|error| format!("无法打开所选路径：{error}"))?;
    if !path.is_dir() {
        return Err("所选路径不是文件夹".into());
    }
    let root_path = path.to_str().ok_or("所选路径包含无法识别的字符")?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(root_path);
    connection
        .query_row(
            "INSERT INTO camps (id, name, root_path, created_at, last_opened_at)
         VALUES (lower(hex(randomblob(16))), ?1, ?2, unixepoch(),
             max(unixepoch(), coalesce((SELECT max(last_opened_at) + 1 FROM camps), 0)))
         ON CONFLICT(root_path) DO UPDATE SET last_opened_at = excluded.last_opened_at
         RETURNING id, name, root_path, created_at, last_opened_at",
            params![name, root_path],
            read_camp,
        )
        .map_err(database_error)
}

fn recent_camps(connection: &Connection) -> Result<Vec<Camp>, String> {
    let mut statement = connection
        .prepare(
            "SELECT id, name, root_path, created_at, last_opened_at FROM camps
         ORDER BY last_opened_at DESC, root_path ASC LIMIT 5",
        )
        .map_err(database_error)?;
    statement
        .query_map([], read_camp)
        .map_err(database_error)?
        .collect::<rusqlite::Result<_>>()
        .map_err(database_error)
}

pub(crate) fn run_root(connection: &Connection, camp_id: &str) -> Result<PathBuf, String> {
    let stored: String = connection
        .query_row(
            "SELECT root_path FROM camps WHERE id = ?1",
            [camp_id],
            |row| row.get(0),
        )
        .map_err(|error| match error {
            rusqlite::Error::QueryReturnedNoRows => "未找到要运行的基地".into(),
            error => database_error(error),
        })?;
    let path = Path::new(&stored);
    let root = path
        .canonicalize()
        .map_err(|error| format!("无法打开基地目录：{error}"))?;
    if root != path || !root.is_dir() {
        return Err("基地目录已更改，请重新选择基地".into());
    }
    Ok(root)
}

#[tauri::command]
pub fn open_camp(
    connection: tauri::State<'_, Mutex<Connection>>,
    path: String,
) -> Result<Camp, String> {
    let connection = connection
        .lock()
        .map_err(|_| "无法访问基地数据库".to_string())?;
    save_camp(&connection, Path::new(&path))
}

#[tauri::command]
pub fn list_recent_camps(
    connection: tauri::State<'_, Mutex<Connection>>,
) -> Result<Vec<Camp>, String> {
    let connection = connection
        .lock()
        .map_err(|_| "无法访问基地数据库".to_string())?;
    recent_camps(&connection)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture(std::path::PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "pixelcode-camps-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    fn validates_paths_without_saving_invalid_entries() {
        let fixture = Fixture::new();
        let connection = crate::db::open(&fixture.0).unwrap();
        assert!(
            save_camp(&connection, &fixture.0.join("missing"))
                .unwrap_err()
                .contains("无法打开")
        );
        assert_eq!(
            save_camp(&connection, &fixture.0.join("pixelcode.db")).unwrap_err(),
            "所选路径不是文件夹"
        );
        assert!(recent_camps(&connection).unwrap().is_empty());
        let camp = save_camp(&connection, &fixture.0.join(".")).unwrap();
        assert_eq!(
            camp.root_path,
            fixture.0.canonicalize().unwrap().to_str().unwrap()
        );
    }

    #[test]
    fn run_root_requires_a_saved_directory() {
        let fixture = Fixture::new();
        let connection = crate::db::open(&fixture.0).unwrap();
        let directory = fixture.0.join("camp");
        std::fs::create_dir(&directory).unwrap();
        let camp = save_camp(&connection, &directory).unwrap();
        assert_eq!(
            run_root(&connection, &camp.id).unwrap(),
            directory.canonicalize().unwrap()
        );
        assert!(run_root(&connection, "missing").is_err());
        std::fs::remove_dir(&directory).unwrap();
        assert!(run_root(&connection, &camp.id).is_err());
    }

    #[test]
    fn recent_five_are_unique_and_reopening_preserves_identity() {
        let fixture = Fixture::new();
        let connection = crate::db::open(&fixture.0).unwrap();
        let first = save_camp(&connection, &fixture.0).unwrap();
        for index in 0..6 {
            let path = fixture.0.join(index.to_string());
            std::fs::create_dir(&path).unwrap();
            save_camp(&connection, &path).unwrap();
        }
        let recent = recent_camps(&connection).unwrap();
        assert_eq!(recent.len(), 5);
        assert!(Path::new(&recent[0].root_path).ends_with("5"));
        assert!(Path::new(&recent[4].root_path).ends_with("1"));
        let reopened = save_camp(&connection, &fixture.0.join(".")).unwrap();
        assert_eq!(reopened.id, first.id);
        assert_eq!(reopened.created_at, first.created_at);
        assert!(reopened.last_opened_at > first.last_opened_at);
        assert_eq!(recent_camps(&connection).unwrap()[0].id, first.id);
        let count: i64 = connection
            .query_row("SELECT count(*) FROM camps", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 7);
    }

    #[test]
    fn database_errors_are_readable() {
        let connection = Connection::open_in_memory().unwrap();
        assert!(
            recent_camps(&connection)
                .unwrap_err()
                .contains("无法读取或保存基地")
        );
        let fixture = Fixture::new();
        assert!(
            save_camp(&connection, &fixture.0)
                .unwrap_err()
                .contains("无法读取或保存基地")
        );
    }
}

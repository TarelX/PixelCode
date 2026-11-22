use std::{
    fs,
    path::{Component, Path, PathBuf},
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

use rusqlite::{Connection, params};
use serde::Serialize;

pub struct ArtifactRoot(pub PathBuf);

#[derive(Clone, Serialize)]
pub struct Artifact {
    pub id: String,
    pub task_id: String,
    pub r#type: String,
    pub file_path: String,
    pub content_preview: String,
    pub created_at: i64,
    pub absolute_path: PathBuf,
}

#[derive(Serialize)]
pub struct ArtifactContent {
    artifact: Artifact,
    pub(crate) content: String,
}

const OUTSIDE: &str = "产物路径越出允许目录";

pub fn resolve(root: &Path, relative: &Path) -> Result<PathBuf, String> {
    let mut normalized = PathBuf::new();
    for component in relative.components() {
        match component {
            Component::Normal(part) => normalized.push(part),
            Component::CurDir => (),
            Component::ParentDir if normalized.pop() => (),
            _ => return Err(OUTSIDE.into()),
        }
    }
    if normalized.as_os_str().is_empty() {
        return Err("产物路径不能为空".into());
    }
    Ok(root.join(normalized))
}

fn checked(root: &Path, path: &Path) -> Result<PathBuf, String> {
    let root = root
        .canonicalize()
        .map_err(|e| format!("无法访问产物目录：{e}"))?;
    let path = path
        .canonicalize()
        .map_err(|e| format!("无法访问产物文件：{e}"))?;
    if !path.starts_with(root) {
        return Err(OUTSIDE.into());
    }
    Ok(path)
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && !id.contains(['/', '\\'])
        && !id.contains("..")
        && id != "."
        && !id.contains(':')
}

pub fn write_artifact(
    conn: &Connection,
    root: &Path,
    camp_id: &str,
    task_id: &str,
    kind: &str,
    relative: &Path,
    content: &str,
) -> Result<Artifact, String> {
    if !valid_id(camp_id) || !valid_id(task_id) {
        return Err("产物所属基地或任务 ID 无效".into());
    }
    let local = resolve(Path::new(""), relative)?;
    let relative = Path::new(camp_id).join(task_id).join(local);
    fs::create_dir_all(root).map_err(|e| format!("无法创建产物目录：{e}"))?;
    let path = resolve(root, &relative)?;
    let parent = path.parent().ok_or(OUTSIDE)?;
    let mut current = root.to_path_buf();
    for component in relative.parent().ok_or(OUTSIDE)?.components() {
        current.push(component);
        if !current.exists() {
            fs::create_dir(&current).map_err(|e| format!("无法创建产物目录：{e}"))?;
        }
        checked(root, &current)?;
    }
    checked(root, parent)?;
    if path.symlink_metadata().is_ok() {
        checked(root, &path)?;
    }
    let created_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| format!("无法读取系统时间：{e}"))?
        .as_millis() as i64;
    let artifact = Artifact {
        id: format!(
            "artifact-{created_at}-{}",
            std::sync::atomic::AtomicU64::fetch_add(
                &NEXT_ID,
                1,
                std::sync::atomic::Ordering::Relaxed
            )
        ),
        task_id: task_id.into(),
        r#type: kind.into(),
        file_path: relative
            .components()
            .map(|c| c.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/"),
        content_preview: content.chars().take(200).collect(),
        created_at,
        absolute_path: parent
            .canonicalize()
            .map_err(|e| format!("无法访问产物目录：{e}"))?
            .join(path.file_name().ok_or(OUTSIDE)?),
    };
    fs::write(&path, content).map_err(|e| format!("无法写入产物：{e}"))?;
    conn.execute("INSERT INTO artifacts (id, task_id, type, file_path, content_preview, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)", params![artifact.id, artifact.task_id, artifact.r#type, artifact.file_path, artifact.content_preview, artifact.created_at]).map_err(|e| format!("无法保存产物记录：{e}"))?;
    Ok(artifact)
}

static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Artifact> {
    Ok(Artifact {
        id: row.get(0)?,
        task_id: row.get(1)?,
        r#type: row.get(2)?,
        file_path: row.get(3)?,
        content_preview: row.get::<_, Option<String>>(4)?.unwrap_or_default(),
        created_at: row.get(5)?,
        absolute_path: PathBuf::new(),
    })
}

pub(crate) fn read(conn: &Connection, root: &Path, id: &str) -> Result<ArtifactContent, String> {
    let mut artifact = conn.query_row("SELECT id, task_id, type, file_path, content_preview, created_at FROM artifacts WHERE id = ?1", [id], from_row).map_err(|e| format!("无法读取产物记录：{e}"))?;
    artifact.absolute_path = checked(root, &resolve(root, Path::new(&artifact.file_path))?)?;
    let content =
        fs::read_to_string(&artifact.absolute_path).map_err(|e| format!("无法读取产物：{e}"))?;
    Ok(ArtifactContent { artifact, content })
}

#[tauri::command]
pub fn read_artifact(
    conn: tauri::State<'_, Mutex<Connection>>,
    root: tauri::State<'_, ArtifactRoot>,
    artifact_id: String,
) -> Result<ArtifactContent, String> {
    let conn = conn.lock().map_err(|_| "无法访问产物数据库")?;
    read(&conn, &root.0, &artifact_id)
}

#[tauri::command]
pub fn list_artifacts(
    conn: tauri::State<'_, Mutex<Connection>>,
    root: tauri::State<'_, ArtifactRoot>,
    task_id: String,
) -> Result<Vec<Artifact>, String> {
    let conn = conn.lock().map_err(|_| "无法访问产物数据库")?;
    let mut statement = conn.prepare("SELECT id, task_id, type, file_path, content_preview, created_at FROM artifacts WHERE task_id = ?1 ORDER BY created_at, id").map_err(|e| format!("无法查询产物：{e}"))?;
    let rows = statement
        .query_map([task_id], from_row)
        .map_err(|e| format!("无法查询产物：{e}"))?;
    rows.map(|row| {
        let mut artifact = row.map_err(|e| format!("无法读取产物记录：{e}"))?;
        artifact.absolute_path =
            checked(&root.0, &resolve(&root.0, Path::new(&artifact.file_path))?)?;
        Ok(artifact)
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestDir(PathBuf);
    impl TestDir {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "pixelcode-artifacts-{}-{}",
                std::process::id(),
                NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for TestDir {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    fn rejects_unsafe_paths_and_ids() {
        for path in ["", "../x", "a/../../x"] {
            assert!(resolve(Path::new("root"), Path::new(path)).is_err());
        }
        assert!(resolve(Path::new("root"), &std::env::current_dir().unwrap()).is_err());
        for id in ["", "..", "a/b", "a\\b", "a..b"] {
            assert!(!valid_id(id));
        }
        assert!(
            resolve(Path::new("root"), Path::new("a/../b"))
                .unwrap()
                .ends_with("b")
        );
    }

    #[test]
    fn reviewer_report_is_saved_as_review_md() {
        let dir = TestDir::new();
        let conn = crate::db::open(dir.path()).unwrap();
        let root = dir.path().join("artifacts");
        let config = crate::roles::tests::reviewer_config();
        for content in ["", "结论：通过\n问题清单：未发现问题"] {
            let artifact = write_artifact(
                &conn,
                &root,
                "camp",
                "review",
                "report",
                Path::new(config.artifact_name),
                content,
            )
            .unwrap();
            assert_eq!(artifact.file_path, "camp/review/review.md");
            assert_eq!(
                fs::read_to_string(&artifact.absolute_path).unwrap(),
                content
            );
            assert_eq!(read(&conn, &root, &artifact.id).unwrap().content, content);
        }
        assert!(!root.join("camp/review/output.md").exists());
    }

    #[test]
    fn captain_persists_an_inert_draft_and_matching_plan_json() {
        let dir = TestDir::new();
        let mut conn = crate::db::open(dir.path()).unwrap();
        let roster = crate::roles::planning_roster(&conn, "camp").unwrap();
        let json = include_str!("../../tests/scheduler/captain-plan.json");
        let crate::plans::Reply::Draft(plan) = crate::plans::parse(json, &roster, None).unwrap()
        else {
            panic!("fixture must be a draft");
        };
        let root = dir.path().join("artifacts");
        let artifact = plan
            .save(&mut conn, &root, "camp", "plan-1", "Fix login", 1)
            .unwrap();
        let (stored, status): (String, String) = conn
            .query_row(
                "SELECT plan_json, status FROM plans WHERE id = 'plan-1' AND camp_id = 'camp'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(status, "draft");
        assert_eq!(artifact.file_path, "camp/plan-1/plan.json");
        assert_eq!(read(&conn, &root, &artifact.id).unwrap().content, stored);
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM tasks", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            0
        );
        let invalid_root = &artifact.absolute_path;
        assert!(
            plan.save(&mut conn, invalid_root, "camp", "plan-2", "Retry", 2)
                .is_err()
        );
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM plans", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            1
        );
    }

    #[test]
    fn nested_roundtrip_and_unicode_preview() {
        let dir = TestDir::new();
        let conn = crate::db::open(dir.path()).unwrap();
        let root = dir.path().join("artifacts");
        for content in [String::new(), "中文产物".repeat(80)] {
            let artifact = write_artifact(
                &conn,
                &root,
                "camp",
                "task",
                "report",
                Path::new("nested/output.md"),
                &content,
            )
            .unwrap();
            let result = read(&conn, &root, &artifact.id).unwrap();
            assert_eq!(result.content, content);
            assert_eq!(
                result.artifact.content_preview,
                content.chars().take(200).collect::<String>()
            );
            assert!(
                Path::new(&result.artifact.file_path).ends_with(
                    Path::new("camp")
                        .join("task")
                        .join("nested")
                        .join("output.md")
                )
            );
        }
        assert!(read(&conn, &root, "missing").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn blocks_symlink_escape_on_write_and_read() {
        let dir = TestDir::new();
        let outside = TestDir::new();
        let conn = crate::db::open(dir.path()).unwrap();
        let root = dir.path().join("artifacts");
        let artifact = write_artifact(
            &conn,
            &root,
            "camp",
            "task",
            "report",
            Path::new("output.md"),
            "safe",
        )
        .unwrap();
        fs::remove_dir_all(root.join("camp")).unwrap();
        std::os::unix::fs::symlink(outside.path(), root.join("camp")).unwrap();
        assert!(
            write_artifact(
                &conn,
                &root,
                "camp",
                "task",
                "report",
                Path::new("output.md"),
                "unsafe"
            )
            .is_err()
        );
        fs::create_dir(outside.path().join("task")).unwrap();
        fs::write(outside.path().join("task/output.md"), "outside").unwrap();
        assert_eq!(read(&conn, &root, &artifact.id).err().unwrap(), OUTSIDE);
    }
}

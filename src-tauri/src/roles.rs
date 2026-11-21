use std::sync::Mutex;

use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};

const PRESETS: &[&str] = &[
    include_str!("../../roles/diaochan.yaml"),
    include_str!("../../roles/guanyu.yaml"),
    include_str!("../../roles/kongming.yaml"),
    include_str!("../../roles/lvbu.yaml"),
    include_str!("../../roles/pangtong.yaml"),
    include_str!("../../roles/xushu.yaml"),
    include_str!("../../roles/zhangzhao.yaml"),
];
const SKILLS: &[(&str, &str)] = &[
    (
        "api-design-conventions",
        include_str!("../../skills/api-design-conventions/SKILL.md"),
    ),
    (
        "bug-reproduction",
        include_str!("../../skills/bug-reproduction/SKILL.md"),
    ),
    (
        "code-review-checklist",
        include_str!("../../skills/code-review-checklist/SKILL.md"),
    ),
    (
        "frontend-component-usage",
        include_str!("../../skills/frontend-component-usage/SKILL.md"),
    ),
    (
        "pixel-design-basics",
        include_str!("../../skills/pixel-design-basics/SKILL.md"),
    ),
    (
        "test-writing",
        include_str!("../../skills/test-writing/SKILL.md"),
    ),
];
const ROLE_TYPES: &[&str] = &[
    "captain",
    "architect",
    "designer",
    "frontend",
    "backend",
    "debugger",
    "reviewer",
];

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoleSource {
    #[default]
    Preset,
    ModifiedPreset,
    Custom,
}

#[derive(Debug, Serialize)]
pub struct SkillInfo {
    id: &'static str,
    summary: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Loadout {
    cli: String,
    model: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Permissions {
    pub(crate) allow: Vec<String>,
    pub(crate) deny: Vec<String>,
}

const READ_ONLY_TOOLS: &[&str] = &["Read", "Grep", "Glob"];
const READ_ONLY_DENY: &[&str] = &["Write", "Edit", "Bash", "NotebookEdit"];

fn read_only_tool(rule: &str) -> bool {
    READ_ONLY_TOOLS.contains(&rule.split_once('(').map_or(rule, |(tool, _)| tool))
}

pub(crate) struct RunConfig {
    pub cli: String,
    pub model: String,
    pub system_prompt: String,
    pub permissions: Permissions,
    pub artifact_name: &'static str,
    pub is_captain: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RoleConfig {
    id: String,
    role_type: String,
    name: String,
    name_en: Option<String>,
    title: Option<String>,
    avatar: Option<String>,
    personality: String,
    skills: Vec<String>,
    loadouts: Vec<Loadout>,
    permissions: Permissions,
    task_types: Vec<String>,
    #[serde(skip_deserializing)]
    source: RoleSource,
}

fn presets() -> Result<Vec<RoleConfig>, String> {
    PRESETS
        .iter()
        .map(|text| {
            serde_yaml::from_str(text).map_err(|error| format!("无法读取预设角色：{error}"))
        })
        .collect()
}

fn skills() -> Vec<SkillInfo> {
    SKILLS
        .iter()
        .map(|(id, text)| SkillInfo {
            id,
            summary: text
                .lines()
                .map(str::trim)
                .find(|line| !line.is_empty() && !line.starts_with('#'))
                .unwrap_or_default()
                .to_string(),
        })
        .collect()
}

fn validate(role: &RoleConfig) -> Result<(), String> {
    if matches!(role.role_type.as_str(), "captain" | "reviewer")
        && role
            .permissions
            .allow
            .iter()
            .any(|rule| !read_only_tool(rule))
    {
        return Err("队长和审查员仅允许只读工具 Read、Grep、Glob".into());
    }
    if role.id.is_empty()
        || !role.id.split('-').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        })
    {
        return Err("角色 ID 必须是小写英文字母、数字及单个连字符组成的非空标识".into());
    }
    if role.name.trim().is_empty() || role.personality.trim().is_empty() {
        return Err("角色名称和人设不能为空".into());
    }
    if !ROLE_TYPES.contains(&role.role_type.as_str()) {
        return Err("角色类型必须是队长、架构、设计、前端、后端、调试或审查类型".into());
    }
    if role.loadouts.is_empty()
        || role
            .loadouts
            .iter()
            .any(|l| !matches!(l.cli.as_str(), "claude-code" | "codex"))
    {
        return Err("角色至少需要一个武器槽，CLI 仅支持 claude-code 或 codex".into());
    }
    if let Some(skill) = role
        .skills
        .iter()
        .find(|skill| !SKILLS.iter().any(|(id, _)| id == skill))
    {
        return Err(format!("未找到内置技能：{skill}"));
    }
    Ok(())
}

fn database_error(error: rusqlite::Error) -> String {
    format!("无法读取或保存角色：{error}")
}

fn json<T: Serialize>(value: &T) -> Result<String, String> {
    serde_json::to_string(value).map_err(|error| format!("无法编码角色配置：{error}"))
}

fn effective_roles(
    connection: &Connection,
    camp_id: Option<&str>,
) -> Result<Vec<RoleConfig>, String> {
    let mut roles = presets()?;
    let mut statement = connection
        .prepare(
            "SELECT id, role_type, name, name_en, title, avatar, personality,
         skills_json, loadouts_json, permissions_json, task_types_json FROM roles
         WHERE (camp_id IS NULL AND NOT EXISTS (
             SELECT 1 FROM roles scoped WHERE scoped.id = roles.id AND scoped.camp_id = ?1
         )) OR camp_id = ?1
         ORDER BY created_at, id",
        )
        .map_err(database_error)?;
    let rows = statement
        .query_map([camp_id], |row| {
            Ok((
                RoleConfig {
                    id: row.get(0)?,
                    role_type: row.get(1)?,
                    name: row.get(2)?,
                    name_en: row.get(3)?,
                    title: row.get(4)?,
                    avatar: row.get(5)?,
                    personality: row.get(6)?,
                    skills: vec![],
                    loadouts: vec![],
                    permissions: Permissions {
                        allow: vec![],
                        deny: vec![],
                    },
                    task_types: vec![],
                    source: RoleSource::Custom,
                },
                [
                    row.get::<_, String>(7)?,
                    row.get(8)?,
                    row.get(9)?,
                    row.get(10)?,
                ],
            ))
        })
        .map_err(database_error)?;
    for row in rows {
        let (mut role, data) = row.map_err(database_error)?;
        let decode_error = |error| format!("无法读取角色 {} 的配置：{error}", role.id);
        role.skills = serde_json::from_str(&data[0]).map_err(decode_error)?;
        role.loadouts = serde_json::from_str(&data[1]).map_err(decode_error)?;
        role.permissions = serde_json::from_str(&data[2]).map_err(decode_error)?;
        role.task_types = serde_json::from_str(&data[3]).map_err(decode_error)?;
        if let Some(index) = roles.iter().position(|preset| preset.id == role.id) {
            role.source = RoleSource::ModifiedPreset;
            roles[index] = role;
        } else {
            roles.push(role);
        }
    }
    Ok(roles)
}

pub(crate) fn run_config(
    connection: &Connection,
    camp_id: &str,
    role_id: &str,
    loadout_index: usize,
) -> Result<RunConfig, String> {
    let mut role = effective_roles(connection, Some(camp_id))?
        .into_iter()
        .find(|role| role.id == role_id)
        .ok_or("未找到要运行的角色")?;
    let loadout = role
        .loadouts
        .get(loadout_index)
        .ok_or("未找到角色的武器槽")?;
    if matches!(role.role_type.as_str(), "captain" | "reviewer") {
        role.permissions.allow.retain(|rule| read_only_tool(rule));
        for tool in READ_ONLY_DENY {
            if !role.permissions.deny.iter().any(|rule| rule == tool) {
                role.permissions.deny.push((*tool).into());
            }
        }
    }
    let artifact_name = match role.role_type.as_str() {
        "captain" => "plan.json",
        "reviewer" => "review.md",
        _ => "output.md",
    };
    let mut system_prompt = role.personality;
    for id in role.skills {
        let (_, text) = SKILLS
            .iter()
            .find(|(skill, _)| *skill == id)
            .ok_or_else(|| format!("未找到内置技能：{id}"))?;
        system_prompt.push_str("\n\n");
        system_prompt.push_str(text);
    }
    Ok(RunConfig {
        cli: loadout.cli.clone(),
        model: loadout.model.clone(),
        system_prompt,
        permissions: role.permissions,
        artifact_name,
        is_captain: role.role_type == "captain",
    })
}

#[derive(Serialize)]
pub(crate) struct PlanningRole {
    pub id: String,
    pub(crate) role_type: String,
    title: Option<String>,
    task_types: Vec<String>,
}

pub(crate) fn planning_roster(
    connection: &Connection,
    camp_id: &str,
) -> Result<Vec<PlanningRole>, String> {
    Ok(effective_roles(connection, Some(camp_id))?
        .into_iter()
        .map(|role| PlanningRole {
            id: role.id,
            role_type: role.role_type,
            title: role.title,
            task_types: role.task_types,
        })
        .collect())
}

fn upsert(connection: &Connection, role: &RoleConfig, camp_id: Option<&str>) -> Result<(), String> {
    validate(role)?;
    connection
        .execute(
            "INSERT INTO roles (id, camp_id, role_type, name, name_en, title, avatar, personality,
         skills_json, loadouts_json, permissions_json, task_types_json, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, unixepoch(), unixepoch())
         ON CONFLICT DO UPDATE SET role_type=excluded.role_type,
         name=excluded.name, name_en=excluded.name_en, title=excluded.title, avatar=excluded.avatar,
         personality=excluded.personality, skills_json=excluded.skills_json,
         loadouts_json=excluded.loadouts_json, permissions_json=excluded.permissions_json,
         task_types_json=excluded.task_types_json, updated_at=excluded.updated_at",
            params![
                role.id,
                camp_id,
                role.role_type,
                role.name,
                role.name_en,
                role.title,
                role.avatar,
                role.personality,
                json(&role.skills)?,
                json(&role.loadouts)?,
                json(&role.permissions)?,
                json(&role.task_types)?
            ],
        )
        .map_err(database_error)?;
    Ok(())
}

fn remove(connection: &Connection, id: &str) -> Result<(), String> {
    connection
        .execute("DELETE FROM roles WHERE id = ?1", [id])
        .map_err(database_error)?;
    Ok(())
}

#[tauri::command]
pub fn list_roles(
    connection: tauri::State<'_, Mutex<Connection>>,
    camp_id: Option<String>,
) -> Result<Vec<RoleConfig>, String> {
    let connection = connection.lock().map_err(|_| "无法访问角色数据库")?;
    effective_roles(&connection, camp_id.as_deref())
}

#[tauri::command]
pub fn list_skills() -> Vec<SkillInfo> {
    skills()
}

#[tauri::command]
pub fn save_role(
    connection: tauri::State<'_, Mutex<Connection>>,
    role: RoleConfig,
    camp_id: Option<String>,
) -> Result<(), String> {
    let connection = connection.lock().map_err(|_| "无法访问角色数据库")?;
    upsert(&connection, &role, camp_id.as_deref())
}

#[tauri::command]
pub fn delete_role(
    connection: tauri::State<'_, Mutex<Connection>>,
    id: String,
) -> Result<(), String> {
    let connection = connection.lock().map_err(|_| "无法访问角色数据库")?;
    remove(&connection, &id)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn connection() -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(include_str!("../migrations/001_initial.sql"))
            .unwrap();
        connection
            .execute_batch(include_str!("../migrations/002_role_task_types.sql"))
            .unwrap();
        connection
    }

    #[test]
    fn bundled_presets_are_unique_and_valid() {
        let roles = presets().unwrap();
        assert_eq!(roles.len(), 7);
        let ids: std::collections::HashSet<_> = roles.iter().map(|r| &r.id).collect();
        assert_eq!(ids.len(), 7);
        for role in &roles {
            validate(role).unwrap();
        }
    }

    #[test]
    fn lists_builtin_skills_with_summaries() {
        let skills = skills();
        assert_eq!(skills.len(), SKILLS.len());
        let test = skills.iter().find(|s| s.id == "test-writing").unwrap();
        assert_eq!(test.summary, "测试编写规范");
        assert!(skills.iter().all(|s| !s.summary.is_empty()));
    }

    #[test]
    fn run_config_uses_camp_override_and_selected_loadout() {
        let connection = connection();
        let mut role = presets().unwrap().remove(0);
        role.personality = "Camp persona".into();
        role.skills = vec!["test-writing".into()];
        role.loadouts[0].model = "haiku".into();
        role.permissions.allow = vec!["Read".into()];
        upsert(&connection, &role, Some("camp")).unwrap();
        let config = run_config(&connection, "camp", &role.id, 0).unwrap();
        assert_eq!(config.cli, "claude-code");
        assert_eq!(config.model, "haiku");
        assert_eq!(
            config.system_prompt,
            format!(
                "Camp persona\n\n{}",
                include_str!("../../skills/test-writing/SKILL.md")
            )
        );
        assert_eq!(config.permissions.allow, ["Read"]);
        assert_eq!(config.artifact_name, "output.md");
        assert_eq!(
            run_config(&connection, "another-camp", &role.id, 0)
                .unwrap()
                .model,
            "sonnet"
        );
        assert_eq!(
            run_config(&connection, "camp", &role.id, 1).unwrap().cli,
            "codex"
        );
        assert!(run_config(&connection, "camp", &role.id, 99).is_err());
    }

    pub(crate) fn reviewer_config() -> RunConfig {
        run_config(&connection(), "camp", "reviewer", 0).unwrap()
    }

    #[test]
    fn captain_permissions_are_clamped_even_for_legacy_rows() {
        let connection = connection();
        let mut role = presets()
            .unwrap()
            .into_iter()
            .find(|r| r.id == "captain")
            .unwrap();
        assert!(validate(&role).is_ok());
        upsert(&connection, &role, Some("camp")).unwrap();
        role.permissions
            .allow
            .extend(["Write".into(), "Edit".into(), "Bash".into()]);
        assert!(validate(&role).is_err());
        connection
            .execute(
                "UPDATE roles SET permissions_json = ?1 WHERE id = 'captain' AND camp_id = 'camp'",
                [json(&role.permissions).unwrap()],
            )
            .unwrap();
        let run = run_config(&connection, "camp", "captain", 1).unwrap();
        assert!(run.is_captain);
        assert_eq!(run.artifact_name, "plan.json");
        assert_eq!(run.permissions.allow, READ_ONLY_TOOLS);
    }

    #[test]
    fn reviewer_permissions_and_artifact_are_read_only() {
        let config = reviewer_config();
        assert_eq!(config.permissions.allow, ["Read", "Grep", "Glob"]);
        assert_eq!(config.artifact_name, "review.md");
        for tool in READ_ONLY_DENY {
            assert!(config.permissions.deny.iter().any(|rule| rule == tool));
        }
    }

    #[test]
    fn reviewer_overrides_are_rejected_and_legacy_permissions_are_clamped() {
        let connection = connection();
        let mut role = presets()
            .unwrap()
            .into_iter()
            .find(|r| r.role_type == "reviewer")
            .unwrap();
        for id in ["reviewer", "custom-reviewer"] {
            role.id = id.into();
            role.name = "Custom".into();
            for tool in [
                "Write",
                "Edit",
                "Bash(git status *)",
                "NotebookEdit",
                "mcp__write",
            ] {
                role.permissions.allow = vec!["Read".into(), tool.into()];
                assert!(
                    upsert(&connection, &role, Some("camp"))
                        .unwrap_err()
                        .contains("仅允许只读工具")
                );
            }
            role.permissions.allow = vec!["Read".into()];
            role.permissions.deny.clear();
            upsert(&connection, &role, Some("camp")).unwrap();
            connection.execute(
                "UPDATE roles SET permissions_json = ?1 WHERE id = ?2",
                params![r#"{"allow":["Read","Write","Edit","Bash","NotebookEdit","mcp__write"],"deny":[]}"#, id],
            ).unwrap();
            let config = run_config(&connection, "camp", id, 0).unwrap();
            assert_eq!(config.permissions.allow, ["Read"]);
            for tool in READ_ONLY_DENY {
                assert!(config.permissions.deny.iter().any(|rule| rule == tool));
            }
            role.permissions.allow.clear();
            upsert(&connection, &role, Some("camp")).unwrap();
            assert!(
                run_config(&connection, "camp", id, 0)
                    .unwrap()
                    .permissions
                    .allow
                    .is_empty()
            );
            role.permissions.allow = vec!["Read(src/**)".into()];
            role.permissions.deny = vec!["Read(secrets/**)".into()];
            upsert(&connection, &role, Some("camp")).unwrap();
            let config = run_config(&connection, "camp", id, 0).unwrap();
            assert_eq!(config.permissions.allow, ["Read(src/**)"]);
            assert!(config.permissions.deny.contains(&"Read(secrets/**)".into()));
        }
        let config = run_config(&connection, "camp", "frontend", 0).unwrap();
        assert!(config.permissions.allow.iter().any(|tool| tool == "Write"));
        assert!(!config.permissions.deny.iter().any(|tool| tool == "Bash"));
        assert!(run_config(&connection, "camp", "missing", 0).is_err());
    }

    #[test]
    fn override_custom_scoping_and_delete() {
        let connection = connection();
        let original = presets().unwrap();
        assert_eq!(effective_roles(&connection, None).unwrap().len(), 7);
        let mut role = original[0].clone();
        role.name = "修改名称".into();
        upsert(&connection, &role, None).unwrap();
        upsert(&connection, &role, None).unwrap();
        let roles = effective_roles(&connection, Some("other")).unwrap();
        assert_eq!(roles[0].name, role.name);
        assert_eq!(roles[0].source, RoleSource::ModifiedPreset);
        assert_eq!(roles[0].task_types, role.task_types);
        role.name = "大营名称".into();
        upsert(&connection, &role, Some("camp")).unwrap();
        upsert(&connection, &role, Some("camp")).unwrap();
        assert_eq!(effective_roles(&connection, Some("camp")).unwrap().len(), 7);
        assert_eq!(
            effective_roles(&connection, None).unwrap()[0].name,
            "修改名称"
        );
        assert_eq!(
            effective_roles(&connection, Some("other")).unwrap()[0].name,
            "修改名称"
        );
        assert_eq!(
            effective_roles(&connection, Some("camp")).unwrap()[0].name,
            role.name
        );
        remove(&connection, &role.id).unwrap();
        assert_eq!(
            effective_roles(&connection, Some("camp")).unwrap()[0].source,
            RoleSource::Preset
        );
        role.id = "custom".into();
        upsert(&connection, &role, None).unwrap();
        role.id = "local".into();
        upsert(&connection, &role, Some("camp")).unwrap();
        assert_eq!(effective_roles(&connection, None).unwrap().len(), 8);
        let roles = effective_roles(&connection, Some("camp")).unwrap();
        assert_eq!(roles[7].id, "custom");
        assert_eq!(roles[8].id, "local");
        assert_eq!(roles[8].source, RoleSource::Custom);
        remove(&connection, "local").unwrap();
        remove(&connection, "missing").unwrap();
        assert_eq!(effective_roles(&connection, Some("camp")).unwrap().len(), 8);
    }

    #[test]
    fn rejects_invalid_configs_without_writing() {
        let connection = connection();
        let original = presets().unwrap().remove(0);
        for (field, value) in [
            ("id", serde_json::json!("")),
            ("id", serde_json::json!("Bad")),
            ("id", serde_json::json!("../bad")),
            ("id", serde_json::json!("a--b")),
            ("name", serde_json::json!(" ")),
            ("personality", serde_json::json!("")),
            ("role_type", serde_json::json!("unknown")),
            ("loadouts", serde_json::json!([])),
            (
                "loadouts",
                serde_json::json!([{"cli":"unknown","model":"auto"}]),
            ),
            ("skills", serde_json::json!(["missing"])),
        ] {
            let mut value_role = serde_json::to_value(&original).unwrap();
            value_role[field] = value;
            let role = serde_json::from_value(value_role).unwrap();
            assert!(upsert(&connection, &role, None).is_err(), "{field}");
        }
        assert_eq!(effective_roles(&connection, None).unwrap().len(), 7);
        connection.execute_batch("DROP TABLE roles").unwrap();
        assert!(
            effective_roles(&connection, None)
                .unwrap_err()
                .contains("无法读取或保存角色")
        );
    }
}

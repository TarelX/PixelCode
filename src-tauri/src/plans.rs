use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};

use crate::{artifacts, roles::PlanningRole};

#[derive(Serialize)]
pub(crate) struct StoredPlan {
    id: String,
    status: String,
    scenario: String,
    user_request: String,
    created_at: i64,
    plan: Plan,
}

fn latest(conn: &Connection, camp_id: &str) -> Result<Option<StoredPlan>, String> {
    conn.query_row(
        "SELECT id, status, scenario, user_request, created_at, plan_json FROM plans
         WHERE camp_id = ?1 ORDER BY created_at DESC, rowid DESC LIMIT 1",
        [camp_id],
        |row| {
            let json: String = row.get(5)?;
            let plan = serde_json::from_str(&json).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    5,
                    rusqlite::types::Type::Text,
                    Box::new(error),
                )
            })?;
            Ok(StoredPlan {
                id: row.get(0)?,
                status: row.get(1)?,
                scenario: row.get(2)?,
                user_request: row.get(3)?,
                created_at: row.get(4)?,
                plan,
            })
        },
    )
    .optional()
    .map_err(|error| format!("无法读取锦囊计划：{error}"))
}

#[tauri::command]
pub(crate) fn latest_plan(
    conn: tauri::State<'_, std::sync::Mutex<Connection>>,
    camp_id: String,
) -> Result<Option<StoredPlan>, String> {
    let conn = conn.lock().map_err(|error| error.to_string())?;
    latest(&conn, &camp_id)
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Clarification {
    questions: Vec<String>,
    answer: Option<String>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Plan {
    scenario: String,
    recruited_roles: Vec<String>,
    pub(crate) tasks: Vec<Task>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Task {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) status: Option<String>,
    pub(crate) id: String,
    pub(crate) title: String,
    pub(crate) description: String,
    pub(crate) role_id: String,
    pub(crate) depends_on: Vec<String>,
    pub(crate) input_artifacts: Vec<String>,
    pub(crate) output_artifacts: Vec<String>,
    pub(crate) acceptance_criteria: String,
    pub(crate) check_command: Option<CheckCommand>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CheckCommand {
    pub(crate) program: String,
    pub(crate) args: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Questions {
    pub questions: Vec<String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
pub(crate) enum Reply {
    Draft(Plan),
    Clarify(Questions),
}

pub(crate) const INSTRUCTIONS: &str = r#"
You are a read-only planner. Do not implement tasks, edit files, run checks, approve, or deliver.
The JSON input carries the user's request, the camp's roster, and optional clarification.
Use roster IDs only. Tailor the task count and dependencies to the request. For example, bug fixes
may use reproduce → fix → review; new projects may use architecture → design → implementation → test.
Check and delivery gates are run by the application, not by you.
Return exactly one JSON object with no Markdown or commentary in your final answer.
If essential details are missing, return {"questions":["A concise question in the user's language"]}.
If clarification.answer is null, the user asked you to decide: make reasonable assumptions and return a draft.
Otherwise use this draft shape (check_command can be null if there is no applicable check):
{"scenario":"bug-fix","recruited_roles":["debugger"],"tasks":[
  {"id":"reproduce","title":"…","description":"…","role_id":"debugger","depends_on":[],
   "input_artifacts":[],"output_artifacts":["repro.md"],"acceptance_criteria":"…",
   "check_command":{"program":"npm","args":["test"]}}
]}
Use short ASCII task IDs and relative artifact names. Dependencies are task IDs in this plan.
Recruit every assigned role. Use single-executable check suggestions with argument arrays, never shell scripts.
"#;

pub(crate) fn input(
    request: &str,
    roster: &[PlanningRole],
    clarification: Option<&Clarification>,
) -> Result<String, String> {
    if let Some(clarification) = clarification {
        validate_questions(&clarification.questions)?;
        if clarification
            .answer
            .as_ref()
            .is_some_and(|s| s.trim().is_empty())
        {
            return Err("澄清答复不能为空".into());
        }
    }
    Ok(serde_json::json!({
        "user_request": request.trim(),
        "roster": roster,
        "clarification": clarification,
    })
    .to_string())
}

fn validate_questions(questions: &[String]) -> Result<(), String> {
    if questions.is_empty() || questions.len() > 5 || questions.iter().any(|s| s.trim().is_empty())
    {
        return Err("队长须提供 1–5 个非空澄清问题".into());
    }
    Ok(())
}

pub(crate) fn parse(
    output: &str,
    roster: &[PlanningRole],
    clarification: Option<&Clarification>,
) -> Result<Reply, String> {
    if output.len() > 256 * 1024 {
        return Err("队长回复超过 256 KiB".into());
    }
    let reply: Reply = serde_json::from_str(output)
        .map_err(|error| format!("队长未返回有效的草案或澄清 JSON：{error}"))?;
    match &reply {
        Reply::Draft(plan) => plan.validate(roster)?,
        Reply::Clarify(_) if clarification.is_some_and(|c| c.answer.is_none()) => {
            return Err("选择由队长决定后，队长仍返回了澄清问题；请重试".into());
        }
        Reply::Clarify(questions) => validate_questions(&questions.questions)?,
    }
    Ok(reply)
}

impl Plan {
    fn validate(&self, roster: &[PlanningRole]) -> Result<(), String> {
        if self.scenario.trim().is_empty() || self.tasks.is_empty() {
            return Err("草案必须包含任务类型和至少一张锦囊".into());
        }
        let available: HashSet<_> = roster.iter().map(|role| role.id.as_str()).collect();
        let mut recruited = HashSet::new();
        for id in &self.recruited_roles {
            if !available.contains(id.as_str()) || !recruited.insert(id.as_str()) {
                return Err(format!("草案招募了未知或重复的角色：{id}"));
            }
        }
        let mut indices = HashMap::new();
        for (index, task) in self.tasks.iter().enumerate() {
            if task.id.is_empty()
                || !task
                    .id
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_'))
                || indices.insert(task.id.as_str(), index).is_some()
            {
                return Err(format!("锦囊 ID 无效或重复：{}", task.id));
            }
            if !recruited.contains(task.role_id.as_str()) {
                return Err(format!(
                    "锦囊 {} 分派给未招募的角色：{}",
                    task.id, task.role_id
                ));
            }
            if [&task.title, &task.description, &task.acceptance_criteria]
                .iter()
                .any(|s| s.trim().is_empty())
            {
                return Err(format!("锦囊 {} 缺少标题、描述或验收标准", task.id));
            }
            for path in task.input_artifacts.iter().chain(&task.output_artifacts) {
                if path
                    .split('/')
                    .any(|part| part.trim().is_empty() || matches!(part, "." | ".."))
                    || path.contains([':', '\\'])
                    || path.chars().any(char::is_control)
                {
                    return Err(format!("锦囊 {} 的产物名称不是安全的相对路径", task.id));
                }
            }
            if let Some(command) = &task.check_command
                && (command.program.trim().is_empty()
                    || command.program.contains('\0')
                    || command.args.iter().any(|arg| arg.contains('\0')))
            {
                return Err(format!("锦囊 {} 的检查命令无效", task.id));
            }
        }

        let mut followers = vec![Vec::new(); self.tasks.len()];
        let mut incoming = Vec::with_capacity(self.tasks.len());
        for (index, task) in self.tasks.iter().enumerate() {
            let mut seen = HashSet::new();
            for id in &task.depends_on {
                let parent = indices
                    .get(id.as_str())
                    .ok_or_else(|| format!("锦囊 {} 依赖不存在的锦囊：{id}", task.id))?;
                if !seen.insert(parent) {
                    return Err(format!("锦囊 {} 的依赖重复：{id}", task.id));
                }
                followers[*parent].push(index);
            }
            incoming.push(seen.len());
        }
        let mut ready: Vec<_> = incoming
            .iter()
            .enumerate()
            .filter_map(|(i, &count)| (count == 0).then_some(i))
            .collect();
        let mut visited = 0;
        while let Some(index) = ready.pop() {
            visited += 1;
            for &next in &followers[index] {
                incoming[next] -= 1;
                if incoming[next] == 0 {
                    ready.push(next);
                }
            }
        }
        if visited != self.tasks.len() {
            return Err("锦囊草案存在循环依赖".into());
        }
        Ok(())
    }

    pub(crate) fn save(
        &self,
        conn: &mut Connection,
        root: &Path,
        camp_id: &str,
        id: &str,
        request: &str,
        created_at: i64,
    ) -> Result<artifacts::Artifact, String> {
        let db_error = |error| format!("无法保存锦囊草案：{error}");
        let json =
            serde_json::to_string_pretty(self).map_err(|e| format!("无法编码锦囊草案：{e}"))?;
        let transaction = conn.transaction().map_err(db_error)?;
        transaction.execute(
            "INSERT INTO plans (id, camp_id, scenario, user_request, plan_json, status, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 'draft', ?6)",
            params![id, camp_id, self.scenario, request.trim(), json, created_at],
        ).map_err(db_error)?;
        let artifact = artifacts::write_artifact(
            &transaction,
            root,
            camp_id,
            id,
            "plan",
            Path::new("plan.json"),
            &json,
        )?;
        transaction.commit().map_err(db_error)?;
        Ok(artifact)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    #[test]
    fn latest_plan_is_scoped_ordered_and_parsed() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(include_str!("../migrations/001_initial.sql"))
            .unwrap();
        assert!(latest(&conn, "camp").unwrap().is_none());
        let fixture = include_str!("../../tests/scheduler/captain-plan.json");
        for (id, camp, time) in [
            ("old", "camp", 1),
            ("new", "camp", 2),
            ("other", "other", 3),
        ] {
            conn.execute(
                "INSERT INTO plans VALUES (?1, ?2, 'bug-fix', 'Fix login', ?3, 'draft', ?4)",
                params![id, camp, fixture, time],
            )
            .unwrap();
        }
        let stored = latest(&conn, "camp").unwrap().unwrap();
        assert_eq!(stored.id, "new");
        assert_eq!(stored.status, "draft");
        assert_eq!(stored.scenario, "bug-fix");
        assert_eq!(stored.user_request, "Fix login");
        assert_eq!(stored.created_at, 2);
        assert_eq!(
            serde_json::to_value(stored.plan).unwrap(),
            serde_json::from_str::<Value>(fixture).unwrap()
        );
        conn.execute("UPDATE plans SET plan_json = '{}' WHERE id = 'new'", [])
            .unwrap();
        assert!(latest(&conn, "camp").is_err());
    }

    fn roster() -> Vec<PlanningRole> {
        crate::roles::planning_roster(&crate::roles::tests::connection(), "camp").unwrap()
    }

    #[test]
    fn accepts_out_of_order_dag_but_rejects_invalid_drafts() {
        let roster = roster();
        let fixture = include_str!("../../tests/scheduler/captain-plan.json");
        assert!(matches!(
            parse(fixture, &roster, None).unwrap(),
            Reply::Draft(_)
        ));
        let original: Value = serde_json::from_str(fixture).unwrap();
        for (pointer, invalid, error) in [
            ("/tasks/1/id", json!("fix"), "ID 无效或重复"),
            ("/tasks/1/depends_on", json!(["fix"]), "循环依赖"),
            (
                "/tasks/0/depends_on",
                json!(["reproduce", "reproduce"]),
                "依赖重复",
            ),
            ("/tasks/1/depends_on", json!(["missing"]), "依赖不存在"),
            ("/tasks/1/role_id", json!("reviewer"), "未招募"),
            ("/recruited_roles/0", json!("missing"), "未知或重复"),
            ("/tasks/1/title", json!(" "), "缺少标题"),
            ("/tasks/1/output_artifacts", json!(["../repo"]), "相对路径"),
            (
                "/tasks/0/check_command",
                json!("npm test"),
                "有效的草案或澄清 JSON",
            ),
        ] {
            let mut draft = original.clone();
            *draft.pointer_mut(pointer).unwrap() = invalid;
            let result = parse(&draft.to_string(), &roster, None);
            assert!(
                matches!(result, Err(reason) if reason.contains(error)),
                "{pointer}"
            );
        }
    }

    #[test]
    fn clarification_is_separate_from_a_draft() {
        let roster = roster();
        let question = r#"{"questions":["Which browser?"]}"#;
        assert!(matches!(
            parse(question, &roster, None).unwrap(),
            Reply::Clarify(_)
        ));
        let followup = Clarification {
            questions: vec!["Which browser?".into()],
            answer: None,
        };
        assert!(parse(question, &roster, Some(&followup)).is_err());
        assert!(parse(r#"{"questions":[" "]}"#, &roster, None).is_err());
        let encoded: Value =
            serde_json::from_str(&input("Fix the refresh bug", &roster, Some(&followup)).unwrap())
                .unwrap();
        assert_eq!(encoded["user_request"], "Fix the refresh bug");
        assert_eq!(
            encoded["clarification"],
            json!({"questions":["Which browser?"],"answer":null})
        );
    }
}

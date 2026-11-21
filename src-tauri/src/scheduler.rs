use std::{
    collections::{HashMap, HashSet},
    io::Read,
    path::Path,
    process::{Command, Stdio},
};

use rusqlite::{Connection, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager, WebviewWindow};

use crate::{
    adapters::claude_code::StartOptions,
    artifacts, camps,
    plans::{self, CheckCommand, Plan},
    roles,
    supervisor::{self, RunSlot},
};

#[derive(Clone, Serialize)]
struct TaskEvent {
    camp_id: String,
    plan_id: String,
    task_id: String,
    status: String,
    attempts: u32,
    review_rounds: u32,
    message: Option<String>,
}

trait Runner {
    fn run(&mut self, role: &str, prompt: &str) -> Result<String, String>;
    fn check(&mut self, command: &CheckCommand) -> Result<(bool, String), String>;
    fn stopped(&self) -> bool;
    fn emit(&self, event: &TaskEvent) -> Result<(), String>;
}

fn db_error(error: rusqlite::Error) -> String {
    format!("无法更新锦囊执行记录：{error}")
}

fn approve(
    conn: &mut Connection,
    camp: &str,
    id: &str,
    assignments: &HashMap<String, String>,
) -> Result<Plan, String> {
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(db_error)?;
    let mut plan = plans::latest(&tx, camp)?
        .filter(|stored| stored.id == id && stored.status == "draft")
        .ok_or("只有当前大营最新的待点将草案可以批准")?
        .plan;
    let roster = roles::planning_roster(&tx, camp)?;
    plan.assign(assignments, &roster)?;
    for task in &mut plan.tasks {
        task.status = Some("pending".into());
    }
    let json = serde_json::to_string(&plan).map_err(|e| format!("无法编码锦囊计划：{e}"))?;
    tx.execute(
        "UPDATE plans SET status='executing',plan_json=?2 WHERE id=?1",
        params![id, json],
    )
    .map_err(db_error)?;
    for task in &plan.tasks {
        let dependencies: Vec<_> = task
            .depends_on
            .iter()
            .map(|dep| format!("{id}-{dep}"))
            .collect();
        tx.execute(
            "INSERT INTO tasks (id,camp_id,plan_id,title,description,role_id,depends_on,input_artifacts,output_artifacts,acceptance_criteria,check_command,status,created_at,updated_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,'pending',unixepoch(),unixepoch())",
            params![format!("{id}-{}", task.id), camp, id, task.title, task.description, task.role_id,
                serde_json::to_string(&dependencies).unwrap(), serde_json::to_string(&task.input_artifacts).unwrap(),
                serde_json::to_string(&task.output_artifacts).unwrap(), task.acceptance_criteria,
                task.check_command.as_ref().map(|c| serde_json::to_string(c).unwrap())],
        ).map_err(db_error)?;
    }
    tx.commit().map_err(db_error)?;
    Ok(plan)
}

fn finish(conn: &Connection, id: &str, succeeded: bool) -> Result<(), String> {
    conn.execute(
        "UPDATE plans SET status=CASE
            WHEN ?2 AND EXISTS (SELECT 1 FROM tasks WHERE plan_id=?1)
                AND NOT EXISTS (SELECT 1 FROM tasks WHERE plan_id=?1 AND status<>'done')
            THEN 'awaiting_delivery' ELSE 'aborted' END
         WHERE id=?1 AND status='executing'",
        params![id, succeeded],
    )
    .map_err(db_error)?;
    Ok(())
}

fn order(plan: &Plan) -> Result<Vec<usize>, String> {
    let mut incoming: Vec<_> = plan
        .tasks
        .iter()
        .map(|task| task.depends_on.len())
        .collect();
    let mut result = Vec::new();
    while result.len() < plan.tasks.len() {
        let next = (0..plan.tasks.len())
            .find(|&i| incoming[i] == 0 && !result.contains(&i))
            .ok_or("锦囊依赖存在循环或缺失")?;
        result.push(next);
        for (i, task) in plan.tasks.iter().enumerate() {
            if task.depends_on.contains(&plan.tasks[next].id) {
                incoming[i] -= 1;
            }
        }
    }
    Ok(result)
}

fn status(
    conn: &Connection,
    runner: &impl Runner,
    event: &mut TaskEvent,
    next: &str,
    message: Option<String>,
) -> Result<(), String> {
    event.status = next.into();
    event.message = message;
    let tx = conn.unchecked_transaction().map_err(db_error)?;
    tx.execute("UPDATE tasks SET status=?1,attempts=?2,review_rounds=?3,updated_at=unixepoch() WHERE id=?4",
        params![next, event.attempts, event.review_rounds, event.task_id]).map_err(db_error)?;
    let json: String = tx
        .query_row(
            "SELECT plan_json FROM plans WHERE id=?1",
            [&event.plan_id],
            |row| row.get(0),
        )
        .map_err(db_error)?;
    let mut plan: Plan =
        serde_json::from_str(&json).map_err(|e| format!("无法读取锦囊计划：{e}"))?;
    let task = plan
        .tasks
        .iter_mut()
        .find(|task| format!("{}-{}", event.plan_id, task.id) == event.task_id)
        .ok_or("计划中未找到此锦囊")?;
    task.status = Some(next.into());
    let json = serde_json::to_string(&plan).map_err(|e| format!("无法编码锦囊计划：{e}"))?;
    tx.execute(
        "UPDATE plans SET plan_json=?2 WHERE id=?1",
        params![event.plan_id, json],
    )
    .map_err(db_error)?;
    if let Some(message) = &event.message {
        tx.execute("INSERT INTO messages (id,task_id,role,type,content,created_at) VALUES (?1,?2,'system','scheduler',?3,unixepoch())",
            params![format!("{}-{next}-{}-{}", event.task_id, event.attempts, event.review_rounds), event.task_id, message]).map_err(db_error)?;
    }
    tx.commit().map_err(db_error)?;
    runner.emit(event)
}

fn tail(output: &str) -> &str {
    let mut start = output.len().saturating_sub(8192);
    while !output.is_char_boundary(start) {
        start += 1;
    }
    &output[start..]
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Review {
    verdict: String,
    reason: String,
}

fn parse_review(text: &str) -> Option<Review> {
    text.lines()
        .rev()
        .find_map(|line| {
            let line = line.trim();
            let line = line
                .strip_prefix("```json")
                .or_else(|| line.strip_prefix("```"))
                .unwrap_or(line)
                .trim();
            let line = line.strip_suffix("```").unwrap_or(line).trim();
            serde_json::from_str::<Review>(line).ok()
        })
        .filter(|review| {
            matches!(review.verdict.as_str(), "approve" | "reject")
                && !review.reason.trim().is_empty()
        })
}

const REVIEW_VERDICT: &str = r#"不得修改文件。最后仅返回单行 JSON，例如 {"verdict":"approve","reason":"原因"}。verdict 必须为 approve 或 reject。"#;

fn review_rejection(text: &str) -> Option<String> {
    match parse_review(text) {
        Some(review) if review.verdict == "approve" => None,
        Some(review) => Some(review.reason),
        None => Some("审查员未返回有效的 verdict/reason JSON".into()),
    }
}

fn execute(
    conn: &Connection,
    root: &Path,
    camp: &str,
    id: &str,
    plan: &Plan,
    runner: &mut impl Runner,
) -> Result<(), String> {
    let roster = roles::planning_roster(conn, camp)?;
    let reviewer = roster.iter().find(|role| role.role_type == "reviewer");
    let mut completed = HashSet::new();
    for task in &plan.tasks {
        runner.emit(&TaskEvent {
            camp_id: camp.into(),
            plan_id: id.into(),
            task_id: format!("{id}-{}", task.id),
            status: "pending".into(),
            attempts: 0,
            review_rounds: 0,
            message: None,
        })?;
    }
    for index in order(plan)? {
        let task = &plan.tasks[index];
        if runner.stopped() {
            return Ok(());
        }
        let mut event = TaskEvent {
            camp_id: camp.into(),
            plan_id: id.into(),
            task_id: format!("{id}-{}", task.id),
            status: "pending".into(),
            attempts: 0,
            review_rounds: 0,
            message: None,
        };
        let result = (|| {
            let is_reviewer = roster
                .iter()
                .any(|role| role.id == task.role_id && role.role_type == "reviewer");
            let mut prompt = format!(
                "{}\n{}\n验收标准：{}",
                task.title, task.description, task.acceptance_criteria
            );
            for name in &task.input_artifacts {
                let mut found = false;
                for dep in task
                    .depends_on
                    .iter()
                    .filter(|dep| completed.contains(*dep))
                {
                    let dependency = format!("{id}-{dep}");
                    let path = format!("{camp}/{dependency}/{name}");
                    let mut query = conn.prepare("SELECT id FROM artifacts WHERE task_id=?1 AND file_path=?2 ORDER BY created_at DESC, rowid DESC LIMIT 1").map_err(db_error)?;
                    let ids = query
                        .query_map(params![dependency, path], |row| row.get::<_, String>(0))
                        .map_err(db_error)?;
                    for artifact_id in ids {
                        let content =
                            artifacts::read(conn, root, &artifact_id.map_err(db_error)?)?.content;
                        prompt.push_str(&format!("\n输入产物 {name}：\n{content}"));
                        found = true;
                    }
                }
                if !found {
                    return Err(format!("已完成的依赖锦囊缺少输入产物：{name}"));
                }
            }
            if is_reviewer {
                prompt.push_str(&format!("\n{REVIEW_VERDICT}"));
            }
            let mut feedback = String::new();
            loop {
                if runner.stopped() {
                    return Ok(None);
                }
                status(conn, runner, &mut event, "running", None)?;
                let output = runner.run(&task.role_id, &format!("{prompt}\n{feedback}"))?;
                if runner.stopped() {
                    return Ok(None);
                }
                let name = task
                    .output_artifacts
                    .first()
                    .map(String::as_str)
                    .unwrap_or("output.md");
                artifacts::write_artifact(
                    conn,
                    root,
                    camp,
                    &event.task_id,
                    "output",
                    Path::new(name),
                    &output,
                )?;
                status(conn, runner, &mut event, "checking", None)?;
                if let Some(command) = &task.check_command {
                    let (passed, log) = runner.check(command)?;
                    if runner.stopped() {
                        return Ok(None);
                    }
                    artifacts::write_artifact(
                        conn,
                        root,
                        camp,
                        &event.task_id,
                        "check",
                        Path::new(&format!(
                            "checks/{}-{}.log",
                            event.attempts, event.review_rounds
                        )),
                        &log,
                    )?;
                    if !passed {
                        event.attempts += 1;
                        feedback = format!("检查失败，请返工：\n{}", tail(&log));
                        if event.attempts > 3 {
                            status(conn, runner, &mut event, "blocked", Some(feedback))?;
                            return Ok(None);
                        }
                        continue;
                    }
                }
                if is_reviewer {
                    if let Some(reason) = review_rejection(&output) {
                        event.review_rounds += 1;
                        status(
                            conn,
                            runner,
                            &mut event,
                            "blocked",
                            Some(format!("审查未通过：{reason}")),
                        )?;
                        return Ok(None);
                    }
                } else {
                    let reviewer = reviewer.ok_or("大营缺少审查员角色")?;
                    status(conn, runner, &mut event, "reviewing", None)?;
                    let text = runner.run(
                        &reviewer.id,
                        &format!(
                            "审查以下任务及产物。{REVIEW_VERDICT}\n{prompt}\n角色产物：\n{output}"
                        ),
                    )?;
                    if runner.stopped() {
                        return Ok(None);
                    }
                    artifacts::write_artifact(
                        conn,
                        root,
                        camp,
                        &event.task_id,
                        "review",
                        Path::new("review.md"),
                        &text,
                    )?;
                    if let Some(reason) = review_rejection(&text) {
                        event.review_rounds += 1;
                        feedback = format!("审查打回，请返工：{reason}");
                        if event.review_rounds > 3 {
                            status(conn, runner, &mut event, "blocked", Some(feedback))?;
                            return Ok(None);
                        }
                        continue;
                    }
                }
                status(conn, runner, &mut event, "done", None)?;
                return Ok(Some(()));
            }
        })();
        if runner.stopped() && !matches!(result, Ok(Some(()))) {
            status(
                conn,
                runner,
                &mut event,
                "pending",
                Some("用户已停止执行".into()),
            )?;
            return Ok(());
        }
        match result {
            Ok(Some(())) => {
                completed.insert(task.id.clone());
            }
            Ok(None) => return Ok(()),
            Err(error) => {
                status(conn, runner, &mut event, "failed", Some(error.clone()))?;
                return Err(error);
            }
        }
    }
    Ok(())
}

fn check(command: &CheckCommand, cwd: &Path, slot: &RunSlot) -> Result<(bool, String), String> {
    let mut child = Command::new(&command.program)
        .args(&command.args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("无法启动检查命令 {}：{e}", command.program))?;
    let mut stdout = child.stdout.take().ok_or("无法读取检查输出")?;
    let mut stderr = child.stderr.take().ok_or("无法读取检查错误输出")?;
    slot.attach(child)?;
    std::thread::scope(|scope| {
        let out = scope.spawn(move || {
            let mut bytes = Vec::new();
            stdout.read_to_end(&mut bytes).map(|_| bytes)
        });
        let err = scope.spawn(move || {
            let mut bytes = Vec::new();
            stderr.read_to_end(&mut bytes).map(|_| bytes)
        });
        let result = slot.wait();
        let out = out
            .join()
            .map_err(|_| "读取检查输出线程失败")?
            .map_err(|e| format!("无法读取检查输出：{e}"))?;
        let err = err
            .join()
            .map_err(|_| "读取检查错误线程失败")?
            .map_err(|e| format!("无法读取检查错误输出：{e}"))?;
        Ok((
            result?.success(),
            format!(
                "{}{}",
                String::from_utf8_lossy(&out),
                String::from_utf8_lossy(&err)
            ),
        ))
    })
}

struct LiveRunner<'a> {
    conn: &'a Connection,
    camp: &'a str,
    cwd: &'a Path,
    slot: &'a RunSlot,
    window: &'a WebviewWindow,
}

impl Runner for LiveRunner<'_> {
    fn run(&mut self, role: &str, prompt: &str) -> Result<String, String> {
        let config = roles::run_config(self.conn, self.camp, role, 0)?;
        supervisor::dispatch(
            &config.cli,
            StartOptions {
                cwd: self.cwd,
                model: &config.model,
                system_prompt: &config.system_prompt,
                prompt,
                permissions: &config.permissions,
            },
            self.slot,
            &|_| Ok(()),
        )
    }
    fn check(&mut self, command: &CheckCommand) -> Result<(bool, String), String> {
        check(command, self.cwd, self.slot)
    }
    fn stopped(&self) -> bool {
        self.slot.is_stopped()
    }
    fn emit(&self, event: &TaskEvent) -> Result<(), String> {
        self.window
            .emit_to(self.window.label(), "plan-task", event)
            .map_err(|e| format!("无法发送锦囊状态事件：{e}"))
    }
}

#[tauri::command]
pub async fn approve_plan_execution(
    window: WebviewWindow,
    camp_id: String,
    plan_id: String,
    assignments: HashMap<String, String>,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let slot = window.state::<RunSlot>();
        let active = slot.acquire()?;
        let data_dir = window
            .path()
            .app_data_dir()
            .map_err(|e| format!("无法访问应用数据目录：{e}"))?;
        let mut conn =
            crate::db::open(&data_dir).map_err(|e| format!("无法访问大营数据库：{e}"))?;
        conn.busy_timeout(std::time::Duration::from_secs(5))
            .map_err(db_error)?;
        let cwd = camps::run_root(&conn, &camp_id)?;
        let root = window.state::<artifacts::ArtifactRoot>();
        let plan = approve(&mut conn, &camp_id, &plan_id, &assignments)?;
        let result = execute(
            &conn,
            &root.0,
            &camp_id,
            &plan_id,
            &plan,
            &mut LiveRunner {
                conn: &conn,
                camp: &camp_id,
                cwd: &cwd,
                slot: &slot,
                window: &window,
            },
        );
        let final_status = finish(&conn, &plan_id, result.is_ok() && !slot.is_stopped());
        drop(active);
        let _ = window.emit_to(window.label(), "plan-state", &camp_id);
        final_status?;
        result
    })
    .await
    .map_err(|e| format!("锦囊执行失败：{e}"))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[derive(Default)]
    struct Fake {
        failures: u32,
        rejects: u32,
        runs: Vec<String>,
        events: RefCell<Vec<TaskEvent>>,
        error: bool,
        stop: bool,
        invalid_review: bool,
        stop_on_done: bool,
    }
    impl Runner for Fake {
        fn run(&mut self, role: &str, prompt: &str) -> Result<String, String> {
            self.runs.push(role.into());
            if self.error {
                return Err("无法启动角色 CLI".into());
            }
            if role == "reviewer" {
                assert!(prompt.contains(REVIEW_VERDICT));
                if self.invalid_review {
                    return Ok("not JSON".into());
                }
                let verdict = if self.rejects > 0 {
                    self.rejects -= 1;
                    "reject"
                } else {
                    "approve"
                };
                return Ok(format!(
                    "审查已完成。\n```json\n{}\n```",
                    serde_json::json!({"verdict": verdict, "reason": "审查说明"})
                ));
            }
            if role == "backend" {
                assert!(prompt.contains("输入产物 repro.md：\n角色输出"));
            }
            Ok("角色输出".into())
        }
        fn check(&mut self, _: &CheckCommand) -> Result<(bool, String), String> {
            let passed = self.failures == 0;
            self.failures = self.failures.saturating_sub(1);
            Ok((passed, "检查日志".into()))
        }
        fn stopped(&self) -> bool {
            (self.stop && !self.runs.is_empty())
                || (self.stop_on_done
                    && self
                        .events
                        .borrow()
                        .iter()
                        .any(|event| event.status == "done"))
        }
        fn emit(&self, event: &TaskEvent) -> Result<(), String> {
            self.events.borrow_mut().push(event.clone());
            Ok(())
        }
    }

    fn connection(status: &str) -> Connection {
        let conn = roles::tests::connection();
        conn.execute("INSERT INTO plans (id,camp_id,scenario,user_request,plan_json,status,created_at) VALUES ('plan','camp','bug-fix','fix',?1,?2,1)", params![include_str!("../../tests/scheduler/captain-plan.json"), status]).unwrap();
        conn
    }

    fn prepare(conn: &mut Connection, camp: &str, id: &str) -> Result<Plan, String> {
        approve(conn, camp, id, &HashMap::new())
    }

    fn run(fake: &mut Fake) -> Connection {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let root = std::env::current_dir()
            .unwrap()
            .join("target")
            .join(format!(
                "scheduler-test-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
        let mut conn = connection("draft");
        let plan = prepare(&mut conn, "camp", "plan").unwrap();
        let result = execute(&conn, &root, "camp", "plan", &plan, fake);
        assert_eq!(result.is_err(), fake.error);
        finish(&conn, "plan", result.is_ok() && !fake.stopped()).unwrap();
        if root.exists() {
            std::fs::remove_dir_all(root).unwrap();
        }
        conn
    }

    fn state(conn: &Connection, task: &str) -> (String, u32, u32) {
        let state: (String, u32, u32) = conn
            .query_row(
                "SELECT status,attempts,review_rounds FROM tasks WHERE id=?1",
                [format!("plan-{task}")],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        let json: String = conn
            .query_row("SELECT plan_json FROM plans WHERE id='plan'", [], |row| {
                row.get(0)
            })
            .unwrap();
        let plan: Plan = serde_json::from_str(&json).unwrap();
        assert_eq!(
            plan.tasks
                .iter()
                .find(|entry| entry.id == task)
                .unwrap()
                .status
                .as_deref(),
            Some(state.0.as_str())
        );
        state
    }

    #[test]
    fn prepare_resets_existing_json_statuses_and_transitions_are_atomic() {
        let mut conn = connection("draft");
        let json: String = conn
            .query_row("SELECT plan_json FROM plans", [], |row| row.get(0))
            .unwrap();
        let mut plan: Plan = serde_json::from_str(&json).unwrap();
        for task in &mut plan.tasks {
            task.status = Some("done".into());
        }
        conn.execute(
            "UPDATE plans SET plan_json=?1",
            [serde_json::to_string(&plan).unwrap()],
        )
        .unwrap();
        prepare(&mut conn, "camp", "plan").unwrap();
        assert_eq!(state(&conn, "fix").0, "pending");
        assert_eq!(state(&conn, "reproduce").0, "pending");
        let mut event = TaskEvent {
            camp_id: "camp".into(),
            plan_id: "plan".into(),
            task_id: "plan-reproduce".into(),
            status: "pending".into(),
            attempts: 0,
            review_rounds: 0,
            message: None,
        };
        let fake = Fake::default();
        for next in [
            "running",
            "checking",
            "reviewing",
            "blocked",
            "failed",
            "pending",
            "done",
        ] {
            status(&conn, &fake, &mut event, next, None).unwrap();
            assert_eq!(state(&conn, "reproduce").0, next);
        }
        conn.execute_batch("CREATE TRIGGER deny_plan_update BEFORE UPDATE ON plans BEGIN SELECT RAISE(ABORT, 'test'); END").unwrap();
        assert!(status(&conn, &fake, &mut event, "running", None).is_err());
        assert_eq!(state(&conn, "reproduce").0, "done");
    }

    #[test]
    fn review_accepts_last_json_line_after_prose_and_fences() {
        let approve = r#"{"verdict":"approve","reason":"通过"}"#;
        let reject = r#"{"verdict":"reject","reason":"需要修改"}"#;
        for text in [
            format!("审查说明\n  {approve}  "),
            format!("审查说明\n```json\n{approve}\n```"),
            format!("审查说明\n```json {approve}```"),
        ] {
            assert_eq!(parse_review(&text).unwrap().verdict, "approve");
        }
        assert_eq!(
            parse_review(&format!("{approve}\n说明\n{reject}"))
                .unwrap()
                .verdict,
            "reject"
        );
        for text in ["", "只有说明", r#"{"verdict":"approve","reason":""}"#] {
            assert!(parse_review(text).is_none());
        }
    }

    #[test]
    fn stop_after_completion_preserves_done_and_leaves_next_pending() {
        let mut fake = Fake {
            stop_on_done: true,
            ..Fake::default()
        };
        let conn = run(&mut fake);
        assert_eq!(state(&conn, "reproduce").0, "done");
        assert_eq!(state(&conn, "fix").0, "pending");
        assert!(
            !fake
                .events
                .borrow()
                .iter()
                .any(|event| event.status == "pending" && event.message.is_some())
        );
    }

    #[test]
    fn orders_dependencies_and_reworks_checks() {
        let mut fake = Fake {
            failures: 3,
            ..Fake::default()
        };
        let conn = run(&mut fake);
        assert_eq!(fake.runs[0], "debugger");
        assert_eq!(fake.runs[2], "backend");
        assert_eq!(state(&conn, "fix"), ("done".into(), 3, 0));
        assert_eq!(
            conn.query_row("SELECT status FROM plans", [], |r| r.get::<_, String>(0))
                .unwrap(),
            "awaiting_delivery"
        );
    }

    #[test]
    fn four_check_failures_block_and_leave_dependents_pending() {
        let mut conn = connection("draft");
        let mut plan = prepare(&mut conn, "camp", "plan").unwrap();
        plan.tasks[1].check_command = plan.tasks[0].check_command.take();
        let root = std::env::current_dir()
            .unwrap()
            .join("target/scheduler-blocked-test");
        let mut fake = Fake {
            failures: 4,
            ..Fake::default()
        };
        execute(&conn, &root, "camp", "plan", &plan, &mut fake).unwrap();
        finish(&conn, "plan", true).unwrap();
        assert_eq!(state(&conn, "reproduce"), ("blocked".into(), 4, 0));
        assert_eq!(state(&conn, "fix").0, "pending");
        assert_eq!(
            plans::latest(&conn, "camp").unwrap().unwrap().status,
            "aborted"
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn reviews_rework_and_block_at_four_rejects() {
        for (rejects, invalid, expected) in [
            (1, false, "done"),
            (4, false, "blocked"),
            (0, true, "blocked"),
        ] {
            let mut fake = Fake {
                rejects,
                invalid_review: invalid,
                ..Fake::default()
            };
            let conn = run(&mut fake);
            assert_eq!(
                state(&conn, "reproduce"),
                (expected.into(), 0, if expected == "done" { 1 } else { 4 })
            );
            if expected == "blocked" {
                assert_eq!(state(&conn, "fix").0, "pending");
            }
        }
    }

    #[test]
    fn refuses_foreign_or_non_draft_plan_without_changes() {
        for status in [
            "draft",
            "executing",
            "awaiting_delivery",
            "completed",
            "aborted",
        ] {
            let mut conn = connection(status);
            assert!(
                prepare(
                    &mut conn,
                    if status == "draft" { "other" } else { "camp" },
                    "plan"
                )
                .is_err()
            );
            assert_eq!(
                conn.query_row("SELECT count(*) FROM tasks", [], |r| r.get::<_, u32>(0))
                    .unwrap(),
                0
            );
        }
    }

    #[test]
    fn approval_validates_assignments_and_cannot_run_twice() {
        let mut conn = connection("draft");
        let assignment = |role: &str| HashMap::from([("fix".to_string(), role.to_string())]);
        assert!(approve(&mut conn, "camp", "plan", &assignment("unknown")).is_err());
        assert_eq!(
            plans::latest(&conn, "camp").unwrap().unwrap().status,
            "draft"
        );
        let plan = approve(&mut conn, "camp", "plan", &assignment("frontend")).unwrap();
        assert_eq!(plan.tasks[0].role_id, "frontend");
        assert_eq!(
            conn.query_row("SELECT role_id FROM tasks WHERE id='plan-fix'", [], |r| {
                r.get::<_, String>(0)
            })
            .unwrap(),
            "frontend"
        );
        assert!(prepare(&mut conn, "camp", "plan").is_err());
    }

    #[test]
    fn reviewer_tasks_skip_review_and_empty_outputs_use_fallback() {
        let mut conn = connection("draft");
        let mut plan = prepare(&mut conn, "camp", "plan").unwrap();
        plan.tasks.remove(0);
        plan.tasks[0].role_id = "reviewer".into();
        plan.tasks[0].output_artifacts.clear();
        let root = std::env::current_dir()
            .unwrap()
            .join("target/scheduler-reviewer-test");
        let mut fake = Fake::default();
        execute(&conn, &root, "camp", "plan", &plan, &mut fake).unwrap();
        assert_eq!(fake.runs, ["reviewer"]);
        assert_eq!(state(&conn, "reproduce").0, "done");
        assert!(
            root.join("camp")
                .join("plan-reproduce")
                .join("output.md")
                .is_file()
        );
        assert!(
            !fake
                .events
                .borrow()
                .iter()
                .any(|event| event.status == "reviewing")
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn reviewer_task_requires_approval_before_delivery() {
        let root = std::env::current_dir()
            .unwrap()
            .join("target/scheduler-reviewer-verdict-test");
        for (rejects, invalid_review, reason) in [
            (1, false, "审查说明"),
            (0, true, "审查员未返回有效的 verdict/reason JSON"),
        ] {
            let mut conn = connection("draft");
            let plan = approve(
                &mut conn,
                "camp",
                "plan",
                &HashMap::from([("reproduce".into(), "reviewer".into())]),
            )
            .unwrap();
            let mut fake = Fake {
                rejects,
                invalid_review,
                ..Fake::default()
            };

            execute(&conn, &root, "camp", "plan", &plan, &mut fake).unwrap();
            finish(&conn, "plan", true).unwrap();

            assert_eq!(fake.runs, ["reviewer"]);
            assert_eq!(state(&conn, "reproduce"), ("blocked".into(), 0, 1));
            assert_eq!(state(&conn, "fix").0, "pending");
            assert_eq!(
                plans::latest(&conn, "camp").unwrap().unwrap().status,
                "aborted"
            );
            assert_eq!(
                fake.events.borrow().last().unwrap().message.as_deref(),
                Some(format!("审查未通过：{reason}").as_str())
            );
        }
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn cli_error_and_stop_halt_the_plan() {
        for stop in [false, true] {
            let mut fake = Fake {
                error: !stop,
                stop,
                ..Fake::default()
            };
            let conn = run(&mut fake);
            assert_eq!(
                state(&conn, "reproduce").0,
                if stop { "pending" } else { "failed" }
            );
            assert_eq!(state(&conn, "fix").0, "pending");
            assert_eq!(
                plans::latest(&conn, "camp").unwrap().unwrap().status,
                "aborted"
            );
            let message: String = conn
                .query_row("SELECT content FROM messages LIMIT 1", [], |r| r.get(0))
                .unwrap();
            assert!(message.contains(if stop {
                "用户已停止"
            } else {
                "无法启动"
            }));
        }
    }

    #[test]
    fn real_checks_capture_stdout_and_stderr() {
        let slot = RunSlot::default();
        let _active = slot.acquire().unwrap();
        let cwd = std::env::current_dir().unwrap();
        for (arg, passed, text) in [
            ("--version", true, "rustc"),
            ("--no-such-flag", false, "error"),
        ] {
            let (success, output) = check(
                &CheckCommand {
                    program: "rustc".into(),
                    args: vec![arg.into()],
                },
                &cwd,
                &slot,
            )
            .unwrap();
            assert_eq!(success, passed);
            assert!(output.contains(text), "{output}");
        }
        assert!(
            check(
                &CheckCommand {
                    program: "pixelcode-nonexistent-check-command".into(),
                    args: vec![]
                },
                &cwd,
                &slot
            )
            .unwrap_err()
            .contains("无法启动检查命令")
        );
    }
}

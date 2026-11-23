use std::{
    collections::{HashMap, HashSet},
    io::Read,
    path::Path,
    process::{Command, Stdio},
    sync::{
        Mutex, MutexGuard,
        atomic::{AtomicBool, Ordering},
    },
};

use rusqlite::{Connection, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager, WebviewWindow};

use crate::{
    adapters::claude_code::StartOptions,
    artifacts, camps,
    git::GitService,
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

trait Runner: Sync {
    fn run(&self, role: &str, prompt: &str, cwd: &Path) -> Result<String, String>;
    fn check(&self, command: &CheckCommand, cwd: &Path) -> Result<(bool, String), String>;
    fn stopped(&self) -> bool;
    fn emit(&self, event: &TaskEvent) -> Result<(), String>;
}

fn db_error(error: rusqlite::Error) -> String {
    format!("无法更新计划卡执行记录：{error}")
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
        .ok_or("只有当前基地最新的待出战草案可以批准")?
        .plan;
    let roster = roles::planning_roster(&tx, camp)?;
    plan.assign(assignments, &roster)?;
    for task in &mut plan.tasks {
        task.status = Some("pending".into());
    }
    let json = serde_json::to_string(&plan).map_err(|e| format!("无法编码计划卡计划：{e}"))?;
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
            .ok_or("计划卡依赖存在循环或缺失")?;
        result.push(next);
        for (i, task) in plan.tasks.iter().enumerate() {
            if task.depends_on.contains(&plan.tasks[next].id) {
                incoming[i] -= 1;
            }
        }
    }
    Ok(result)
}

fn paths_overlap(left: &str, right: &str) -> bool {
    let mut left = left.split('/');
    let mut right = right.split('/');
    loop {
        match (left.next(), right.next()) {
            (Some(a), Some(b)) if a == b => (),
            (Some(_), Some(_)) => return false,
            _ => return true,
        }
    }
}

fn scopes_overlap(left: &[String], right: &[String]) -> bool {
    !left.is_empty()
        && !right.is_empty()
        && left
            .iter()
            .any(|a| right.iter().any(|b| paths_overlap(a, b)))
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
        serde_json::from_str(&json).map_err(|e| format!("无法读取计划卡计划：{e}"))?;
    let task = plan
        .tasks
        .iter_mut()
        .find(|task| format!("{}-{}", event.plan_id, task.id) == event.task_id)
        .ok_or("计划中未找到此计划卡")?;
    task.status = Some(next.into());
    let json = serde_json::to_string(&plan).map_err(|e| format!("无法编码计划卡计划：{e}"))?;
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

const EMPTY_OUTPUT: &str = "调度器检查：角色未产出任何内容";

fn review_rejection(text: &str) -> Option<String> {
    match parse_review(text) {
        Some(review) if review.verdict == "approve" => None,
        Some(review) => Some(review.reason),
        None => Some("审查员未返回有效的 verdict/reason JSON".into()),
    }
}

const MAX_PARALLEL: usize = 3;
static INTEGRATION: Mutex<()> = Mutex::new(());

fn database(conn: &Mutex<Connection>) -> Result<MutexGuard<'_, Connection>, String> {
    conn.lock().map_err(|_| "无法访问基地数据库".into())
}

struct Execution<'a, R> {
    conn: &'a Mutex<Connection>,
    root: &'a Path,
    cwd: &'a Path,
    worktrees: &'a Path,
    camp: &'a str,
    id: &'a str,
    plan: &'a Plan,
    runner: &'a R,
}

enum Attempt {
    Done,
    Rework(String),
    Blocked(String),
    Stopped,
}

impl<R: Runner> Execution<'_, R> {
    fn execute(&self) -> Result<(), String> {
        let sequence = order(self.plan)?;
        let git = GitService::detect(self.cwd);
        let limit = if git.is_some() { MAX_PARALLEL } else { 1 };
        for task in &self.plan.tasks {
            self.runner.emit(&self.event(&task.id))?;
        }
        let halt = AtomicBool::new(false);
        std::thread::scope(|scope| {
            let (send, receive) = std::sync::mpsc::channel();
            let mut started = HashSet::new();
            let mut completed = HashSet::new();
            let mut active = 0;
            let mut active_tasks: HashSet<usize> = HashSet::new();
            let mut first_error = None;
            loop {
                for &index in &sequence {
                    if active >= limit || halt.load(Ordering::SeqCst) || self.runner.stopped() {
                        break;
                    }
                    let task = &self.plan.tasks[index];
                    if started.contains(&index)
                        || !task.depends_on.iter().all(|dep| completed.contains(dep))
                        || active_tasks.iter().any(|&active_index| {
                            scopes_overlap(&task.files, &self.plan.tasks[active_index].files)
                        })
                    {
                        continue;
                    }
                    started.insert(index);
                    active_tasks.insert(index);
                    active += 1;
                    let send = send.clone();
                    let git = git.as_ref();
                    let halt = &halt;
                    scope.spawn(move || {
                        let result = self.task(index, git, halt);
                        if !matches!(result, Ok(true)) {
                            halt.store(true, Ordering::SeqCst);
                        }
                        let _ = send.send((index, result));
                    });
                }
                if active == 0 {
                    break;
                }
                let (index, result) = receive.recv().map_err(|e| e.to_string())?;
                active -= 1;
                active_tasks.remove(&index);
                match result {
                    Ok(true) => {
                        completed.insert(self.plan.tasks[index].id.clone());
                    }
                    Ok(false) => (),
                    Err(error) => {
                        if first_error.is_none() {
                            first_error = Some(error);
                        }
                    }
                }
            }
            first_error.map_or(Ok(()), Err)
        })
    }

    fn event(&self, task: &str) -> TaskEvent {
        TaskEvent {
            camp_id: self.camp.into(),
            plan_id: self.id.into(),
            task_id: format!("{}-{task}", self.id),
            status: "pending".into(),
            attempts: 0,
            review_rounds: 0,
            message: None,
        }
    }

    fn status(
        &self,
        event: &mut TaskEvent,
        next: &str,
        message: Option<String>,
    ) -> Result<(), String> {
        status(&*database(self.conn)?, self.runner, event, next, message)
    }

    fn artifact(
        &self,
        event: &TaskEvent,
        kind: &str,
        name: &str,
        content: &str,
    ) -> Result<(), String> {
        artifacts::write_artifact(
            &*database(self.conn)?,
            self.root,
            self.camp,
            &event.task_id,
            kind,
            Path::new(name),
            content,
        )?;
        Ok(())
    }

    fn task(
        &self,
        index: usize,
        git: Option<&GitService>,
        halt: &AtomicBool,
    ) -> Result<bool, String> {
        let task = &self.plan.tasks[index];
        let mut event = self.event(&task.id);
        let result = (|| {
            let roster = roles::planning_roster(&*database(self.conn)?, self.camp)?;
            let is_reviewer = roster
                .iter()
                .any(|role| role.id == task.role_id && role.role_type == "reviewer");
            let reviewer = roster.iter().find(|role| role.role_type == "reviewer");
            let mut prompt = format!(
                "{}\n{}\n验收标准：{}",
                task.title, task.description, task.acceptance_criteria
            );
            {
                let conn = database(self.conn)?;
                for name in &task.input_artifacts {
                    let mut found = false;
                    for dep in &task.depends_on {
                        let dependency = format!("{}-{dep}", self.id);
                        let path = format!("{}/{dependency}/{name}", self.camp);
                        let artifact_id = conn.query_row(
                            "SELECT id FROM artifacts WHERE task_id=?1 AND file_path=?2 ORDER BY created_at DESC, rowid DESC LIMIT 1",
                            params![dependency, path], |row| row.get::<_, String>(0));
                        match artifact_id {
                            Ok(id) => {
                                let content = artifacts::read(&conn, self.root, &id)?.content;
                                prompt.push_str(&format!("\n输入产物 {name}：\n{content}"));
                                found = true;
                            }
                            Err(rusqlite::Error::QueryReturnedNoRows) => (),
                            Err(e) => return Err(db_error(e)),
                        }
                    }
                    if !found {
                        return Err(format!("已完成的依赖计划卡缺少输入产物：{name}"));
                    }
                }
            }
            if is_reviewer {
                prompt.push_str(&format!("\n{REVIEW_VERDICT}"));
            }
            let worktree = if let Some(git) = git {
                let _lock = INTEGRATION.lock().map_err(|_| "无法访问改动集成锁")?;
                Some(git.worktree(self.worktrees, self.camp, &event.task_id)?)
            } else {
                None
            };
            let cwd = worktree
                .as_ref()
                .map_or(self.cwd, |tree| tree.path.as_path());
            let mut feedback = String::new();
            loop {
                if self.runner.stopped() {
                    return Ok(false);
                }
                self.status(&mut event, "running", None)?;
                let output = self
                    .runner
                    .run(&task.role_id, &format!("{prompt}\n{feedback}"), cwd);
                let patch = worktree
                    .as_ref()
                    .map(|tree| tree.patch())
                    .transpose()?
                    .unwrap_or_default();
                if !patch.is_empty() {
                    let text = std::str::from_utf8(&patch)
                        .map_err(|e| format!("无法保存改动补丁：{e}"))?;
                    self.artifact(&event, "output", "changes.diff", text)?;
                }
                if self.runner.stopped() {
                    return Ok(false);
                }
                let output = output?;
                self.artifact(
                    &event,
                    "output",
                    task.output_artifacts
                        .first()
                        .map(String::as_str)
                        .unwrap_or("output.md"),
                    &output,
                )?;
                if let Some(reason) = roles::blocked_reason(&output) {
                    halt.store(true, Ordering::SeqCst);
                    self.status(&mut event, "blocked", Some(reason))?;
                    return Ok(false);
                }
                let _integration = INTEGRATION.lock().map_err(|_| "无法访问改动集成锁")?;
                if let Some(git) = git
                    && let Err(error) = git.apply(&patch, false)
                {
                    halt.store(true, Ordering::SeqCst);
                    self.status(
                        &mut event,
                        "blocked",
                        Some(format!("无法把改动并入基地：{error}")),
                    )?;
                    return Ok(false);
                }
                let attempt = (|| {
                    if self.runner.stopped() {
                        return Ok(Attempt::Stopped);
                    }
                    self.status(&mut event, "checking", None)?;
                    let failure = if output.trim().is_empty() {
                        Some(EMPTY_OUTPUT.to_string())
                    } else if let Some(command) = &task.check_command {
                        let (passed, log) = self.runner.check(command, self.cwd)?;
                        if self.runner.stopped() {
                            return Ok(Attempt::Stopped);
                        }
                        self.artifact(
                            &event,
                            "check",
                            &format!("checks/{}-{}.log", event.attempts, event.review_rounds),
                            &log,
                        )?;
                        (!passed).then_some(log)
                    } else {
                        None
                    };
                    if let Some(log) = failure {
                        event.attempts += 1;
                        let feedback = format!("检查失败，请返工：\n{}", tail(&log));
                        return Ok(if event.attempts > 3 {
                            Attempt::Blocked(feedback)
                        } else {
                            Attempt::Rework(feedback)
                        });
                    }
                    if is_reviewer {
                        if let Some(reason) = review_rejection(&output) {
                            event.review_rounds += 1;
                            return Ok(Attempt::Blocked(format!("审查未通过：{reason}")));
                        }
                    } else {
                        let reviewer = reviewer.ok_or("基地缺少审查员角色")?;
                        self.status(&mut event, "reviewing", None)?;
                        let text = self.runner.run(&reviewer.id, &format!("审查以下任务及产物。{REVIEW_VERDICT}\n{prompt}\n角色产物：\n{output}"), cwd)?;
                        if self.runner.stopped() {
                            return Ok(Attempt::Stopped);
                        }
                        self.artifact(&event, "review", "review.md", &text)?;
                        if let Some(reason) = roles::blocked_reason(&text) {
                            return Ok(Attempt::Blocked(reason));
                        }
                        if let Some(reason) = review_rejection(&text) {
                            event.review_rounds += 1;
                            let feedback = format!("审查打回，请返工：{reason}");
                            return Ok(if event.review_rounds > 3 {
                                Attempt::Blocked(feedback)
                            } else {
                                Attempt::Rework(feedback)
                            });
                        }
                    }
                    self.status(&mut event, "done", None)?;
                    Ok::<_, String>(Attempt::Done)
                })();
                if !matches!(attempt, Ok(Attempt::Done))
                    && let Some(git) = git
                    && let Err(error) = git.apply(&patch, true)
                {
                    halt.store(true, Ordering::SeqCst);
                    self.status(
                        &mut event,
                        "blocked",
                        Some(format!("无法撤回基地改动：{error}")),
                    )?;
                    return Ok(false);
                }
                match attempt? {
                    Attempt::Done => return Ok(true),
                    Attempt::Stopped => return Ok(false),
                    Attempt::Blocked(message) => {
                        halt.store(true, Ordering::SeqCst);
                        self.status(&mut event, "blocked", Some(message))?;
                        return Ok(false);
                    }
                    Attempt::Rework(message) => feedback = message,
                }
            }
        })();
        if self.runner.stopped() && !matches!(result, Ok(true)) {
            self.status(&mut event, "pending", Some("用户已停止执行".into()))?;
            return Ok(false);
        }
        if let Err(error) = &result {
            halt.store(true, Ordering::SeqCst);
            self.status(&mut event, "failed", Some(error.clone()))?;
        }
        result
    }
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
    conn: &'a Mutex<Connection>,
    camp: &'a str,
    slot: &'a RunSlot,
    window: &'a WebviewWindow,
}

impl Runner for LiveRunner<'_> {
    fn run(&self, role: &str, prompt: &str, cwd: &Path) -> Result<String, String> {
        let config = roles::run_config(&*database(self.conn)?, self.camp, role, 0)?;
        supervisor::dispatch(
            &config.cli,
            StartOptions {
                cwd,
                model: &config.model,
                system_prompt: &config.system_prompt,
                prompt,
                permissions: &config.permissions,
            },
            self.slot,
            &|_| Ok(()),
        )
    }
    fn check(&self, command: &CheckCommand, cwd: &Path) -> Result<(bool, String), String> {
        check(command, cwd, self.slot)
    }
    fn stopped(&self) -> bool {
        self.slot.is_stopped()
    }
    fn emit(&self, event: &TaskEvent) -> Result<(), String> {
        self.window
            .emit_to(self.window.label(), "plan-task", event)
            .map_err(|e| format!("无法发送计划卡状态事件：{e}"))
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
            crate::db::open(&data_dir).map_err(|e| format!("无法访问基地数据库：{e}"))?;
        conn.busy_timeout(std::time::Duration::from_secs(5))
            .map_err(db_error)?;
        let cwd = camps::run_root(&conn, &camp_id)?;
        let root = window.state::<artifacts::ArtifactRoot>();
        let plan = approve(&mut conn, &camp_id, &plan_id, &assignments)?;
        let conn = Mutex::new(conn);
        let runner = LiveRunner {
            conn: &conn,
            camp: &camp_id,
            slot: &slot,
            window: &window,
        };
        let result = Execution {
            conn: &conn,
            root: &root.0,
            cwd: &cwd,
            worktrees: &data_dir.join("worktrees"),
            camp: &camp_id,
            id: &plan_id,
            plan: &plan,
            runner: &runner,
        }
        .execute();
        let final_status = finish(
            &*database(&conn)?,
            &plan_id,
            result.is_ok() && !slot.is_stopped(),
        );
        drop(active);
        let _ = window.emit_to(window.label(), "plan-state", &camp_id);
        final_status?;
        result
    })
    .await
    .map_err(|e| format!("计划卡执行失败：{e}"))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Fake {
        failures: u32,
        rejects: u32,
        runs: Mutex<Vec<String>>,
        events: Mutex<Vec<TaskEvent>>,
        checks: Mutex<u32>,
        reviews: Mutex<u32>,
        outputs: Mutex<u32>,
        error: bool,
        stop: bool,
        invalid_review: bool,
        stop_on_done: bool,
        empty: u32,
        blocked: bool,
        blocked_review: bool,
    }
    impl Runner for Fake {
        fn run(&self, role: &str, prompt: &str, _: &Path) -> Result<String, String> {
            self.runs.lock().unwrap().push(role.into());
            if self.error {
                return Err("无法启动角色 CLI".into());
            }
            if role == "reviewer" {
                assert!(prompt.contains(REVIEW_VERDICT));
                if self.blocked_review {
                    return Ok("审查受阻。\nBLOCKED: 无法访问任务声明的 files 范围".into());
                }
                if self.invalid_review {
                    return Ok("not JSON".into());
                }
                let mut reviews = self.reviews.lock().unwrap();
                *reviews += 1;
                let verdict = if *reviews <= self.rejects {
                    "reject"
                } else {
                    "approve"
                };
                return Ok(format!(
                    "审查已完成。\n```json\n{}\n```",
                    serde_json::json!({"verdict": verdict, "reason": "审查说明"})
                ));
            }
            let mut outputs = self.outputs.lock().unwrap();
            *outputs += 1;
            if *outputs <= self.empty {
                return Ok(" \n".into());
            }
            if role == "backend" {
                assert!(prompt.contains("输入产物 repro.md：\n角色输出"));
            }
            if self.blocked {
                return Ok("分析完成。\nBLOCKED: 需要 git push 权限".into());
            }
            Ok("角色输出".into())
        }
        fn check(&self, _: &CheckCommand, _: &Path) -> Result<(bool, String), String> {
            let mut checks = self.checks.lock().unwrap();
            let passed = *checks >= self.failures;
            *checks += 1;
            Ok((passed, "检查日志".into()))
        }
        fn stopped(&self) -> bool {
            (self.stop && !self.runs.lock().unwrap().is_empty())
                || (self.stop_on_done
                    && self
                        .events
                        .lock()
                        .unwrap()
                        .iter()
                        .any(|event| event.status == "done"))
        }
        fn emit(&self, event: &TaskEvent) -> Result<(), String> {
            self.events.lock().unwrap().push(event.clone());
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
        let result = execute(&mut conn, &root, "camp", "plan", &plan, fake);
        assert_eq!(result.is_err(), fake.error);
        finish(&conn, "plan", result.is_ok() && !fake.stopped()).unwrap();
        if root.exists() {
            std::fs::remove_dir_all(root).unwrap();
        }
        conn
    }

    fn execute(
        conn: &mut Connection,
        root: &Path,
        camp: &str,
        id: &str,
        plan: &Plan,
        runner: &impl Runner,
    ) -> Result<(), String> {
        std::fs::create_dir_all(root).unwrap();
        let shared = Mutex::new(std::mem::replace(
            conn,
            Connection::open_in_memory().unwrap(),
        ));
        let result = Execution {
            conn: &shared,
            root,
            cwd: root,
            worktrees: &root.join("worktrees"),
            camp,
            id,
            plan,
            runner,
        }
        .execute();
        *conn = shared.into_inner().unwrap();
        result
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
                .lock()
                .unwrap()
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
        assert_eq!(fake.runs.lock().unwrap()[0], "debugger");
        assert_eq!(fake.runs.lock().unwrap()[2], "backend");
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
        execute(&mut conn, &root, "camp", "plan", &plan, &mut fake).unwrap();
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
    fn empty_outputs_fail_the_scheduler_check() {
        for (empty, expected, attempts) in [(1, "done", 1), (4, "blocked", 4)] {
            let mut fake = Fake {
                empty,
                ..Fake::default()
            };
            let conn = run(&mut fake);
            assert_eq!(state(&conn, "reproduce"), (expected.into(), attempts, 0));
            if expected == "blocked" {
                assert!(
                    fake.events
                        .lock()
                        .unwrap()
                        .last()
                        .unwrap()
                        .message
                        .as_deref()
                        .unwrap()
                        .contains(EMPTY_OUTPUT)
                );
                assert_eq!(state(&conn, "fix").0, "pending");
            }
        }
    }

    #[test]
    fn worker_blocked_report_blocks_the_task() {
        let mut fake = Fake {
            blocked: true,
            ..Fake::default()
        };
        let conn = run(&mut fake);
        assert_eq!(state(&conn, "reproduce"), ("blocked".into(), 0, 0));
        assert_eq!(state(&conn, "fix").0, "pending");
        assert_eq!(
            plans::latest(&conn, "camp").unwrap().unwrap().status,
            "aborted"
        );
        let events = fake.events.lock().unwrap();
        let blocked = events.iter().find(|event| event.status == "blocked").unwrap();
        assert_eq!(blocked.message.as_deref(), Some("需要 git push 权限"));
    }

    #[test]
    fn reviewer_blocked_report_blocks_the_task() {
        let mut fake = Fake {
            blocked_review: true,
            ..Fake::default()
        };
        let conn = run(&mut fake);
        assert_eq!(state(&conn, "reproduce"), ("blocked".into(), 0, 0));
        assert_eq!(state(&conn, "fix").0, "pending");
        let events = fake.events.lock().unwrap();
        let blocked = events.iter().find(|event| event.status == "blocked").unwrap();
        assert_eq!(
            blocked.message.as_deref(),
            Some("无法访问任务声明的 files 范围")
        );
    }

    #[test]
    fn non_coding_plan_runs_to_delivery() {
        let mut conn = roles::tests::connection();
        conn.execute("INSERT INTO plans (id,camp_id,scenario,user_request,plan_json,status,created_at) VALUES ('plan','camp','research','notes',?1,'draft',1)", [include_str!("../../tests/scheduler/captain-plan-docs.json")]).unwrap();
        let plan = prepare(&mut conn, "camp", "plan").unwrap();
        let root = std::env::current_dir()
            .unwrap()
            .join("target/scheduler-docs-test");
        let mut fake = Fake::default();
        execute(&mut conn, &root, "camp", "plan", &plan, &mut fake).unwrap();
        finish(&conn, "plan", true).unwrap();
        assert_eq!(
            *fake.runs.lock().unwrap(),
            ["captain", "reviewer", "reviewer"]
        );
        assert_eq!(state(&conn, "actions").0, "done");
        assert_eq!(state(&conn, "verify").0, "done");
        assert!(
            root.join("camp")
                .join("plan-actions")
                .join("action-items.md")
                .is_file()
        );
        assert_eq!(
            plans::latest(&conn, "camp").unwrap().unwrap().status,
            "awaiting_delivery"
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
    fn approval_rejects_invalid_file_scopes_before_dispatch() {
        let mut conn = connection("draft");
        let json: String = conn
            .query_row("SELECT plan_json FROM plans", [], |row| row.get(0))
            .unwrap();
        let mut plan: Plan = serde_json::from_str(&json).unwrap();
        plan.tasks[0].files = vec!["../escape.txt".into()];
        conn.execute(
            "UPDATE plans SET plan_json=?1",
            [serde_json::to_string(&plan).unwrap()],
        )
        .unwrap();
        let error = prepare(&mut conn, "camp", "plan").err().unwrap();
        assert!(error.contains("写入范围"));
        assert_eq!(
            plans::latest(&conn, "camp").unwrap().unwrap().status,
            "draft"
        );
        assert_eq!(
            conn.query_row("SELECT count(*) FROM tasks", [], |r| { r.get::<_, u32>(0) })
                .unwrap(),
            0
        );
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
        execute(&mut conn, &root, "camp", "plan", &plan, &mut fake).unwrap();
        assert_eq!(*fake.runs.lock().unwrap(), ["reviewer"]);
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
                .lock()
                .unwrap()
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

            execute(&mut conn, &root, "camp", "plan", &plan, &mut fake).unwrap();
            finish(&conn, "plan", true).unwrap();

            assert_eq!(*fake.runs.lock().unwrap(), ["reviewer"]);
            assert_eq!(state(&conn, "reproduce"), ("blocked".into(), 0, 1));
            assert_eq!(state(&conn, "fix").0, "pending");
            assert_eq!(
                plans::latest(&conn, "camp").unwrap().unwrap().status,
                "aborted"
            );
            assert_eq!(
                fake.events
                    .lock()
                    .unwrap()
                    .last()
                    .unwrap()
                    .message
                    .as_deref(),
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
    fn parallel_tasks_integrate_and_dependent_sees_both() {
        parallel_case(3, true, 2, false, false);
    }

    #[test]
    fn parallelism_is_capped_at_three() {
        parallel_case(7, true, 3, false, false);
    }

    #[test]
    fn block_stops_new_work_but_inflight_sibling_finishes() {
        parallel_case(3, true, 2, true, false);
    }

    #[test]
    fn conflicting_patch_is_blocked_and_retained() {
        parallel_case(2, true, 2, false, true);
    }

    #[test]
    fn overlapping_file_scopes_serialize_while_disjoint_scopes_parallelize() {
        use crate::git::tests::{cleanup, directory, repo};
        let root = directory("scope-root");
        let data = directory("scope-data");
        repo(&root);
        let mut conn = connection("draft");
        let mut plan: Plan =
            serde_json::from_str(include_str!("../../tests/scheduler/captain-plan.json")).unwrap();
        let template = serde_json::to_string(&plan.tasks[0]).unwrap();
        plan.tasks.clear();
        for (index, files) in [
            vec!["src".to_string()],
            vec!["src/app.ts".to_string()],
            vec!["docs/guide.md".to_string()],
            vec!["tests/app.test.ts".to_string()],
        ]
        .into_iter()
        .enumerate()
        {
            let mut task: plans::Task = serde_json::from_str(&template).unwrap();
            task.id = format!("t{index}");
            task.title = format!("T{index}");
            task.role_id = "debugger".into();
            task.depends_on = vec![];
            task.input_artifacts.clear();
            task.output_artifacts = vec!["output.md".into()];
            task.check_command = None;
            task.files = files;
            plan.tasks.push(task);
        }
        conn.execute(
            "UPDATE plans SET plan_json=?1",
            [serde_json::to_string(&plan).unwrap()],
        )
        .unwrap();
        let plan = prepare(&mut conn, "camp", "plan").unwrap();
        let conn = Mutex::new(conn);
        let runner = ScopeRunner {
            state: Mutex::default(),
            changed: std::sync::Condvar::new(),
            width: 3,
            root: root.clone(),
        };
        Execution {
            conn: &conn,
            root: &data.join("artifacts"),
            cwd: &root,
            worktrees: &data.join("worktrees"),
            camp: "camp",
            id: "plan",
            plan: &plan,
            runner: &runner,
        }
        .execute()
        .unwrap();
        let conn = conn.into_inner().unwrap();
        let recorded = runner.state.lock().unwrap();
        assert_eq!(recorded.peak, 3);
        for index in 0..4 {
            assert_eq!(state(&conn, &format!("t{index}")).0, "done");
        }
        let interval = |index| {
            recorded
                .intervals
                .iter()
                .find(|(task, _, _)| *task == index)
                .map(|(_, start, end)| (*start, *end))
                .unwrap()
        };
        let (_, overlap_end) = interval(0);
        let (nested_start, _) = interval(1);
        assert!(nested_start > overlap_end);
        let (disjoint_start, _) = interval(2);
        assert!(disjoint_start < overlap_end);
        cleanup(&root);
        cleanup(&data);
    }

    #[test]
    fn file_scope_overlap_uses_path_segments() {
        let scope = |paths: &[&str]| {
            paths
                .iter()
                .map(|path| path.to_string())
                .collect::<Vec<_>>()
        };
        assert!(scopes_overlap(&scope(&["src"]), &scope(&["src/app.ts"])));
        assert!(scopes_overlap(&scope(&["src/app.ts"]), &scope(&["src"])));
        assert!(scopes_overlap(
            &scope(&["src/app.ts"]),
            &scope(&["src/app.ts"])
        ));
        assert!(!scopes_overlap(&scope(&["src"]), &scope(&["src2/app.ts"])));
        assert!(!scopes_overlap(
            &scope(&["src/a.ts"]),
            &scope(&["src/b.ts"])
        ));
        assert!(!scopes_overlap(&scope(&[]), &scope(&["src"])));
    }

    #[test]
    fn non_git_tasks_are_sequential_in_camp_root() {
        parallel_case(3, false, 1, false, false);
    }

    #[derive(Default)]
    struct ScopeState {
        active: usize,
        peak: usize,
        entered: usize,
        order: usize,
        intervals: Vec<(usize, usize, usize)>,
    }

    struct ScopeRunner {
        state: Mutex<ScopeState>,
        changed: std::sync::Condvar,
        width: usize,
        root: std::path::PathBuf,
    }

    impl Runner for ScopeRunner {
        fn run(&self, role: &str, prompt: &str, cwd: &Path) -> Result<String, String> {
            if role == "reviewer" {
                return Ok(r#"{"verdict":"approve","reason":"ok"}"#.into());
            }
            let index = prompt
                .lines()
                .next()
                .unwrap()
                .strip_prefix('T')
                .unwrap()
                .parse::<usize>()
                .unwrap();
            let start = {
                let mut state = self.state.lock().unwrap();
                state.active += 1;
                state.peak = state.peak.max(state.active);
                state.entered += 1;
                state.order += 1;
                let order = state.order;
                state.intervals.push((index, order, 0));
                self.changed.notify_all();
                order
            };
            let (state, timeout) = self
                .changed
                .wait_timeout_while(
                    self.state.lock().unwrap(),
                    std::time::Duration::from_secs(10),
                    |state| state.entered < self.width,
                )
                .unwrap();
            assert!(!timeout.timed_out(), "disjoint scopes did not overlap");
            drop(state);
            std::thread::sleep(std::time::Duration::from_millis(30));
            std::fs::write(
                cwd.join(format!("file-{index}.txt")),
                format!("task {index}\n"),
            )
            .unwrap();
            let mut state = self.state.lock().unwrap();
            state.active -= 1;
            state.order += 1;
            let end = state.order;
            if let Some((_, _, slot)) = state
                .intervals
                .iter_mut()
                .find(|(task, start_order, _)| *task == index && *start_order == start)
            {
                *slot = end;
            }
            self.changed.notify_all();
            Ok("output".into())
        }

        fn check(&self, _: &CheckCommand, cwd: &Path) -> Result<(bool, String), String> {
            assert_eq!(cwd, self.root);
            Ok((true, "checked".into()))
        }

        fn stopped(&self) -> bool {
            false
        }

        fn emit(&self, _: &TaskEvent) -> Result<(), String> {
            Ok(())
        }
    }

    #[derive(Default)]
    struct ParallelState {
        active: usize,
        peak: usize,
        entered: usize,
        done: HashSet<String>,
        blocked: bool,
        paths: Vec<std::path::PathBuf>,
        started: Vec<usize>,
        checks: usize,
    }

    struct ParallelRunner {
        state: Mutex<ParallelState>,
        changed: std::sync::Condvar,
        width: usize,
        root: std::path::PathBuf,
        isolated: bool,
        block: bool,
        conflict: bool,
        dependent: bool,
    }

    impl Runner for ParallelRunner {
        fn run(&self, role: &str, prompt: &str, cwd: &Path) -> Result<String, String> {
            if role == "reviewer" {
                assert!(
                    self.state
                        .lock()
                        .unwrap()
                        .paths
                        .iter()
                        .any(|path| path == cwd)
                );
                return Ok(r#"{"verdict":"approve","reason":"ok"}"#.into());
            }
            let index = prompt
                .lines()
                .next()
                .unwrap()
                .strip_prefix('T')
                .unwrap()
                .parse::<usize>()
                .unwrap();
            let mut state = self.state.lock().unwrap();
            if self.dependent && index == 2 {
                if self.block {
                    panic!("blocked plan started another task");
                }
                assert!(state.done.contains("plan-t0") && state.done.contains("plan-t1"));
                assert!(cwd.join("file-0.txt").is_file() && cwd.join("file-1.txt").is_file());
            }
            state.started.push(index);
            state.paths.push(cwd.to_path_buf());
            if self.isolated {
                assert!(cwd.ends_with(Path::new("camp").join(format!("plan-t{index}"))));
            } else {
                assert_eq!(cwd, self.root);
            }
            state.active += 1;
            state.peak = state.peak.max(state.active);
            state.entered += 1;
            self.changed.notify_all();
            let (mut state, timeout) = self
                .changed
                .wait_timeout_while(state, std::time::Duration::from_secs(10), |state| {
                    state.entered < self.width
                })
                .unwrap();
            assert!(!timeout.timed_out(), "independent tasks did not overlap");
            if self.block && index == 1 {
                let (next, timeout) = self
                    .changed
                    .wait_timeout_while(state, std::time::Duration::from_secs(10), |state| {
                        !state.blocked
                    })
                    .unwrap();
                assert!(!timeout.timed_out(), "sibling never blocked");
                state = next;
            }
            drop(state);
            std::thread::sleep(std::time::Duration::from_millis(30));
            if !(self.block && index == 0) {
                let file = if self.conflict {
                    "base.txt".into()
                } else {
                    format!("file-{index}.txt")
                };
                std::fs::write(cwd.join(file), format!("task {index}\n")).unwrap();
            }
            self.state.lock().unwrap().active -= 1;
            Ok(if self.block && index == 0 {
                "".into()
            } else {
                "output".into()
            })
        }

        fn check(&self, _: &CheckCommand, cwd: &Path) -> Result<(bool, String), String> {
            assert_eq!(cwd, self.root);
            self.state.lock().unwrap().checks += 1;
            Ok((true, "checked in root".into()))
        }

        fn stopped(&self) -> bool {
            false
        }

        fn emit(&self, event: &TaskEvent) -> Result<(), String> {
            let mut state = self.state.lock().unwrap();
            if event.status == "done" {
                state.done.insert(event.task_id.clone());
            }
            if event.status == "blocked" {
                state.blocked = true;
            }
            self.changed.notify_all();
            Ok(())
        }
    }

    fn parallel_case(count: usize, git: bool, width: usize, block: bool, conflict: bool) {
        use crate::git::tests::{directory, repo};
        let root = directory("parallel-root");
        let data = directory("parallel-data");
        if git {
            repo(&root);
        }
        let mut conn = connection("draft");
        let mut plan: Plan =
            serde_json::from_str(include_str!("../../tests/scheduler/captain-plan.json")).unwrap();
        let template = serde_json::to_string(&plan.tasks[0]).unwrap();
        plan.tasks.clear();
        let dependent = count == 3;
        for i in 0..count {
            let mut task: plans::Task = serde_json::from_str(&template).unwrap();
            task.id = format!("t{i}");
            task.title = format!("T{i}");
            task.role_id = "debugger".into();
            task.depends_on = if dependent && i == 2 {
                if block {
                    vec!["t1".into()]
                } else {
                    vec!["t0".into(), "t1".into()]
                }
            } else {
                vec![]
            };
            task.input_artifacts.clear();
            task.output_artifacts = vec!["output.md".into()];
            task.check_command = Some(CheckCommand {
                program: "fake".into(),
                args: vec![],
            });
            plan.tasks.push(task);
        }
        conn.execute(
            "UPDATE plans SET plan_json=?1",
            [serde_json::to_string(&plan).unwrap()],
        )
        .unwrap();
        let plan = prepare(&mut conn, "camp", "plan").unwrap();
        let conn = Mutex::new(conn);
        let runner = ParallelRunner {
            state: Mutex::default(),
            changed: std::sync::Condvar::new(),
            width,
            root: root.clone(),
            isolated: git,
            block,
            conflict,
            dependent,
        };
        Execution {
            conn: &conn,
            root: &data.join("artifacts"),
            cwd: &root,
            worktrees: &data.join("worktrees"),
            camp: "camp",
            id: "plan",
            plan: &plan,
            runner: &runner,
        }
        .execute()
        .unwrap();
        let conn = conn.into_inner().unwrap();
        let recorded = runner.state.lock().unwrap();
        assert_eq!(recorded.peak, width);
        for path in &recorded.paths {
            if git {
                assert!(path.starts_with(data.join("worktrees")));
                assert!(!path.exists());
            }
        }
        if block {
            assert_eq!(state(&conn, "t0").0, "blocked");
            assert_eq!(state(&conn, "t1").0, "done");
            assert_eq!(state(&conn, "t2").0, "pending");
            assert!(!recorded.started.contains(&2));
        } else if conflict {
            let states: Vec<_> = (0..2).map(|i| state(&conn, &format!("t{i}")).0).collect();
            assert_eq!(states.iter().filter(|s| s.as_str() == "done").count(), 1);
            assert_eq!(states.iter().filter(|s| s.as_str() == "blocked").count(), 1);
            let blocked = states.iter().position(|s| s == "blocked").unwrap();
            let patch = data
                .join("artifacts")
                .join("camp")
                .join(format!("plan-t{blocked}"))
                .join("changes.diff");
            assert!(std::fs::read_to_string(patch).unwrap().contains("base.txt"));
            let message: String = conn
                .query_row(
                    "SELECT content FROM messages WHERE task_id=?1",
                    [format!("plan-t{blocked}")],
                    |r| r.get(0),
                )
                .unwrap();
            assert!(message.contains("无法把改动并入基地"));
        } else {
            for i in 0..count {
                assert_eq!(state(&conn, &format!("t{i}")).0, "done");
                assert_eq!(
                    std::fs::read_to_string(root.join(format!("file-{i}.txt"))).unwrap(),
                    format!("task {i}\n")
                );
            }
            assert_eq!(recorded.checks, count);
        }
        drop(recorded);
        crate::git::tests::cleanup(&root);
        crate::git::tests::cleanup(&data);
    }

    #[test]
    fn isolated_reworks_rollback_and_preserve_worktree_changes() {
        for mode in ["check", "review", "error", "stop", "revert"] {
            let root = crate::git::tests::directory("rework-root");
            let data = crate::git::tests::directory("rework-data");
            crate::git::tests::repo(&root);
            let mut conn = connection("draft");
            let mut plan: Plan =
                serde_json::from_str(include_str!("../../tests/scheduler/captain-plan.json"))
                    .unwrap();
            plan.tasks.truncate(1);
            let task = &mut plan.tasks[0];
            task.id = "single".into();
            task.depends_on.clear();
            task.input_artifacts.clear();
            task.role_id = "debugger".into();
            task.check_command = Some(CheckCommand {
                program: "fake".into(),
                args: vec![],
            });
            conn.execute(
                "UPDATE plans SET plan_json=?1",
                [serde_json::to_string(&plan).unwrap()],
            )
            .unwrap();
            let plan = prepare(&mut conn, "camp", "plan").unwrap();
            let conn = Mutex::new(conn);
            let runner = ReworkRunner {
                root: &root,
                mode,
                attempts: Mutex::new(0),
                stopped: AtomicBool::new(false),
            };
            let result = Execution {
                conn: &conn,
                root: &data.join("artifacts"),
                cwd: &root,
                worktrees: &data.join("worktrees"),
                camp: "camp",
                id: "plan",
                plan: &plan,
                runner: &runner,
            }
            .execute();
            assert_eq!(result.is_err(), mode == "error");
            let expected = match mode {
                "error" => "failed",
                "stop" => "pending",
                "revert" => "blocked",
                _ => "done",
            };
            assert_eq!(state(&conn.lock().unwrap(), "single").0, expected);
            let content = std::fs::read_to_string(root.join("base.txt")).unwrap();
            assert_eq!(
                content,
                match mode {
                    "error" | "stop" => "original\n",
                    "revert" => "external edit\n",
                    _ => "attempt 2\n",
                }
            );
            assert!(
                data.join("artifacts/camp/plan-single/changes.diff")
                    .is_file()
            );
            assert!(!data.join("worktrees/camp/plan-single").exists());
            crate::git::tests::cleanup(&root);
            crate::git::tests::cleanup(&data);
        }
    }

    struct ReworkRunner<'a> {
        root: &'a Path,
        mode: &'a str,
        attempts: Mutex<u32>,
        stopped: AtomicBool,
    }

    impl Runner for ReworkRunner<'_> {
        fn run(&self, role: &str, _: &str, cwd: &Path) -> Result<String, String> {
            let mut attempts = self.attempts.lock().unwrap();
            if role == "reviewer" {
                let verdict = if self.mode == "review" && *attempts == 1 {
                    "reject"
                } else {
                    "approve"
                };
                return Ok(format!(r#"{{"verdict":"{verdict}","reason":"review"}}"#));
            }
            assert_eq!(
                std::fs::read_to_string(self.root.join("base.txt")).unwrap(),
                "original\n"
            );
            if *attempts > 0 {
                assert_eq!(
                    std::fs::read_to_string(cwd.join("base.txt")).unwrap(),
                    format!("attempt {}\n", *attempts)
                );
            }
            *attempts += 1;
            std::fs::write(cwd.join("base.txt"), format!("attempt {}\n", *attempts)).unwrap();
            Ok("output".into())
        }
        fn check(&self, _: &CheckCommand, cwd: &Path) -> Result<(bool, String), String> {
            assert_eq!(cwd, self.root);
            assert_eq!(
                std::fs::read_to_string(cwd.join("base.txt")).unwrap(),
                format!("attempt {}\n", *self.attempts.lock().unwrap())
            );
            match self.mode {
                "error" => Err("check error".into()),
                "stop" => {
                    self.stopped.store(true, Ordering::SeqCst);
                    Ok((true, String::new()))
                }
                "revert" => {
                    std::fs::write(cwd.join("base.txt"), "external edit\n").unwrap();
                    Ok((false, "check failed".into()))
                }
                "check" if *self.attempts.lock().unwrap() == 1 => {
                    Ok((false, "check failed".into()))
                }
                _ => Ok((true, "check passed".into())),
            }
        }
        fn stopped(&self) -> bool {
            self.stopped.load(Ordering::SeqCst)
        }
        fn emit(&self, _: &TaskEvent) -> Result<(), String> {
            Ok(())
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

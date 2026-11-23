use std::sync::{
    Mutex, TryLockError,
    atomic::{AtomicU64, Ordering},
};
use std::{
    collections::HashMap,
    process::Child,
    thread::{self, ThreadId},
};

use rusqlite::Connection;
use serde::Serialize;
use tauri::{Emitter, Manager, WebviewWindow};

use crate::{
    adapters::{RunOutput, claude_code, codex},
    artifacts, camps, notifications, plans, process_tree, roles, run_events,
};

const ROLE_RUN_EVENT: &str = "role-run";
pub(crate) fn dispatch(
    cli: &str,
    options: claude_code::StartOptions<'_>,
    state: &RunSlot,
    emit: &impl Fn(RunUpdate) -> Result<(), String>,
) -> Result<RunOutput, String> {
    match cli {
        "claude-code" => claude_code::start(options, state, emit),
        "codex" => codex::start(options, state, emit),
        _ => Err(format!("不支持的 CLI：{cli}")),
    }
}

static NEXT_RUN_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Default)]
pub(crate) struct RunSlot {
    active: Mutex<()>,
    pub child: Mutex<HashMap<ThreadId, Child>>,
    stopped: std::sync::atomic::AtomicBool,
}

impl RunSlot {
    pub(crate) fn acquire(&self) -> Result<std::sync::MutexGuard<'_, ()>, String> {
        let guard = self.active.try_lock().map_err(|error| match error {
            TryLockError::WouldBlock => "已有角色正在运行",
            TryLockError::Poisoned(_) => "无法访问角色运行状态",
        })?;
        self.stopped.store(false, Ordering::SeqCst);
        Ok(guard)
    }

    pub(crate) fn is_stopped(&self) -> bool {
        self.stopped.load(Ordering::SeqCst)
    }

    pub fn attach(&self, process: Child) -> Result<(), String> {
        let mut child = self.child.lock().map_err(|_| "无法访问角色进程")?;
        if self.stopped.load(Ordering::SeqCst) {
            process_tree::terminate(&[process.id()]);
        }
        child.insert(thread::current().id(), process);
        Ok(())
    }

    pub fn wait(&self) -> Result<std::process::ExitStatus, String> {
        loop {
            {
                let mut child = self.child.lock().map_err(|_| "无法访问角色进程")?;
                if let Some(status) = child
                    .get_mut(&thread::current().id())
                    .ok_or("角色进程不存在")?
                    .try_wait()
                    .map_err(|e| e.to_string())?
                {
                    child.remove(&thread::current().id());
                    return Ok(status);
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }

    pub fn stop(&self) -> Result<(), String> {
        let mut pids = Vec::new();
        let mut running = false;
        let mut error = None;
        {
            let mut child = self.child.lock().map_err(|_| "无法访问角色进程")?;
            let active = self.active.try_lock().is_err();
            for child in child.values_mut() {
                match child.try_wait() {
                    Ok(Some(_)) => (),
                    Ok(None) => {
                        running = true;
                        pids.push(child.id());
                    }
                    Err(e) => error = Some(e.to_string()),
                }
            }
            if running || active {
                self.stopped.store(true, Ordering::SeqCst);
            }
        }
        process_tree::terminate(&pids);
        error.map_or(Ok(()), Err)
    }

    pub fn terminate(&self) {
        let mut pids = Vec::new();
        if let Ok(mut children) = self.child.lock() {
            for child in children.values_mut() {
                if !matches!(child.try_wait(), Ok(Some(_))) {
                    pids.push(child.id());
                }
            }
        }
        process_tree::terminate(&pids);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_stop_is_noop() {
        let slot = RunSlot::default();
        slot.stop().unwrap();
        assert!(!slot.stopped.load(Ordering::SeqCst));
    }

    #[test]
    fn stop_kills_children_on_two_threads() {
        let slot = RunSlot::default();
        let _active = slot.acquire().unwrap();
        let ready = std::sync::Barrier::new(3);
        let stopped = std::sync::Barrier::new(3);
        std::thread::scope(|scope| {
            for _ in 0..2 {
                let slot = &slot;
                let ready = &ready;
                let stopped = &stopped;
                scope.spawn(move || {
                    #[cfg(unix)]
                    let child = {
                        let mut command = std::process::Command::new("sleep");
                        command.arg("30");
                        process_tree::spawn(&mut command).unwrap()
                    };
                    #[cfg(windows)]
                    let child = {
                        let mut command = std::process::Command::new("ping");
                        command
                            .args(["-n", "30", "127.0.0.1"])
                            .stdout(std::process::Stdio::null());
                        process_tree::spawn(&mut command).unwrap()
                    };
                    slot.attach(child).unwrap();
                    ready.wait();
                    stopped.wait();
                    assert!(!slot.wait().unwrap().success());
                });
            }
            ready.wait();
            assert_eq!(slot.child.lock().unwrap().len(), 2);
            slot.stop().unwrap();
            stopped.wait();
        });
        assert!(slot.child.lock().unwrap().is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn stop_kills_child_and_covers_stop_before_spawn() {
        for early in [false, true] {
            let slot = RunSlot::default();
            let _active = slot.active.lock().unwrap();
            if early {
                slot.stop().unwrap();
            }
            let child = {
                let mut command = std::process::Command::new("sleep");
                command.arg("30");
                process_tree::spawn(&mut command).unwrap()
            };
            slot.attach(child).unwrap();
            if !early {
                slot.stop().unwrap();
            }
            assert!(!slot.wait().unwrap().success());
            assert!(slot.stopped.load(Ordering::SeqCst));
            assert!(slot.child.lock().unwrap().is_empty());
        }
    }
}

#[tauri::command]
pub fn stop_role_run(state: tauri::State<'_, RunSlot>) -> Result<(), String> {
    state.stop()
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum RunUpdate {
    Started,
    Text { text: String },
    Tool { name: String },
    Waiting,
    UsageLimited { resets_at: Option<u64> },
    TurnMetrics {
        input_tokens: u64,
        output_tokens: u64,
        cache_tokens: Option<u64>,
    },
    Completed,
    Blocked { message: String },
    Stopped,
    Failed { message: String },
}

#[derive(Clone, Serialize)]
pub(crate) struct RoleRunEvent<'a> {
    pub(crate) run_id: String,
    pub(crate) task_id: &'a str,
    pub(crate) artifact: Option<artifacts::Artifact>,
    pub(crate) camp_id: &'a str,
    pub(crate) role_id: &'a str,
    #[serde(flatten)]
    pub(crate) update: RunUpdate,
}

#[derive(Serialize)]
pub(crate) struct ClarificationQuestions {
    pub(crate) questions: Vec<String>,
    pub(crate) pending_id: String,
}

#[tauri::command]
pub async fn start_role_run(
    window: WebviewWindow,
    camp_id: String,
    role_id: String,
    loadout_index: usize,
    prompt: String,
    clarification: Option<plans::Clarification>,
    pending_answer: Option<run_events::PendingAnswerRequest>,
) -> Result<Option<ClarificationQuestions>, String> {
    if prompt.trim().is_empty() {
        return Err("任务内容不能为空".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let (camp_id, mut role_id, mut loadout_index, mut prompt, mut clarification) =
            (camp_id, role_id, loadout_index, prompt, clarification);
        if let Some(pending) = pending_answer {
            let answer = clarification.as_ref().and_then(|item| item.answer.clone());
            let state = window.state::<Mutex<Connection>>();
            let connection = state.lock().map_err(|_| "无法访问基地数据库")?;
            match run_events::claim(&connection, &camp_id, &pending.id, &pending.key, answer.as_deref())? {
                Some(pending) => {
                    role_id = pending.role_id;
                    loadout_index = pending.loadout_index;
                    prompt = pending.request;
                    clarification = Some(plans::Clarification {
                        questions: pending.questions,
                        answer,
                    });
                }
                None => return Ok(None),
            }
        }
        let state = window.state::<RunSlot>();
        let _active = state.acquire()?;
        let (cwd, mut config, roster) = {
            let state = window.state::<Mutex<Connection>>();
            let connection = state.lock().map_err(|_| "无法访问基地数据库")?;
            let cwd = camps::run_root(&connection, &camp_id)?;
            let config = roles::run_config(&connection, &camp_id, &role_id, loadout_index)?;
            let roster = config
                .is_captain
                .then(|| roles::planning_roster(&connection, &camp_id))
                .transpose()?;
            (cwd, config, roster)
        };
        let input = if let Some(roster) = &roster {
            config.system_prompt.push_str(plans::INSTRUCTIONS);
            Some(plans::input(&prompt, roster, clarification.as_ref())?)
        } else if clarification.is_some() {
            return Err("只有队长可以继续澄清任务".into());
        } else {
            None
        };
        let run_id = NEXT_RUN_ID.fetch_add(1, Ordering::Relaxed);
        let millis = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| format!("无法读取系统时间：{e}"))?
            .as_millis() as i64;
        let prefix = if config.is_captain { "plan" } else { "run" };
        let task_id = format!("{prefix}-{millis}-{run_id}");
        let output = std::cell::RefCell::new(String::new());
        let artifact = std::cell::RefCell::new(None);
        let emit = |update| {
            if let RunUpdate::Text { ref text } = update {
                if config.is_captain {
                    return Ok(());
                }
                output.borrow_mut().push_str(text);
            }
            let event = RoleRunEvent {
                run_id: task_id.clone(),
                task_id: &task_id,
                artifact: artifact.borrow().clone(),
                camp_id: &camp_id,
                role_id: &role_id,
                update,
            };
            let payload = serde_json::to_value(&event)
                .map_err(|error| format!("无法编码角色运行事件：{error}"))?;
            let kind = payload
                .get("type")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("event")
                .to_string();
            {
                let state = window.state::<Mutex<Connection>>();
                let connection = state.lock().map_err(|_| "无法访问运行事件数据库")?;
                run_events::append(&connection, &task_id, &task_id, &kind, payload.clone())?;
            }
            window
                .emit_to(window.label(), ROLE_RUN_EVENT, payload)
                .map_err(|error| format!("无法发送角色运行事件：{error}"))
        };
        let options = claude_code::StartOptions {
            cwd: &cwd,
            model: &config.model,
            system_prompt: &config.system_prompt,
            prompt: input.as_deref().unwrap_or(&prompt),
            permissions: &config.permissions,
            resume: None,
            persist_session: false,
        };
        let result = dispatch(&config.cli, options, &state, &emit);
        if state.stopped.load(Ordering::SeqCst) {
            emit(RunUpdate::Stopped)?;
            return Ok(None);
        }
        enum Outcome {
            Done,
            Waiting { questions: Vec<String>, pending_id: String },
            Blocked(String),
        }
        let result = result.and_then(|run| {
            let final_text = run.text;
            if let Some(reason) = roles::blocked_reason(&final_text) {
                return Ok(Outcome::Blocked(reason));
            }
            let plan = if let Some(roster) = &roster {
                match plans::parse(&final_text, roster, clarification.as_ref())? {
                    plans::Reply::Draft(plan) => Some(plan),
                    plans::Reply::Clarify(reply) => {
                        let state = window.state::<Mutex<Connection>>();
                        let connection = state.lock().map_err(|_| "无法访问待答数据库")?;
                        let pending_id = run_events::create_pending(
                            &connection,
                            run_events::PendingQuestionInput {
                                camp_id: &camp_id,
                                run_id: &task_id,
                                task_id: &task_id,
                                role_id: &role_id,
                                loadout_index,
                                request: &prompt,
                                questions: &reply.questions,
                            },
                        )?;
                        return Ok(Outcome::Waiting {
                            questions: reply.questions,
                            pending_id,
                        });
                    }
                }
            } else {
                None
            };
            let connection = window.state::<Mutex<Connection>>();
            let mut connection = connection.lock().map_err(|_| "无法访问产物数据库")?;
            let root = window.state::<artifacts::ArtifactRoot>();
            *artifact.borrow_mut() = Some(if let Some(plan) = plan {
                plan.save(
                    &mut connection,
                    &root.0,
                    &camp_id,
                    &task_id,
                    &prompt,
                    millis,
                )?
            } else {
                artifacts::write_artifact(
                    &connection,
                    &root.0,
                    &camp_id,
                    &task_id,
                    "report",
                    std::path::Path::new(config.artifact_name),
                    &output.borrow(),
                )?
            });
            Ok(Outcome::Done)
        });
        match result {
            Ok(Outcome::Blocked(message)) => {
                emit(RunUpdate::Blocked { message })?;
                let state = window.state::<Mutex<Connection>>();
                if let Ok(connection) = state.lock() {
                    let _ = notifications::notify(
                        &window,
                        &connection,
                        &format!("role-blocked:{task_id}"),
                        notifications::Kind::Blocked,
                        &camp_id,
                        None,
                        Some(task_id.as_str()),
                    );
                }
                Ok(None)
            }
            Ok(Outcome::Done) => {
                emit(RunUpdate::Completed)?;
                Ok(None)
            }
            Ok(Outcome::Waiting { questions, pending_id }) => {
                emit(RunUpdate::Waiting)?;
                let state = window.state::<Mutex<Connection>>();
                if let Ok(connection) = state.lock() {
                    let _ = notifications::notify(
                        &window,
                        &connection,
                        &format!("needs-answer:{pending_id}"),
                        notifications::Kind::NeedsAnswer,
                        &camp_id,
                        None,
                        Some(task_id.as_str()),
                    );
                }
                Ok(Some(ClarificationQuestions { questions, pending_id }))
            }
            Err(message) => {
                let _ = emit(RunUpdate::Failed {
                    message: message.clone(),
                });
                let state = window.state::<Mutex<Connection>>();
                if let Ok(connection) = state.lock() {
                    let _ = notifications::notify(
                        &window,
                        &connection,
                        &format!("role-failed:{task_id}"),
                        notifications::Kind::Failed,
                        &camp_id,
                        None,
                        Some(task_id.as_str()),
                    );
                }
                Err(message)
            }
        }
    })
    .await
    .map_err(|error| format!("角色运行失败：{error}"))?
}

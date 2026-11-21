use std::process::Child;
use std::sync::{
    Mutex, TryLockError,
    atomic::{AtomicU64, Ordering},
};

use rusqlite::Connection;
use serde::Serialize;
use tauri::{Emitter, Manager, WebviewWindow};

use crate::{
    adapters::{claude_code, codex},
    artifacts, camps, plans, roles,
};

const ROLE_RUN_EVENT: &str = "role-run";
static NEXT_RUN_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Default)]
pub(crate) struct RunSlot {
    active: Mutex<()>,
    pub child: Mutex<Option<Child>>,
    stopped: std::sync::atomic::AtomicBool,
}

impl RunSlot {
    pub fn attach(&self, mut process: Child) -> Result<(), String> {
        let mut child = self.child.lock().map_err(|_| "无法访问角色进程")?;
        if self.stopped.load(Ordering::SeqCst) {
            let _ = process.kill();
        }
        *child = Some(process);
        Ok(())
    }

    pub fn wait(&self) -> Result<std::process::ExitStatus, String> {
        loop {
            {
                let mut child = self.child.lock().map_err(|_| "无法访问角色进程")?;
                if let Some(status) = child
                    .as_mut()
                    .ok_or("角色进程不存在")?
                    .try_wait()
                    .map_err(|e| e.to_string())?
                {
                    child.take();
                    return Ok(status);
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }

    pub fn stop(&self) -> Result<(), String> {
        let mut child = self.child.lock().map_err(|_| "无法访问角色进程")?;
        if self.active.try_lock().is_err() {
            self.stopped.store(true, Ordering::SeqCst);
        }
        if let Some(child) = child.as_mut()
            && child.try_wait().map_err(|e| e.to_string())?.is_none()
        {
            child.kill().map_err(|e| format!("无法停止角色：{e}"))?;
            self.stopped.store(true, Ordering::SeqCst);
        }
        Ok(())
    }

    pub fn terminate(&self) {
        if let Ok(mut child) = self.child.lock()
            && let Some(child) = child.as_mut()
        {
            let _ = child.kill();
        }
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

    #[cfg(unix)]
    #[test]
    fn stop_kills_child_and_covers_stop_before_spawn() {
        for early in [false, true] {
            let slot = RunSlot::default();
            let _active = slot.active.lock().unwrap();
            if early {
                slot.stop().unwrap();
            }
            slot.attach(
                std::process::Command::new("sleep")
                    .arg("30")
                    .spawn()
                    .unwrap(),
            )
            .unwrap();
            if !early {
                slot.stop().unwrap();
            }
            assert!(!slot.wait().unwrap().success());
            assert!(slot.stopped.load(Ordering::SeqCst));
            assert!(slot.child.lock().unwrap().is_none());
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
    Completed,
    Stopped,
    Failed { message: String },
}

#[derive(Clone, Serialize)]
struct RoleRunEvent<'a> {
    run_id: u64,
    task_id: &'a str,
    artifact: Option<artifacts::Artifact>,
    camp_id: &'a str,
    role_id: &'a str,
    #[serde(flatten)]
    update: RunUpdate,
}

#[tauri::command]
pub async fn start_role_run(
    window: WebviewWindow,
    camp_id: String,
    role_id: String,
    loadout_index: usize,
    prompt: String,
    clarification: Option<plans::Clarification>,
) -> Result<Option<Vec<String>>, String> {
    if prompt.trim().is_empty() {
        return Err("任务内容不能为空".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let state = window.state::<RunSlot>();
        let _active = state.active.try_lock().map_err(|error| match error {
            TryLockError::WouldBlock => "已有角色正在运行",
            TryLockError::Poisoned(_) => "无法访问角色运行状态",
        })?;
        state.stopped.store(false, Ordering::SeqCst);
        let (cwd, mut config, roster) = {
            let state = window.state::<Mutex<Connection>>();
            let connection = state.lock().map_err(|_| "无法访问大营数据库")?;
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
            window
                .emit_to(
                    window.label(),
                    ROLE_RUN_EVENT,
                    RoleRunEvent {
                        run_id,
                        task_id: &task_id,
                        artifact: artifact.borrow().clone(),
                        camp_id: &camp_id,
                        role_id: &role_id,
                        update,
                    },
                )
                .map_err(|error| format!("无法发送角色运行事件：{error}"))
        };
        let options = claude_code::StartOptions {
            cwd: &cwd,
            model: &config.model,
            system_prompt: &config.system_prompt,
            prompt: input.as_deref().unwrap_or(&prompt),
            permissions: &config.permissions,
        };
        let result = match config.cli.as_str() {
            "claude-code" => claude_code::start(options, &state, &emit),
            "codex" => codex::start(options, &state, &emit),
            _ => Err(format!("不支持的 CLI：{}", config.cli)),
        };
        if state.stopped.load(Ordering::SeqCst) {
            emit(RunUpdate::Stopped)?;
            return Ok(None);
        }
        let result = result.and_then(|final_text| {
            let plan = if let Some(roster) = &roster {
                match plans::parse(&final_text, roster, clarification.as_ref())? {
                    plans::Reply::Draft(plan) => Some(plan),
                    plans::Reply::Clarify(reply) => return Ok(Some(reply.questions)),
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
            Ok(None)
        });
        match result {
            Ok(questions) => {
                emit(if questions.is_some() {
                    RunUpdate::Waiting
                } else {
                    RunUpdate::Completed
                })?;
                Ok(questions)
            }
            Err(message) => {
                let _ = emit(RunUpdate::Failed {
                    message: message.clone(),
                });
                Err(message)
            }
        }
    })
    .await
    .map_err(|error| format!("角色运行失败：{error}"))?
}

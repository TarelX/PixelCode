use std::sync::{
    Mutex, TryLockError,
    atomic::{AtomicU64, Ordering},
};

use rusqlite::Connection;
use serde::Serialize;
use tauri::{Emitter, Manager, WebviewWindow};

use crate::{
    adapters::{claude_code, codex},
    camps, roles,
};

const ROLE_RUN_EVENT: &str = "role-run";
static NEXT_RUN_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Default)]
pub(crate) struct RunSlot(Mutex<()>);

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum RunUpdate {
    Started,
    Text { text: String },
    Tool { name: String },
    Completed,
    Failed { message: String },
}

#[derive(Clone, Serialize)]
struct RoleRunEvent<'a> {
    run_id: u64,
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
) -> Result<(), String> {
    if prompt.trim().is_empty() {
        return Err("任务内容不能为空".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let state = window.state::<RunSlot>();
        let _active = state.0.try_lock().map_err(|error| match error {
            TryLockError::WouldBlock => "已有角色正在运行",
            TryLockError::Poisoned(_) => "无法访问角色运行状态",
        })?;
        let (cwd, cli, model, system_prompt, permissions) = {
            let state = window.state::<Mutex<Connection>>();
            let connection = state.lock().map_err(|_| "无法访问大营数据库")?;
            let cwd = camps::run_root(&connection, &camp_id)?;
            let (cli, model, system_prompt, permissions) =
                roles::run_config(&connection, &camp_id, &role_id, loadout_index)?;
            (cwd, cli, model, system_prompt, permissions)
        };
        let run_id = NEXT_RUN_ID.fetch_add(1, Ordering::Relaxed);
        let emit = |update| {
            window
                .emit_to(
                    window.label(),
                    ROLE_RUN_EVENT,
                    RoleRunEvent {
                        run_id,
                        camp_id: &camp_id,
                        role_id: &role_id,
                        update,
                    },
                )
                .map_err(|error| format!("无法发送角色运行事件：{error}"))
        };
        let options = claude_code::StartOptions {
            cwd: &cwd,
            model: &model,
            system_prompt: &system_prompt,
            prompt: &prompt,
            permissions: &permissions,
        };
        let result = match cli.as_str() {
            "claude-code" => claude_code::start(options, &emit),
            "codex" => codex::start(options, &emit),
            _ => Err(format!("不支持的 CLI：{cli}")),
        };
        match result {
            Ok(()) => emit(RunUpdate::Completed),
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

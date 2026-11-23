use crate::{
    adapters::claude_code::StartOptions,
    artifacts, camps, plans, roles,
    supervisor::{self, RunSlot, RunUpdate},
};
use rusqlite::{Connection, params};
use serde::Serialize;
use std::{
    collections::HashSet,
    path::Path,
    sync::{
        Mutex,
        atomic::{AtomicU64, Ordering},
    },
};
use tauri::{Emitter, Manager, WebviewWindow};

const ROUNDS: u32 = 2;
const DISCUSSION: &str = "\nDiscuss the user's request with the other roles in a read-only design discussion. Never edit files or implement. Reply concisely in the user's language, building on or challenging prior views. Your final answer must be plain text.";
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Serialize)]
struct Participant {
    id: String,
    name: String,
    role_type: String,
}
#[derive(Clone, Serialize)]
struct Turn {
    role_id: String,
    name: String,
    round: u32,
    text: String,
}
#[derive(Clone, Serialize)]
struct ConversationEvent<'a> {
    camp_id: &'a str,
    conversation_id: &'a str,
    #[serde(flatten)]
    turn: Option<Turn>,
    outcome: Option<&'a str>,
    message: Option<&'a str>,
}
trait Runner {
    fn run(&self, role: &str, config: &roles::RunConfig, prompt: &str) -> Result<String, String>;
    fn stopped(&self) -> bool;
    fn turn(&self, turn: &Turn) -> Result<(), String>;
}
fn participants(
    conn: &Connection,
    camp: &str,
    ids: &[String],
    request: &str,
) -> Result<(Vec<Participant>, String), String> {
    if request.trim().is_empty() {
        return Err("作战会议需求不能为空".into());
    }
    if !(2..=4).contains(&ids.len()) || ids.iter().collect::<HashSet<_>>().len() != ids.len() {
        return Err("作战会议须选择二至四名不同的伙伴".into());
    }
    let roster = roles::effective_roles(conn, Some(camp))?;
    let captain = roster
        .iter()
        .find(|r| r.role_type == "captain")
        .ok_or("基地中没有可汇总作战会议的队长")?
        .id
        .clone();
    let participants = ids
        .iter()
        .map(|id| {
            let role = roster
                .iter()
                .find(|r| &r.id == id)
                .ok_or_else(|| format!("基地中未找到作战会议伙伴：{id}"))?;
            Ok(Participant {
                id: id.clone(),
                name: role.name.clone(),
                role_type: role.role_type.clone(),
            })
        })
        .collect::<Result<_, String>>()?;
    Ok((participants, captain))
}
struct Session<'a> {
    conn: &'a Mutex<Connection>,
    root: &'a Path,
    camp: &'a str,
    id: &'a str,
    millis: i64,
}
impl Session<'_> {
    fn execute(&self, ids: &[String], request: &str, runner: &impl Runner) -> Result<bool, String> {
        let lock = || {
            self.conn
                .lock()
                .map_err(|_| "无法访问基地数据库".to_string())
        };
        let (participants, captain) = participants(&*lock()?, self.camp, ids, request)?;
        let mut transcript = Vec::new();
        for round in 1..=ROUNDS {
            for participant in &participants {
                if runner.stopped() {
                    return Ok(false);
                }
                let mut config = roles::run_config(&*lock()?, self.camp, &participant.id, 0)?;
                config.permissions.clamp_read_only();
                config.system_prompt.push_str(DISCUSSION);
                let prompt = serde_json::json!({ "request": request, "participants": participants, "round": round, "transcript": transcript }).to_string();
                let result = runner.run(&participant.id, &config, &prompt);
                if runner.stopped() {
                    return Ok(false);
                }
                let turn = Turn {
                    role_id: participant.id.clone(),
                    name: participant.name.clone(),
                    round,
                    text: result?,
                };
                lock()?.execute("INSERT INTO messages (id,task_id,role,type,content,payload_json,created_at) VALUES (?1,?2,'assistant','discussion',?3,?4,?5)",
                    params![format!("{}-{}", self.id, transcript.len()), self.id, turn.text, serde_json::json!({"role_id": turn.role_id, "round": round}).to_string(), self.millis])
                    .map_err(|e| format!("无法保存作战会议发言：{e}"))?;
                runner.turn(&turn)?;
                transcript.push(turn);
            }
        }
        if runner.stopped() {
            return Ok(false);
        }
        let (mut config, roster) = {
            let conn = lock()?;
            (
                roles::run_config(&conn, self.camp, &captain, 0)?,
                roles::planning_roster(&conn, self.camp)?,
            )
        };
        config.system_prompt.push_str(plans::INSTRUCTIONS);
        config.system_prompt.push_str(
            "\nThe discussion has settled all open questions. Return a draft plan, not questions.",
        );
        let summary = serde_json::json!({"request": request, "discussion": transcript}).to_string();
        let prompt = plans::input(&summary, &roster, None)?;
        let result = runner.run(&captain, &config, &prompt);
        if runner.stopped() {
            return Ok(false);
        }
        match plans::parse(&result?, &roster, None)? {
            plans::Reply::Draft(plan) => {
                plan.save(
                    &mut *lock()?,
                    self.root,
                    self.camp,
                    self.id,
                    request,
                    self.millis,
                )?;
            }
            plans::Reply::Clarify(reply) => {
                return Err(format!(
                    "队长仍有待澄清的问题：{}",
                    reply.questions.join("；")
                ));
            }
        }
        Ok(true)
    }
}

struct LiveRunner<'a> {
    window: &'a WebviewWindow,
    slot: &'a RunSlot,
    cwd: &'a Path,
    camp: &'a str,
    id: &'a str,
}
impl Runner for LiveRunner<'_> {
    fn run(&self, role: &str, config: &roles::RunConfig, prompt: &str) -> Result<String, String> {
        let run_id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let emit = |update| {
            self.window
                .emit_to(
                    self.window.label(),
                    "role-run",
                    supervisor::RoleRunEvent {
                        run_id,
                        task_id: self.id,
                        camp_id: self.camp,
                        role_id: role,
                        artifact: None,
                        update,
                    },
                )
                .map_err(|e| format!("无法发送作战会议运行事件：{e}"))
        };
        let result = supervisor::dispatch(
            &config.cli,
            StartOptions {
                cwd: self.cwd,
                model: &config.model,
                system_prompt: &config.system_prompt,
                prompt,
                permissions: &config.permissions,
                resume: None,
                persist_session: false,
            },
            self.slot,
            &emit,
        )
        .map(|run| run.text);
        emit(if self.stopped() {
            RunUpdate::Stopped
        } else if let Err(message) = &result {
            RunUpdate::Failed {
                message: message.clone(),
            }
        } else {
            RunUpdate::Completed
        })?;
        result
    }
    fn stopped(&self) -> bool {
        self.slot.is_stopped()
    }
    fn turn(&self, turn: &Turn) -> Result<(), String> {
        self.window
            .emit_to(
                self.window.label(),
                "conversation",
                ConversationEvent {
                    camp_id: self.camp,
                    conversation_id: self.id,
                    turn: Some(turn.clone()),
                    outcome: None,
                    message: None,
                },
            )
            .map_err(|e| format!("无法发送作战会议发言：{e}"))
    }
}
#[tauri::command]
pub async fn start_conversation(
    window: WebviewWindow,
    camp_id: String,
    role_ids: Vec<String>,
    request: String,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let slot = window.state::<RunSlot>();
        let _active = slot.acquire()?;
        let conn = window.state::<Mutex<Connection>>();
        let cwd = camps::run_root(&*conn.lock().map_err(|_| "无法访问基地数据库")?, &camp_id)?;
        let root = window.state::<artifacts::ArtifactRoot>();
        let millis = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| format!("无法读取系统时间：{e}"))?
            .as_millis() as i64;
        let id = format!("talk-{millis}-{}", NEXT_ID.fetch_add(1, Ordering::Relaxed));
        let runner = LiveRunner {
            window: &window,
            slot: &slot,
            cwd: &cwd,
            camp: &camp_id,
            id: &id,
        };
        let result = Session {
            conn: &conn,
            root: &root.0,
            camp: &camp_id,
            id: &id,
            millis,
        }
        .execute(&role_ids, &request, &runner);
        let outcome = match &result {
            Ok(true) => "plan_saved",
            Ok(false) => "stopped",
            Err(_) => "failed",
        };
        window
            .emit_to(
                window.label(),
                "conversation",
                ConversationEvent {
                    camp_id: &camp_id,
                    conversation_id: &id,
                    turn: None,
                    outcome: Some(outcome),
                    message: result.as_ref().err().map(String::as_str),
                },
            )
            .map_err(|e| format!("无法发送作战会议结果：{e}"))?;
        if matches!(result, Ok(true)) {
            window
                .emit_to(window.label(), "plan-state", &camp_id)
                .map_err(|e| format!("无法刷新计划卡墙：{e}"))?;
        }
        result.map(|_| ())
    })
    .await
    .map_err(|e| format!("作战会议运行失败：{e}"))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};
    struct Fake {
        calls: RefCell<Vec<(String, serde_json::Value)>>,
        stop_after: usize,
        reply: String,
    }
    impl Runner for Fake {
        fn run(
            &self,
            role: &str,
            config: &roles::RunConfig,
            prompt: &str,
        ) -> Result<String, String> {
            assert!(config.permissions.allow.iter().all(|rule| ["Read", "Grep", "Glob"].contains(&rule.split('(').next().unwrap())));
            assert!(config.permissions.deny.iter().any(|tool| tool == "Bash"));
            self.calls
                .borrow_mut()
                .push((role.into(), serde_json::from_str(prompt).unwrap()));
            if self.calls.borrow().len() <= 4 {
                Ok(format!("观点{}", self.calls.borrow().len()))
            } else {
                Ok(self.reply.clone())
            }
        }
        fn stopped(&self) -> bool {
            self.calls.borrow().len() >= self.stop_after
        }
        fn turn(&self, _: &Turn) -> Result<(), String> {
            Ok(())
        }
    }
    fn fake(reply: &str, stop_after: usize) -> Fake {
        Fake {
            calls: RefCell::new(Vec::new()),
            stop_after,
            reply: reply.into(),
        }
    }
    fn ids() -> Vec<String> {
        vec!["architect".into(), "backend".into()]
    }
    #[test]
    fn validates_request_and_participants() {
        let conn = roles::tests::connection();
        for (ids, request) in [
            (ids(), " "),
            (vec![], "需求"),
            (vec!["architect".into()], "需求"),
            (vec!["architect".into(); 2], "需求"),
            (vec!["missing".into(), "backend".into()], "需求"),
            (vec!["architect".into(); 5], "需求"),
        ] {
            assert!(participants(&conn, "camp", &ids, request).is_err());
        }
        assert_eq!(
            participants(&conn, "camp", &ids(), "需求").unwrap().0.len(),
            2
        );
    }
    #[test]
    fn ordered_discussion_persists_messages_and_draft() {
        let conn = Mutex::new(roles::tests::connection());
        let root = std::env::temp_dir().join(format!(
            "pixelcode-talk-{}",
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        ));
        let session = Session {
            conn: &conn,
            root: &root,
            camp: "camp",
            id: "talk-test",
            millis: 1,
        };
        let runner = fake(
            include_str!("../../tests/scheduler/captain-plan.json"),
            usize::MAX,
        );
        assert!(session.execute(&ids(), "需求", &runner).unwrap());
        let calls = runner.calls.borrow();
        assert_eq!(
            calls.iter().map(|c| c.0.as_str()).collect::<Vec<_>>(),
            ["architect", "backend", "architect", "backend", "captain"]
        );
        for (index, (_, prompt)) in calls[..4].iter().enumerate() {
            assert_eq!(prompt["transcript"].as_array().unwrap().len(), index);
            assert_eq!(prompt["round"], (index / 2 + 1) as u32);
            if index > 0 {
                assert_eq!(
                    prompt["transcript"][index - 1]["text"],
                    format!("观点{index}")
                );
            }
        }
        assert!(
            calls[4].1["user_request"]
                .as_str()
                .unwrap()
                .contains("观点4")
        );
        let conn = conn.lock().unwrap();
        assert_eq!(conn.query_row("SELECT count(*) FROM messages WHERE task_id='talk-test' AND role='assistant' AND type='discussion' AND json_extract(payload_json,'$.round') IN (1,2)", [], |r| r.get::<_, i64>(0)).unwrap(), 4);
        assert_eq!(
            plans::latest(&conn, "camp").unwrap().unwrap().status,
            "draft"
        );
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn questions_errors_and_stop_never_save_a_plan() {
        for (reply, stop, expected) in [
            (r#"{"questions":["哪个平台？"]}"#, usize::MAX, "哪个平台？"),
            ("not json", usize::MAX, "草案"),
            ("", 0, ""),
            ("", 1, ""),
            ("", 5, ""),
        ] {
            let conn = Mutex::new(roles::tests::connection());
            let session = Session {
                conn: &conn,
                root: Path::new("unused"),
                camp: "camp",
                id: "talk-test",
                millis: 1,
            };
            let runner = fake(reply, stop);
            let result = session.execute(&ids(), "需求", &runner);
            if expected.is_empty() {
                assert!(!result.unwrap());
            } else {
                assert!(result.unwrap_err().contains(expected));
            }
            assert!(
                plans::latest(&conn.lock().unwrap(), "camp")
                    .unwrap()
                    .is_none()
            );
        }
        struct Broken(Cell<bool>);
        impl Runner for Broken {
            fn run(&self, _: &str, _: &roles::RunConfig, _: &str) -> Result<String, String> {
                self.0.set(true);
                Err("无法启动角色".into())
            }
            fn stopped(&self) -> bool {
                false
            }
            fn turn(&self, _: &Turn) -> Result<(), String> {
                unreachable!()
            }
        }
        let conn = Mutex::new(roles::tests::connection());
        let session = Session {
            conn: &conn,
            root: Path::new("unused"),
            camp: "camp",
            id: "talk-test",
            millis: 1,
        };
        assert_eq!(
            session
                .execute(&ids(), "需求", &Broken(Cell::new(false)))
                .unwrap_err(),
            "无法启动角色"
        );
        assert!(
            plans::latest(&conn.lock().unwrap(), "camp")
                .unwrap()
                .is_none()
        );
    }
}

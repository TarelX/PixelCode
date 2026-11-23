CREATE TABLE run_events (
    run_id TEXT NOT NULL,
    seq INTEGER NOT NULL,
    kind TEXT NOT NULL,
    task_id TEXT,
    payload_json TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    PRIMARY KEY (run_id, seq)
);

CREATE TABLE pending_questions (
    id TEXT PRIMARY KEY,
    camp_id TEXT NOT NULL,
    run_id TEXT NOT NULL,
    task_id TEXT NOT NULL,
    role_id TEXT NOT NULL,
    loadout_index INTEGER NOT NULL,
    request TEXT NOT NULL,
    questions_json TEXT NOT NULL,
    answer TEXT,
    answer_key TEXT,
    created_at INTEGER NOT NULL,
    answered_at INTEGER
);

CREATE TABLE notification_log (
    event_id TEXT PRIMARY KEY,
    created_at INTEGER NOT NULL
);

CREATE TABLE camps (
    id TEXT PRIMARY KEY,
    name TEXT,
    root_path TEXT UNIQUE NOT NULL,
    created_at INTEGER,
    last_opened_at INTEGER
);

CREATE TABLE roles (
    id TEXT PRIMARY KEY,
    camp_id TEXT,
    role_type TEXT NOT NULL,
    name TEXT NOT NULL,
    name_en TEXT,
    title TEXT,
    avatar TEXT,
    personality TEXT NOT NULL,
    skills_json TEXT NOT NULL,
    loadouts_json TEXT NOT NULL,
    permissions_json TEXT NOT NULL,
    created_at INTEGER,
    updated_at INTEGER
);

CREATE TABLE tasks (
    id TEXT PRIMARY KEY,
    camp_id TEXT NOT NULL,
    plan_id TEXT NOT NULL,
    title TEXT NOT NULL,
    description TEXT NOT NULL,
    role_id TEXT NOT NULL,
    depends_on TEXT,
    input_artifacts TEXT,
    output_artifacts TEXT,
    acceptance_criteria TEXT NOT NULL,
    check_command TEXT,
    status TEXT NOT NULL,
    attempts INTEGER DEFAULT 0,
    review_rounds INTEGER DEFAULT 0,
    created_at INTEGER,
    updated_at INTEGER
);

CREATE TABLE plans (
    id TEXT PRIMARY KEY,
    camp_id TEXT NOT NULL,
    scenario TEXT NOT NULL,
    user_request TEXT NOT NULL,
    plan_json TEXT NOT NULL,
    status TEXT NOT NULL,
    created_at INTEGER
);

CREATE TABLE artifacts (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL,
    type TEXT NOT NULL,
    file_path TEXT NOT NULL,
    content_preview TEXT,
    created_at INTEGER
);

CREATE TABLE messages (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL,
    role TEXT NOT NULL,
    type TEXT NOT NULL,
    content TEXT NOT NULL,
    payload_json TEXT,
    created_at INTEGER
);

CREATE TABLE app_settings (
    key TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL
);

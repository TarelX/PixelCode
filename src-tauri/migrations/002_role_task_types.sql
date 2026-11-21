ALTER TABLE roles RENAME TO roles_legacy;
CREATE TABLE roles (
    id TEXT NOT NULL,
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
    updated_at INTEGER,
    task_types_json TEXT NOT NULL DEFAULT '[]',
    PRIMARY KEY (id, camp_id)
);
CREATE UNIQUE INDEX roles_global_id ON roles(id) WHERE camp_id IS NULL;
INSERT INTO roles SELECT *, '[]' FROM roles_legacy;
DROP TABLE roles_legacy;

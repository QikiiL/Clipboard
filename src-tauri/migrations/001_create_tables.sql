CREATE TABLE IF NOT EXISTS groups (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    name            TEXT NOT NULL,
    icon            TEXT DEFAULT '📁',
    sort_order      INTEGER DEFAULT 0,
    created_at      TEXT DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS items (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    type            INTEGER NOT NULL,
    content         TEXT,
    content_hash    TEXT NOT NULL,
    file_path       TEXT,
    preview         TEXT,
    copy_count      INTEGER DEFAULT 1,
    is_favorite     INTEGER DEFAULT 0,
    group_id        INTEGER REFERENCES groups(id),
    created_at      TEXT DEFAULT (datetime('now')),
    last_used_at    TEXT DEFAULT (datetime('now'))
);

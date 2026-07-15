PRAGMA foreign_keys = ON;

CREATE TABLE chats (
    id TEXT PRIMARY KEY NOT NULL,
    title TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE messages (
    id TEXT PRIMARY KEY NOT NULL,
    chat_id TEXT NOT NULL REFERENCES chats(id) ON DELETE CASCADE,
    role TEXT NOT NULL CHECK (role IN ('user', 'assistant')),
    content TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('complete', 'streaming', 'failed')),
    created_at TEXT NOT NULL
);

CREATE INDEX messages_chat_created_idx ON messages(chat_id, created_at, id);
CREATE INDEX chats_updated_idx ON chats(updated_at DESC);


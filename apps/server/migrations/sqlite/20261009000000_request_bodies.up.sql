-- A key may keep what was asked and answered (off by default), for as many
-- days as it says, or until deleted when it says none.
ALTER TABLE api_keys ADD COLUMN log_bodies BOOLEAN NOT NULL DEFAULT 0;
ALTER TABLE api_keys ADD COLUMN body_retention_days BIGINT;

-- Apart from `request_logs`, so usage and billing never read them. JSON text:
-- the request as the client sent it, the reply as it got it (a stream folded
-- into one). No foreign key: the purge drops the bodies of deleted keys.
CREATE TABLE request_bodies (
    request_id TEXT PRIMARY KEY,
    key_id INTEGER NOT NULL,
    request TEXT NOT NULL,
    response TEXT,
    created_at TEXT NOT NULL
);
CREATE INDEX idx_request_bodies_key_created ON request_bodies (key_id, created_at);
-- A body is read through its log line.
CREATE INDEX idx_request_logs_request ON request_logs (request_id);

-- OAuth 2.1 for /mcp: llmtrack as the authorization server of its own MCP
-- endpoint. Only SHA-256 hashes of codes and tokens are stored.

-- Authorization codes: single use, minutes long, bound to the PKCE challenge.
CREATE TABLE oauth_codes (
    code_hash TEXT PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    client_id TEXT NOT NULL,
    client_name TEXT NOT NULL,
    redirect_uri TEXT NOT NULL,
    resource TEXT NOT NULL,
    code_challenge TEXT NOT NULL,
    expires_at TEXT NOT NULL
);

-- One row per connected client: its current access and refresh token, both
-- replaced on every refresh.
CREATE TABLE oauth_grants (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    client_id TEXT NOT NULL,
    client_name TEXT NOT NULL,
    resource TEXT NOT NULL,
    access_hash TEXT NOT NULL UNIQUE,
    access_expires_at TEXT NOT NULL,
    refresh_hash TEXT NOT NULL UNIQUE,
    refresh_expires_at TEXT NOT NULL,
    created_at TEXT NOT NULL
);
CREATE INDEX idx_oauth_grants_user ON oauth_grants (user_id);

-- Every refresh token a grant has spent, kept as long as the grant: one coming
-- back means it was stolen, and ends the connection (RFC 9700 4.14.2).
CREATE TABLE oauth_spent_refresh_tokens (
    refresh_hash TEXT PRIMARY KEY,
    grant_id INTEGER NOT NULL REFERENCES oauth_grants(id) ON DELETE CASCADE
);
CREATE INDEX idx_oauth_spent_refresh_tokens_grant ON oauth_spent_refresh_tokens (grant_id);

-- Full-text index over the stored bodies: the string values of the request
-- and the reply (what was said, not the JSON keys), so `/api/logs?text=` and
-- the MCP search find a conversation by its words. Contentless: the text
-- lives in `request_bodies` only. Invalid JSON indexes nothing rather than
-- failing the usage batch that writes it.
CREATE VIRTUAL TABLE request_bodies_fts USING fts5(
    text,
    content = '',
    contentless_delete = 1,
    tokenize = 'unicode61 remove_diacritics 2'
);

-- The words of a body: every string value of its JSON, or nothing.
CREATE TRIGGER request_bodies_fts_insert AFTER INSERT ON request_bodies BEGIN
    INSERT INTO request_bodies_fts (rowid, text) VALUES (
        new.rowid,
        COALESCE(CASE WHEN json_valid(new.request) THEN
            (SELECT group_concat(value, ' ') FROM json_tree(new.request) WHERE type = 'text')
        END, '')
        || ' ' ||
        COALESCE(CASE WHEN json_valid(new.response) THEN
            (SELECT group_concat(value, ' ') FROM json_tree(new.response) WHERE type = 'text')
        END, '')
    );
END;

CREATE TRIGGER request_bodies_fts_delete AFTER DELETE ON request_bodies BEGIN
    DELETE FROM request_bodies_fts WHERE rowid = old.rowid;
END;

INSERT INTO request_bodies_fts (rowid, text)
SELECT b.rowid,
       COALESCE(CASE WHEN json_valid(b.request) THEN
           (SELECT group_concat(value, ' ') FROM json_tree(b.request) WHERE type = 'text')
       END, '')
       || ' ' ||
       COALESCE(CASE WHEN json_valid(b.response) THEN
           (SELECT group_concat(value, ' ') FROM json_tree(b.response) WHERE type = 'text')
       END, '')
FROM request_bodies b;

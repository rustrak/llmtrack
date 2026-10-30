-- OAuth 2.1 for /mcp: llmtrack as the authorization server of its own MCP
-- endpoint. Only SHA-256 hashes of codes and tokens are stored.

-- Authorization codes: single use, minutes long, bound to the PKCE challenge.
CREATE TABLE oauth_codes (
    code_hash TEXT PRIMARY KEY,
    user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    client_id TEXT NOT NULL,
    client_name TEXT NOT NULL,
    redirect_uri TEXT NOT NULL,
    resource TEXT NOT NULL,
    code_challenge TEXT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL
);

-- One row per connected client: its current access and refresh token, both
-- replaced on every refresh.
CREATE TABLE oauth_grants (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    client_id TEXT NOT NULL,
    client_name TEXT NOT NULL,
    resource TEXT NOT NULL,
    access_hash TEXT NOT NULL UNIQUE,
    access_expires_at TIMESTAMPTZ NOT NULL,
    refresh_hash TEXT NOT NULL UNIQUE,
    refresh_expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL
);
CREATE INDEX idx_oauth_grants_user ON oauth_grants (user_id);

-- Every refresh token a grant has spent, kept as long as the grant: one coming
-- back means it was stolen, and ends the connection (RFC 9700 4.14.2).
CREATE TABLE oauth_spent_refresh_tokens (
    refresh_hash TEXT PRIMARY KEY,
    grant_id BIGINT NOT NULL REFERENCES oauth_grants(id) ON DELETE CASCADE
);
CREATE INDEX idx_oauth_spent_refresh_tokens_grant ON oauth_spent_refresh_tokens (grant_id);

-- Full-text index over the stored bodies: the string values of the request
-- and the reply (what was said, not the JSON keys), so `/api/logs?text=` and
-- the MCP search find a conversation by its words. A body too large for a
-- tsvector (1 MB) or not JSON is left unindexed rather than failing the usage
-- batch that writes it.
ALTER TABLE request_bodies ADD COLUMN search tsvector;

CREATE FUNCTION request_bodies_search() RETURNS trigger AS $$
BEGIN
    BEGIN
        NEW.search := jsonb_to_tsvector('simple', NEW.request::jsonb, '["string"]')
            || jsonb_to_tsvector('simple', COALESCE(NEW.response, 'null')::jsonb, '["string"]');
    EXCEPTION WHEN OTHERS THEN
        NEW.search := NULL;
    END;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER request_bodies_search BEFORE INSERT OR UPDATE OF request, response
    ON request_bodies FOR EACH ROW EXECUTE FUNCTION request_bodies_search();

UPDATE request_bodies SET request = request;

CREATE INDEX idx_request_bodies_search ON request_bodies USING GIN (search);

-- llmtrack's schema. Money is integers: spend and budgets in nano-USD, prices
-- (inside models.pricing) in micro-USD per million tokens. Timestamps are
-- always written from Rust, never defaulted here: SQLite compares them as
-- strings, so they must all share one format.

-- People who sign in to the dashboard.
CREATE TABLE users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    email TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('admin', 'member')),
    is_active BOOLEAN NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL,
    last_login TEXT,
    name TEXT,
    -- BCP 47 tag and IANA zone the dashboard renders in; NULL follows the browser.
    language TEXT,
    timezone TEXT,
    -- ISO 4217 code the dashboard shows money in; NULL is USD.
    currency TEXT,
    -- Units of currency one US dollar buys, typed by the user.
    currency_rate REAL
);

-- Rustrak's invitations: shared by hand as a link, accepted by setting a
-- password. "Expired" is computed from expires_at, never stored.
CREATE TABLE invitations (
    token TEXT PRIMARY KEY,
    email TEXT NOT NULL,
    role TEXT NOT NULL,
    status TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    invited_by INTEGER REFERENCES users (id) ON DELETE SET NULL,
    created_at TEXT NOT NULL,
    accepted_at TEXT
);
CREATE INDEX idx_invitations_email ON invitations (email);

-- Who is billed. A budget may reset every period (budget_duration); the
-- reset happens lazily, on the first request after budget_reset_at.
CREATE TABLE teams (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    max_budget_nanos BIGINT,
    spend_nanos BIGINT NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    budget_duration TEXT,
    budget_reset_at TEXT,
    rpm_limit BIGINT,
    tpm_limit BIGINT,
    -- Every model, present and future; otherwise only the team_models rows.
    all_models BOOLEAN NOT NULL DEFAULT 0,
    max_parallel_requests BIGINT
);

CREATE TABLE team_members (
    team_id INTEGER NOT NULL REFERENCES teams (id) ON DELETE CASCADE,
    user_id INTEGER NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    role TEXT NOT NULL CHECK (role IN ('admin', 'member')),
    PRIMARY KEY (team_id, user_id)
);
CREATE INDEX idx_team_members_user ON team_members (user_id);

-- Deployments: several rows may share a name (a model group), and
-- requests for the name are spread across them by weight.
CREATE TABLE models (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    -- What clients send as "model".
    name TEXT NOT NULL,
    provider TEXT NOT NULL,
    -- What the provider is asked for.
    upstream_model TEXT NOT NULL,
    api_base TEXT,
    -- AES-GCM, sealed with a key derived from SECRET_KEY.
    api_key_encrypted TEXT,
    is_active BOOLEAN NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL,
    -- Azure's API version.
    api_version TEXT,
    -- The price-list entry it is billed by, in `catalog` mode.
    catalog_key TEXT,
    -- Own rates (JSON), in `custom` mode.
    pricing TEXT,
    weight BIGINT,
    -- `catalog`, `custom` or `free`.
    pricing_mode TEXT NOT NULL DEFAULT 'catalog'
);
CREATE INDEX idx_models_name ON models (name);

CREATE TABLE team_models (
    team_id INTEGER NOT NULL REFERENCES teams (id) ON DELETE CASCADE,
    model_id INTEGER NOT NULL REFERENCES models (id) ON DELETE CASCADE,
    PRIMARY KEY (team_id, model_id)
);

-- People who spend through a team's keys without signing in to the
-- dashboard: the team's own staff, a client's employees. Dashboard accounts
-- stay in `users`. Created on their own and placed in a team later;
-- deleting a team leaves its people without one.
CREATE TABLE people (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    team_id INTEGER REFERENCES teams (id) ON DELETE SET NULL,
    name TEXT NOT NULL,
    email TEXT,
    created_at TEXT NOT NULL,
    UNIQUE (team_id, name)
);

-- Virtual keys. A team key bills to its team; a personal key (no team) to
-- its user. The raw key is shown once; only its SHA-256 is kept.
CREATE TABLE api_keys (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    team_id INTEGER REFERENCES teams (id) ON DELETE CASCADE,
    user_id INTEGER REFERENCES users (id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    key_hash TEXT NOT NULL UNIQUE,
    last4 TEXT NOT NULL,
    created_by INTEGER REFERENCES users (id) ON DELETE SET NULL,
    max_budget_nanos BIGINT,
    spend_nanos BIGINT NOT NULL DEFAULT 0,
    budget_duration TEXT,
    budget_reset_at TEXT,
    rpm_limit BIGINT,
    tpm_limit BIGINT,
    blocked BOOLEAN NOT NULL DEFAULT 0,
    expires_at TEXT,
    -- Revoked keys stay, so their history keeps a name.
    revoked_at TEXT,
    last_used_at TEXT,
    created_at TEXT NOT NULL,
    max_parallel_requests BIGINT,
    -- A team key may be one person's.
    person_id INTEGER REFERENCES people (id) ON DELETE SET NULL,
    CHECK (team_id IS NOT NULL OR user_id IS NOT NULL)
);
CREATE INDEX idx_api_keys_team ON api_keys (team_id);
CREATE INDEX idx_api_keys_user ON api_keys (user_id);

-- A key's own allowlist inside its team's; empty means everything it may.
CREATE TABLE key_models (
    key_id INTEGER NOT NULL REFERENCES api_keys (id) ON DELETE CASCADE,
    model_id INTEGER NOT NULL REFERENCES models (id) ON DELETE CASCADE,
    PRIMARY KEY (key_id, model_id)
);

-- One row per request (and per failed attempt of the router). No foreign
-- keys: history outlives the keys, teams and models it names. Prompts and
-- completions are never stored.
CREATE TABLE request_logs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    request_id TEXT NOT NULL,
    team_id INTEGER,
    user_id INTEGER,
    key_id INTEGER NOT NULL,
    model_name TEXT NOT NULL,
    provider TEXT NOT NULL,
    status_code INTEGER NOT NULL,
    prompt_tokens BIGINT NOT NULL,
    completion_tokens BIGINT NOT NULL,
    cost_nanos BIGINT NOT NULL,
    latency_ms BIGINT NOT NULL,
    stream BOOLEAN NOT NULL,
    error TEXT,
    created_at TEXT NOT NULL,
    cached_tokens BIGINT NOT NULL DEFAULT 0,
    cache_write_tokens BIGINT NOT NULL DEFAULT 0,
    reasoning_tokens BIGINT NOT NULL DEFAULT 0,
    endpoint TEXT NOT NULL DEFAULT 'chat/completions',
    -- The deployment that answered.
    model_id BIGINT,
    -- The customer (end user), and the request's tags (JSON array).
    end_user TEXT,
    tags TEXT
);
CREATE INDEX idx_request_logs_created ON request_logs (created_at);
CREATE INDEX idx_request_logs_team_created ON request_logs (team_id, created_at);
CREATE INDEX idx_request_logs_key_created ON request_logs (key_id, created_at);
CREATE INDEX idx_request_logs_user_created ON request_logs (user_id, created_at);

-- Daily rollups, what reports read. The key identifies a row; team and user
-- are who it bills to and who may see it.
CREATE TABLE usage_daily (
    day TEXT NOT NULL,
    key_id INTEGER NOT NULL,
    model_name TEXT NOT NULL,
    team_id INTEGER,
    user_id INTEGER,
    requests BIGINT NOT NULL DEFAULT 0,
    failed_requests BIGINT NOT NULL DEFAULT 0,
    prompt_tokens BIGINT NOT NULL DEFAULT 0,
    completion_tokens BIGINT NOT NULL DEFAULT 0,
    cost_nanos BIGINT NOT NULL DEFAULT 0,
    cached_tokens BIGINT NOT NULL DEFAULT 0,
    cache_write_tokens BIGINT NOT NULL DEFAULT 0,
    reasoning_tokens BIGINT NOT NULL DEFAULT 0,
    -- Milliseconds summed, so reports can average latency without reading
    -- the request log.
    latency_ms BIGINT NOT NULL DEFAULT 0,
    -- Whose key it was that day, so reassigning a key does not move its
    -- history.
    person_id INTEGER,
    PRIMARY KEY (day, key_id, model_name)
);
CREATE INDEX idx_usage_daily_team ON usage_daily (team_id, day);
CREATE INDEX idx_usage_daily_user ON usage_daily (user_id, day);
CREATE INDEX idx_usage_daily_person ON usage_daily (person_id, day);

CREATE TABLE usage_daily_end_users (
    day TEXT NOT NULL,
    key_id BIGINT NOT NULL,
    end_user TEXT NOT NULL,
    team_id BIGINT,
    user_id BIGINT,
    requests BIGINT NOT NULL DEFAULT 0,
    failed_requests BIGINT NOT NULL DEFAULT 0,
    prompt_tokens BIGINT NOT NULL DEFAULT 0,
    completion_tokens BIGINT NOT NULL DEFAULT 0,
    cost_nanos BIGINT NOT NULL DEFAULT 0,
    cached_tokens BIGINT NOT NULL DEFAULT 0,
    cache_write_tokens BIGINT NOT NULL DEFAULT 0,
    reasoning_tokens BIGINT NOT NULL DEFAULT 0,
    latency_ms BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (day, key_id, end_user)
);
CREATE INDEX idx_usage_daily_end_users_day ON usage_daily_end_users (day);

-- A request with several tags counts once under each.
CREATE TABLE usage_daily_tags (
    day TEXT NOT NULL,
    key_id BIGINT NOT NULL,
    tag TEXT NOT NULL,
    team_id BIGINT,
    user_id BIGINT,
    requests BIGINT NOT NULL DEFAULT 0,
    failed_requests BIGINT NOT NULL DEFAULT 0,
    prompt_tokens BIGINT NOT NULL DEFAULT 0,
    completion_tokens BIGINT NOT NULL DEFAULT 0,
    cost_nanos BIGINT NOT NULL DEFAULT 0,
    cached_tokens BIGINT NOT NULL DEFAULT 0,
    cache_write_tokens BIGINT NOT NULL DEFAULT 0,
    reasoning_tokens BIGINT NOT NULL DEFAULT 0,
    latency_ms BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (day, key_id, tag)
);
CREATE INDEX idx_usage_daily_tags_day ON usage_daily_tags (day);

-- Instance-wide settings, one row per key: the public URL, the synced price
-- list, the router settings.
CREATE TABLE settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- Labels an admin defines and assigns to keys, to group and filter usage.
-- A filter counts the spend of the keys that carry the label now.
CREATE TABLE labels (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    name TEXT NOT NULL,
    -- One of the dashboard's palette (`models/label.rs`).
    color TEXT NOT NULL,
    description TEXT,
    created_at TIMESTAMPTZ NOT NULL
);
CREATE UNIQUE INDEX idx_labels_name ON labels (LOWER(name));

CREATE TABLE key_labels (
    key_id BIGINT NOT NULL REFERENCES api_keys (id) ON DELETE CASCADE,
    label_id BIGINT NOT NULL REFERENCES labels (id) ON DELETE CASCADE,
    PRIMARY KEY (key_id, label_id)
);
CREATE INDEX idx_key_labels_label ON key_labels (label_id);

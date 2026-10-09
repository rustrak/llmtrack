DROP INDEX idx_request_logs_request;
DROP TABLE request_bodies;
ALTER TABLE api_keys DROP COLUMN body_retention_days;
ALTER TABLE api_keys DROP COLUMN log_bodies;

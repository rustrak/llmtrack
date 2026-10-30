DROP INDEX idx_request_bodies_search;
DROP TRIGGER request_bodies_search ON request_bodies;
DROP FUNCTION request_bodies_search();
ALTER TABLE request_bodies DROP COLUMN search;
DROP TABLE oauth_spent_refresh_tokens;
DROP TABLE oauth_grants;
DROP TABLE oauth_codes;

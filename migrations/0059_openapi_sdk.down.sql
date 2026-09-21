-- 0059_openapi_sdk.down.sql — reverse 0059 (reversible migration).

ALTER TABLE ledgers DROP COLUMN IF EXISTS incoming_events_secret;
DROP TABLE IF EXISTS automation_rules;

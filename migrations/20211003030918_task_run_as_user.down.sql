BEGIN;
ALTER TABLE tasks DROP COLUMN run_as;
ALTER TABLE api_keys ALTER COLUMN user_id SET NULL;
COMMIT;

ALTER TABLE wallets
    ADD COLUMN IF NOT EXISTS kms_key_id TEXT UNIQUE;

UPDATE wallets
SET kms_key_id = CONCAT('legacy:', id)
WHERE kms_key_id IS NULL;

ALTER TABLE wallets
    ALTER COLUMN kms_key_id SET NOT NULL;

ALTER TABLE wallets
    DROP COLUMN IF EXISTS keypair;

DROP DOMAIN IF EXISTS keypair;

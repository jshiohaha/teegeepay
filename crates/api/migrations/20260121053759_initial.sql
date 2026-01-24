CREATE DOMAIN pubkey AS VARCHAR(44);
CREATE DOMAIN keypair AS VARCHAR(88);
CREATE DOMAIN u64 AS NUMERIC(20, 0) CHECK (value >= 0);

CREATE TABLE IF NOT EXISTS users (
    id BIGSERIAL PRIMARY KEY,
    user_id TEXT NOT NULL UNIQUE,
    telegram_user_id BIGINT UNIQUE,
    telegram_username TEXT,
    telegram_first_name TEXT,
    telegram_last_name TEXT,
    telegram_language_code TEXT,
    telegram_auth_date TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_users_user_id ON users(user_id);
CREATE INDEX idx_users_telegram_user_id ON users(telegram_user_id);

CREATE TABLE IF NOT EXISTS wallets (
    id BIGSERIAL PRIMARY KEY,
    user_id BIGINT NOT NULL REFERENCES users(id),
    pubkey pubkey NOT NULL UNIQUE,
    -- WARN: this was for hackathon prototype only. DO NOT DO THIS IN PRODUCTION.
    keypair keypair NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_wallets_pubkey ON wallets(pubkey);

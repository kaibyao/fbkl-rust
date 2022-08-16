CREATE EXTENSION IF NOT EXISTS pgcrypto; /* password hashing */

CREATE TABLE users (
  id BIGSERIAL PRIMARY KEY,
  email TEXT NOT NULL,
  hashed_password TEXT NOT NULL,
  confirmed_at TIMESTAMP WITH TIME ZONE,
  is_superadmin BOOLEAN NOT NULL DEFAULT false,
  inserted_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE UNIQUE INDEX users_unique_email ON users (email);

CREATE TYPE token_type_enum AS ENUM ('registration_confirm', 'session');

CREATE TABLE user_tokens (
  id BIGSERIAL PRIMARY KEY,
  user_id BIGINT NOT NULL REFERENCES users ON DELETE CASCADE,
  token BYTEA NOT NULL,
  token_type token_type_enum NOT NULL,
  sent_to TEXT,
  inserted_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX user_tokens_user_ids ON user_tokens (user_id);

/* TODO: check what context (session or confirm) and sent_to (email address if confirm) do */

CREATE EXTENSION IF NOT EXISTS citext; /* case-insensitive emails */
CREATE EXTENSION IF NOT EXISTS pgcrypto; /* password hashing */

CREATE TABLE users (
  id BIGSERIAL PRIMARY KEY,
  email CITEXT NOT NULL,
  hashed_password TEXT NOT NULL,
  confirmed_at TIMESTAMP WITH TIME ZONE,
  is_superadmin BOOLEAN NOT NULL DEFAULT false,
  created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE UNIQUE INDEX users_unique_email ON users (email);

CREATE TABLE user_tokens (
  id BIGSERIAL PRIMARY KEY,
  user_id BIGSERIAL NOT NULL REFERENCES users ON DELETE CASCADE,
  token TEXT NOT NULL,
  created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
  expires_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP + INTERVAL '60 days'
);

CREATE INDEX user_tokens_user_ids ON user_tokens (user_id);

/* TODO: check what context (session or confirm) and sent_to (email address if confirm) do */

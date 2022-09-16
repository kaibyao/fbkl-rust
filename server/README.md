# fbkl-rust/server

The server app that powers FBKL.

## Setup

1. Install Rust by [following the instructions here](https://www.rust-lang.org/learn/get-started).
2. Install postgresql and [lefthook](https://github.com/evilmartians/lefthook): `brew install postgresql libpq lefthook`.
3. `cd server`.
4. `cargo install sea-orm-cli`.
5. Copy/paste `.env.dev` -> `.env` (TODO: replace with `config` crate and load/override via config file < ENV < CLI flags).
6. Update `.env`.
7. `cd ../db`.
8. Run: `DATABASE_URL=postgres://<your_db_user_name_and_password>@localhost/fbkl_dev sea-orm-cli migrate up`.
9. `cd ..`.
10. Install lefthook: `lefthook install && lefthook run pre-commit`.

TODO: move the above steps into a shell script.

## When changing the database schema...

~~After making a database change, in case your schema.rs or model.rs breaks, run: `diesel print-schema > schema.rs` from `db/`.~~

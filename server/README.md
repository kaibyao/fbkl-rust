# fbkl-rust/server

The server app that powers FBKL.

## Setup

1. Install Rust by [following the instructions here](https://www.rust-lang.org/learn/get-started).
2. Install postgresql and [lefthook](https://github.com/evilmartians/lefthook): `brew install postgresql lefthook`.
3. `cd server`.
4. `cargo install diesel_cli`.
5. Copy/paste `.env.dev` -> `.env` (TODO: replace with `config` crate and load/override via config file < ENV < CLI flags).
6. Update `.env`.
7. Run: `diesel setup`.
8. Run: `diesel migration run`.
9. Install lefthook: `lefthook install && lefthook run pre-commit`.

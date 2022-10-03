# fbkl-rust/server

The server app that powers FBKL.

## Setup

1. Install Rust by [following the instructions here](https://www.rust-lang.org/learn/get-started).
1. Install postgresql and [lefthook](https://github.com/evilmartians/lefthook): `brew install postgresql libpq lefthook`.
1. Configure lefthook: `lefthook install && lefthook run pre-commit`.
1. Copy/paste `.env.dev` -> `.env` (TODO: replace with `config` crate and load/override via config file < ENV < CLI flags).
1. Update `.env`.
1. `cargo install sea-orm-cli`.
1. Run: `sea-orm-cli migrate up`.

TODO: move the above steps into a shell script.

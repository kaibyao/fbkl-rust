# Running Migrator CLI

This library + binary package is used for database updates. Make sure you first have `sea-orm-cli` installed: `cargo install sea-orm-cli`.

## Steps to creating a new database table

1. Create your migration file: `sea-orm-cli migrate generate <name_of_your_migration>`.
2. Update your migration file code.
3. Run your migration: `sea-orm-cli migrate up`.
4. Generate your entity file: `sea-orm-cli generate entity -o entity/src/entities --tables <table_name> --with-serde both`. **IT IS IMPORTANT THAT YOU INCLUDE `--tables <table_name>` or else you will override all custom changes to existing entity files.**
    - If your migration includes foreign key relations, it's possible that your existing entities may have become overriden. Make sure those existing entities only include the new relations.
5. If your new table has an `id` column, you probably don't want the application to be able to overwrite that column. Add the `#[serde(skip_deserializing)]` macro above the column definition. See `entities/user.rs` for example.

## Commands

- Generate a new migration file
    ```sh
    cargo run -- migrate generate MIGRATION_NAME
    ```
- Apply all pending migrations
    ```sh
    cargo run
    ```
    ```sh
    cargo run -- up
    ```
- Apply first 10 pending migrations
    ```sh
    cargo run -- up -n 10
    ```
- Rollback last applied migrations
    ```sh
    cargo run -- down
    ```
- Rollback last 10 applied migrations
    ```sh
    cargo run -- down -n 10
    ```
- Drop all tables from the database, then reapply all migrations
    ```sh
    cargo run -- fresh
    ```
- Rollback all applied migrations, then reapply all migrations
    ```sh
    cargo run -- refresh
    ```
- Rollback all applied migrations
    ```sh
    cargo run -- reset
    ```
- Check the status of all migrations
    ```sh
    cargo run -- status
    ```

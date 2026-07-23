//! This script takes the GraphQL schema defined in /server and generates the SDL files necessary for TypeScript to generate GraphQL types.

#![deny(clippy::all)]

use std::{
    fs,
    io::{self, Write},
};

use async_graphql::{EmptySubscription, Schema};
use fbkl_server::{MutationRoot, QueryRoot};

static SCHEMA_FILE_PATH_FOLDER: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/generated/");
static SCHEMA_FILE_PATH: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/generated/fbkl-schema.graphql");

fn main() -> io::Result<()> {
    fs::create_dir_all(SCHEMA_FILE_PATH_FOLDER)?;
    let mut sdl_file = fs::File::create(SCHEMA_FILE_PATH)?;
    let schema = Schema::build(
        QueryRoot::default(),
        MutationRoot::default(),
        EmptySubscription,
    )
    .finish();

    let sdl = schema.sdl();
    let sdl = sdl.trim();
    sdl_file.write_all(sdl.as_bytes())?;
    sdl_file.flush()?;

    println!(
        "Successfully wrote {} bytes to generated schema file at: {SCHEMA_FILE_PATH}.",
        sdl.len()
    );

    Ok(())
}

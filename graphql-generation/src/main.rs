//! This script takes the GraphQL schema defined in /server and generates the SDL files necessary for TypeScript to generate GraphQL types.

#![deny(clippy::all)]

use std::{
    fs,
    io::{self, Write},
};

use async_graphql::{EmptySubscription, Schema};
use fbkl_server::{MutationRoot, QueryRoot};

static SCHEMA_FILE_PATH_FOLDER: &str = "./generated/";
static SCHEMA_FILE_PATH: &str = "./generated/fbkl-schema.graphql";

fn main() -> io::Result<()> {
    fs::create_dir_all(SCHEMA_FILE_PATH_FOLDER)?;
    let mut sdl_file = fs::File::create(SCHEMA_FILE_PATH)?;
    let schema = Schema::build(
        QueryRoot::default(),
        MutationRoot::default(),
        EmptySubscription,
    )
    .finish();

    let size_bytes_written = sdl_file.write(schema.sdl().trim().as_bytes()).unwrap();
    sdl_file.flush().unwrap();

    println!(
        "Successfully wrote {} bytes to generated schema file at: {}.",
        size_bytes_written, SCHEMA_FILE_PATH
    );

    Ok(())
}

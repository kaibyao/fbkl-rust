mod error;
mod graphql;
mod handlers;
mod server;
mod session;

pub use graphql::*;
pub use server::*;

// #[cfg(test)]
// mod tests {
//     use super::*;

//     // #[test] // this goes above fn declaration
//     // If a user goes to /app:
//     // * he does not have a league selected and he has 0 leagues, default show the create league form.
//     // * he does not have a league selected and he has 1 league, show that league's page.
//     // * he does not have a league selected and he has >1 league, show leagues list page.
//     // * he does have a league selected, go to that league's page.
// }

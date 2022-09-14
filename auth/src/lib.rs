pub extern crate argon2;

use argon2::{
    password_hash::{Error as Argon2Error, PasswordHash, SaltString},
    Argon2, PasswordHasher, PasswordVerifier,
};
use rand::{rngs::OsRng, RngCore};

pub fn generate_token() -> [u8; 16] {
    let mut token = [0u8; 16];
    OsRng.fill_bytes(&mut token);

    token
}

pub fn generate_password_hash(password: String) -> Result<String, Argon2Error> {
    let password_bytes = password.as_bytes();
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();

    // Hash password to PHC string ($argon2id$v=19$...)
    let password_hash = argon2.hash_password(password_bytes, &salt)?.to_string();
    Ok(password_hash)
}

pub fn verify_password_against_hash(
    password_to_test: String,
    hashed_password: String,
) -> Result<(), Argon2Error> {
    let password_to_test_bytes = password_to_test.as_bytes();
    let parsed_password_hash = PasswordHash::new(&hashed_password)?;
    Argon2::default().verify_password(password_to_test_bytes, &parsed_password_hash)?;

    Ok(())
}

// #[cfg(test)]
// mod tests {
//     use super::*;

//     #[test]
//     fn it_works() {
//         let result = add(2, 2);
//         assert_eq!(result, 4);
//     }
// }

pub extern crate argon2;
pub extern crate hex;

use argon2::{
    password_hash::{Error as Argon2Error, PasswordHash, SaltString},
    Argon2, PasswordHasher, PasswordVerifier,
};
use rand::{rngs::OsRng, RngCore};

pub fn decode_token(token_str: &str) -> Result<Vec<u8>, hex::FromHexError> {
    hex::decode(token_str)
}

pub fn encode_token(token_bytes: &Vec<u8>) -> String {
    hex::encode(token_bytes)
}

pub fn generate_token() -> [u8; 16] {
    let mut token = [0u8; 16];
    OsRng.fill_bytes(&mut token);

    token
}

pub fn generate_password_hash(password: &str) -> Result<String, Argon2Error> {
    let password_bytes = password.as_bytes();
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();

    // Hash password to PHC string ($argon2id$v=19$...)
    let password_hash = argon2.hash_password(password_bytes, &salt)?.to_string();
    Ok(password_hash)
}

pub fn verify_password_against_hash(
    password_to_test: &str,
    hashed_password: &str,
) -> Result<(), Argon2Error> {
    let password_to_test_bytes = password_to_test.as_bytes();
    let parsed_password_hash = PasswordHash::new(hashed_password)?;
    Argon2::default().verify_password(password_to_test_bytes, &parsed_password_hash)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_verification() {
        let password = "test";
        let hash = generate_password_hash(password).unwrap();
        verify_password_against_hash(password, &hash).unwrap();
    }

    #[test]
    fn token_encoding() {
        let token = generate_token();
        assert_eq!(token.len(), 16);

        let token_owned = token.into_iter().collect::<Vec<u8>>();
        let encoded_token = encode_token(&token_owned);
        let decoded_token = decode_token(&encoded_token).unwrap();

        assert_eq!(token_owned, decoded_token);
    }
}

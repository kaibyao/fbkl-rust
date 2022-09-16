pub extern crate argon2;
pub extern crate hex;

use argon2::{
    password_hash::{Error as Argon2PasswordHashError, PasswordHash, SaltString},
    Argon2, Error as Argon2Error, Params, ParamsBuilder, PasswordHasher, PasswordVerifier,
};
use rand::{rngs::OsRng, RngCore};

/// Converts a token string into bytes.
pub fn decode_token(token_str: &str) -> Result<Vec<u8>, hex::FromHexError> {
    hex::decode(token_str)
}

/// Converts token byte array (read: vector) into a String.
pub fn encode_token(token_bytes: &Vec<u8>) -> String {
    hex::encode(token_bytes)
}

/// Generates a token with high entropy. See [OWASP Session Management Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Session_Management_Cheat_Sheet.html#session-id-length) for details.
pub fn generate_token() -> [u8; 64] {
    let mut token = [0u8; 64];
    OsRng.fill_bytes(&mut token);

    token
}

/// Hashes the given password using Argon2id. See [OWASP Password Storage Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Password_Storage_Cheat_Sheet.html) for details.
pub fn generate_password_hash(password: &str) -> Result<String, Argon2PasswordHashError> {
    let password_bytes = password.as_bytes();
    let salt = SaltString::generate(&mut OsRng);

    let argon2_params = get_argon2_params()?;
    let argon2 = Argon2::from(argon2_params);

    // Hash password to PHC string ($argon2id$v=19$...)
    let password_hash = argon2.hash_password(password_bytes, &salt)?.to_string();
    Ok(password_hash)
}

/// Checks that a given password matches a hashed password.
pub fn verify_password_against_hash(
    password_to_test: &str,
    hashed_password: &str,
) -> Result<(), Argon2PasswordHashError> {
    let password_to_test_bytes = password_to_test.as_bytes();
    let parsed_password_hash = PasswordHash::new(hashed_password)?;

    let argon2_params = get_argon2_params()?;
    Argon2::from(argon2_params).verify_password(password_to_test_bytes, &parsed_password_hash)?;

    Ok(())
}

fn get_argon2_params() -> Result<Params, Argon2Error> {
    let mut argon2_params = ParamsBuilder::new();

    argon2_params.m_cost(16777)?.p_cost(2)?;

    argon2_params.params()
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
        assert_eq!(token.len(), 64);

        let token_owned = token.into_iter().collect::<Vec<u8>>();
        let encoded_token = encode_token(&token_owned);
        let decoded_token = decode_token(&encoded_token).unwrap();

        assert_eq!(token_owned, decoded_token);
    }
}

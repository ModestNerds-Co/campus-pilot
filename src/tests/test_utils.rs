//
//  campus-pilot-apis
//  test_utils.rs
//
//  Created by Ngonidzashe Mangudya on 2025/10/02.
//  Copyright (c) 2025 Codecraft Solutions. All rights reserved.
//

#[cfg(test)]
mod tests {
    use crate::utils::{
        generate_access_token, generate_refresh_token, hash_password, verify_password, verify_token,
    };
    use uuid::Uuid;

    #[test]
    fn test_hash_password() {
        let password = "TestPassword123!";
        let hash = hash_password(password).expect("Failed to hash password");

        assert!(!hash.is_empty());
        assert_ne!(hash, password);
        assert!(hash.starts_with("$argon2"));
    }

    #[test]
    fn test_verify_password_success() {
        let password = "TestPassword123!";
        let hash = hash_password(password).expect("Failed to hash password");

        let valid = verify_password(password, &hash).expect("Failed to verify password");
        assert!(valid);
    }

    #[test]
    fn test_verify_password_failure() {
        let password = "TestPassword123!";
        let wrong_password = "WrongPassword456!";
        let hash = hash_password(password).expect("Failed to hash password");

        let valid = verify_password(wrong_password, &hash).expect("Failed to verify password");
        assert!(!valid);
    }

    #[test]
    fn test_generate_access_token() {
        let user_id = Uuid::new_v4();
        let email = "test@example.com";
        let roles = vec!["Admin".to_string()];
        let secret = "test-secret-key";

        let token = generate_access_token(user_id, email, roles.clone(), secret)
            .expect("Failed to generate access token");

        assert!(!token.is_empty());

        // Verify the token
        let claims = verify_token(&token, secret).expect("Failed to verify token");
        assert_eq!(claims.sub, user_id);
        assert_eq!(claims.email, email);
        assert_eq!(claims.roles, roles);
    }

    #[test]
    fn test_generate_refresh_token() {
        let user_id = Uuid::new_v4();
        let email = "test@example.com";
        let roles = vec!["Admin".to_string()];
        let secret = "test-secret-key";

        let token = generate_refresh_token(user_id, email, roles.clone(), secret)
            .expect("Failed to generate refresh token");

        assert!(!token.is_empty());

        // Verify the token
        let claims = verify_token(&token, secret).expect("Failed to verify token");
        assert_eq!(claims.sub, user_id);
        assert_eq!(claims.email, email);
        assert_eq!(claims.roles, roles);
    }

    #[test]
    fn test_verify_token_with_wrong_secret() {
        let user_id = Uuid::new_v4();
        let email = "test@example.com";
        let roles = vec!["Admin".to_string()];
        let secret = "test-secret-key";
        let wrong_secret = "wrong-secret-key";

        let token = generate_access_token(user_id, email, roles, secret)
            .expect("Failed to generate access token");

        let result = verify_token(&token, wrong_secret);
        assert!(result.is_err());
    }

    #[test]
    fn test_verify_token_with_invalid_token() {
        let secret = "test-secret-key";
        let invalid_token = "invalid.token.here";

        let result = verify_token(invalid_token, secret);
        assert!(result.is_err());
    }
}

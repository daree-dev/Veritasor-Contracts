#[cfg(test)]
mod tests {
    use deployment_validator::EnvValidator;
    use std::env;
    use tempfile::NamedTempFile;

    #[test]
    fn test_missing_vars() {
        let validator = EnvValidator::new();
        // Unset vars
        env::remove_var("SOROBAN_RPC_URL");
        let err = validator.validate_all().expect_err("should fail on missing vars");
        assert!(matches!(err, deployment_validator::ValidationError::MissingVar(_)));
    }

    #[test]
    fn test_invalid_rpc_url() {
        env::set_var("SOROBAN_RPC_URL", "invalid-url");
        env::set_var("STELLAR_SECRET_KEY", "validkey");
        env::set_var("STELLAR_NETWORK_PASSPHRASE", "Test SDF Network ; September 2015");
        env::set_var("CONTRACT_ADMIN_KEY", "validadmin");

        let validator = EnvValidator::new();
        let err = validator.validate_all().expect_err("invalid RPC should fail");
        assert!(matches!(err, deployment_validator::ValidationError::InvalidFormat(_, _)));
    }

    #[test]
    fn test_valid_complete_env() {
        env::set_var("SOROBAN_RPC_URL", "https://soroban-testnet.stellar.org:443");
        env::set_var("STELLAR_SECRET_KEY", "SCPUX7DFAO5ZBCI6P7VP5S4QZD6GHP5EQKI65W5VMJCVY4G7L6EVNSSS");
        env::set_var("STELLAR_NETWORK_PASSPHRASE", "Test SDF Network ; September 2015");
        env::set_var("CONTRACT_ADMIN_KEY", "SDE5US5HTX2MVDOYGZ5J7R7J3JYNHAOBTLXILN65AEB7M46S7OYY6C6U");
        env::set_var("SOROBAN_FEE_ACCOUNT", "GDSTRP67OHA6KLUHGDSR6R3JAY3PM3FBL7KPIO5WENHFBY7BTU5ZKFI2");

        let validator = EnvValidator::new();
        validator.validate_all().expect("valid env should pass");
    }
}


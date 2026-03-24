use clap::{Parser, Subcommand};
use dotenvy::dotenv;
use env_logger;
use log::{info, error, warn};
use regex::Regex;
use std::env;
use std::process;
use thiserror::Error;
use url::Url;

#[derive(Error, Debug)]
pub enum ValidationError {
    #[error("Missing required env var: {0}")]
    MissingVar(String),
    #[error("Invalid format for {0}: {1}")]
    InvalidFormat(String, String),
    #[error("Unsupported network passphrase: {0}")]
    UnsupportedPassphrase(String),
}

#[derive(Parser)]
#[command(name = "deployment-validator", about = "Validates critical env vars for Veritasor deployment")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Validate environment variables
    Validate {
        /// Path to .env file (default: .env)
        #[arg(short, long, default_value = ".env")]
        env_file: String,
    },
    /// Dry-run deployment check
    Check {
        /// Network: testnet, mainnet, futurenet
        network: String,
    },
}

struct EnvValidator {
    rpc_re: Regex,
    secret_key_re: Regex,
}

impl EnvValidator {
    fn new() -> Self {
        Self {
            rpc_re: Regex::new(r"^https?://[^:]+:\d{4,5}/?$").unwrap(),
            secret_key_re: Regex::new(r"^(G|A|B)([A-Z0-9]{55,56})$").unwrap(), // Stellar secret key
        }
    }

    fn load_env(&self, env_file: &str) -> Result<(), ValidationError> {
        if let Err(e) = dotenv::from_filename(env_file) {
            warn!("No .env file at {:?}, using OS env vars: {}", env_file, e);
        }
        env_logger::init();
        Ok(())
    }

    fn validate_required_var(&self, name: &str, value: Option<String>) -> Result<(), ValidationError> {
        let value = value.ok_or_else(|| ValidationError::MissingVar(name.to_string()))?;
        if value.trim().is_empty() {
            return Err(ValidationError::MissingVar(name.to_string()));
        }

        match name {
            "SOROBAN_RPC_URL" => {
                Url::parse(&value).map_err(|e| ValidationError::InvalidFormat(name.to_string(), e.to_string()))?;
                if !self.rpc_re.is_match(&value) {
                    return Err(ValidationError::InvalidFormat(name.to_string(), "must be http(s)://host:port format".to_string()));
                }
            }
            "STELLAR_SECRET_KEY" | "CONTRACT_ADMIN_KEY" => {
                if !self.secret_key_re.is_match(&value) {
                    return Err(ValidationError::InvalidFormat(name.to_string(), "must be valid Stellar secret key (G/A/B...56 chars)".to_string()));
                }
            }
            "STELLAR_NETWORK_PASSPHRASE" => {
                let supported = [
                    "Test SDF Network ; September 2015",
                    "Public Global Stellar Network ; September 2015",
                    "Test SDF Future Network ; October 2022",
                ];
                if !supported.iter().any(|s| s == &value.as_str()) {
                    return Err(ValidationError::UnsupportedPassphrase(value));
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn validate_all(&self) -> Result<(), ValidationError> {
        let required_vars = [
            "SOROBAN_RPC_URL",
            "STELLAR_SECRET_KEY",
            "STELLAR_NETWORK_PASSPHRASE",
            "CONTRACT_ADMIN_KEY",
        ];

        for &var in &required_vars {
            self.validate_required_var(var, env::var(var).ok())?;
        }

        // Optional but recommended
        if let Ok(value) = env::var("SOROBAN_FEE_ACCOUNT") {
            if value.trim().is_empty() {
                warn!("SOROBAN_FEE_ACCOUNT recommended for production");
            }
        }

        info!("✅ All critical env vars validated successfully");
        Ok(())
    }
}

fn main() {
    let cli = Cli::parse();

    let validator = EnvValidator::new();

    match cli.command {
        Commands::Validate { env_file } => {
            if let Err(e) = validator.load_env(&env_file) {
                error!("Failed to load env: {}", e);
                process::exit(1);
            }
            if let Err(e) = validator.validate_all() {
                error!("Validation failed: {}", e);
                process::exit(1);
            }
            println!("All environment variables are valid for deployment!");
        }
        Commands::Check { network } => {
            println!("Dry-run check for network: {}", network);
            // Future: integrate soroban client checks
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_valid_env() {
        let mut file = NamedTempFile::new().unwrap();
        std::fs::write(
            file.path(),
            r#"
SOROBAN_RPC_URL=https://soroban-testnet.stellar.org:443
STELLAR_SECRET_KEY=SCPUX7DFAO5ZBCI6P7VP5S4QZD6GHP5EQKI65W5VMJCVY4G7L6EVNSSS
STELLAR_NETWORK_PASSPHRASE=Test SDF Network ; September 2015
CONTRACT_ADMIN_KEY=SDE5US5HTX2MVDOYGZ5J7R7J3JYNHAOBTLXILN65AEB7M46S7OYY6C6U
"#,
        ).unwrap();

        validator.load_env(file.path().to_str().unwrap()).unwrap();
        validator.validate_all().unwrap();
    }
}


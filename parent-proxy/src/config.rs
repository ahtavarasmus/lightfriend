use anyhow::{Context, Result};
use std::env;

/// Configuration for the parent proxy
#[derive(Debug, Clone)]
pub struct Config {
    /// Path to the SQLite database
    pub database_path: String,
    /// VSOCK CID for old enclave
    pub old_enclave_cid: u32,
    /// VSOCK port for old enclave
    pub old_enclave_port: u32,
    /// VSOCK CID for new enclave
    pub new_enclave_cid: u32,
    /// VSOCK port for new enclave
    pub new_enclave_port: u32,
    /// HTTP listen port
    pub listen_port: u16,
}

impl Config {
    /// Load configuration from environment variables
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            database_path: env::var("DATABASE_PATH")
                .context("DATABASE_PATH must be set")?,
            old_enclave_cid: env::var("OLD_ENCLAVE_CID")
                .context("OLD_ENCLAVE_CID must be set")?
                .parse()
                .context("OLD_ENCLAVE_CID must be a valid u32")?,
            old_enclave_port: env::var("OLD_ENCLAVE_PORT")
                .context("OLD_ENCLAVE_PORT must be set")?
                .parse()
                .context("OLD_ENCLAVE_PORT must be a valid u32")?,
            new_enclave_cid: env::var("NEW_ENCLAVE_CID")
                .context("NEW_ENCLAVE_CID must be set")?
                .parse()
                .context("NEW_ENCLAVE_CID must be a valid u32")?,
            new_enclave_port: env::var("NEW_ENCLAVE_PORT")
                .context("NEW_ENCLAVE_PORT must be set")?
                .parse()
                .context("NEW_ENCLAVE_PORT must be a valid u32")?,
            listen_port: env::var("LISTEN_PORT")
                .unwrap_or_else(|_| "3000".to_string())
                .parse()
                .context("LISTEN_PORT must be a valid u16")?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_from_env() {
        // Set required env vars
        env::set_var("DATABASE_PATH", "/tmp/test.db");
        env::set_var("OLD_ENCLAVE_CID", "16");
        env::set_var("OLD_ENCLAVE_PORT", "5000");
        env::set_var("NEW_ENCLAVE_CID", "17");
        env::set_var("NEW_ENCLAVE_PORT", "5000");

        let config = Config::from_env().unwrap();
        assert_eq!(config.database_path, "/tmp/test.db");
        assert_eq!(config.old_enclave_cid, 16);
        assert_eq!(config.old_enclave_port, 5000);
        assert_eq!(config.new_enclave_cid, 17);
        assert_eq!(config.new_enclave_port, 5000);
        assert_eq!(config.listen_port, 3000);

        // Clean up
        env::remove_var("DATABASE_PATH");
        env::remove_var("OLD_ENCLAVE_CID");
        env::remove_var("OLD_ENCLAVE_PORT");
        env::remove_var("NEW_ENCLAVE_CID");
        env::remove_var("NEW_ENCLAVE_PORT");
    }
}

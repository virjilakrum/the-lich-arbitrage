//! Wallet configuration and management

use crate::error::{LichError, Result};
use serde::{Deserialize, Serialize};
use solana_sdk::{pubkey::Pubkey, signature::Keypair, signer::Signer};
use std::path::PathBuf;

/// Wallet configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletConfig {
    /// Wallet type (file, env, hardware)
    pub wallet_type: WalletType,
    
    /// Path to wallet file (for file-based wallets)
    pub wallet_path: Option<PathBuf>,
    
    /// Environment variable name containing the private key
    pub env_var_name: Option<String>,
    
    /// Wallet public key (for verification)
    pub public_key: Option<String>,
    
    /// Enable wallet encryption
    pub encrypted: bool,
    
    /// Passphrase environment variable (for encrypted wallets)
    pub passphrase_env: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WalletType {
    /// Load from file
    File,
    /// Load from environment variable
    Environment,
    /// Hardware wallet (future implementation)
    Hardware,
}

impl WalletConfig {
    /// Load the keypair based on configuration
    pub fn load_keypair(&self) -> Result<Keypair> {
        match self.wallet_type {
            WalletType::File => self.load_from_file(),
            WalletType::Environment => self.load_from_env(),
            WalletType::Hardware => Err(LichError::Wallet(
                "Hardware wallet support not yet implemented".to_string()
            )),
        }
    }
    
    /// Load keypair from file
    fn load_from_file(&self) -> Result<Keypair> {
        let path = self.wallet_path.as_ref()
            .ok_or_else(|| LichError::Wallet("Wallet path not specified".to_string()))?;
        
        if !path.exists() {
            return Err(LichError::Wallet(format!(
                "Wallet file not found: {}", 
                path.display()
            )));
        }
        
        let keypair_bytes = if self.encrypted {
            self.load_encrypted_file(path)?
        } else {
            std::fs::read(path)?
        };
        
        // Try to parse as JSON array first (Solana CLI format)
        if let Ok(json_bytes) = serde_json::from_slice::<Vec<u8>>(&keypair_bytes) {
            if json_bytes.len() == 64 {
                return Keypair::from_bytes(&json_bytes)
                    .map_err(|e| LichError::Wallet(format!("Invalid keypair format: {}", e)));
            }
        }
        
        // Try to parse as raw bytes
        if keypair_bytes.len() == 64 {
            return Keypair::from_bytes(&keypair_bytes)
                .map_err(|e| LichError::Wallet(format!("Invalid keypair format: {}", e)));
        }
        
        Err(LichError::Wallet("Invalid keypair file format".to_string()))
    }
    
    /// Load keypair from environment variable
    fn load_from_env(&self) -> Result<Keypair> {
        let env_var = self.env_var_name.as_ref()
            .ok_or_else(|| LichError::Wallet("Environment variable name not specified".to_string()))?;
        
        let private_key_str = std::env::var(env_var)
            .map_err(|_| LichError::Wallet(format!("Environment variable {} not found", env_var)))?;
        
        // Try to parse as base58 string
        if let Ok(bytes) = bs58::decode(&private_key_str).into_vec() {
            if bytes.len() == 64 {
                return Keypair::from_bytes(&bytes)
                    .map_err(|e| LichError::Wallet(format!("Invalid keypair format: {}", e)));
            }
        }
        
        // Try to parse as JSON array
        if let Ok(json_bytes) = serde_json::from_str::<Vec<u8>>(&private_key_str) {
            if json_bytes.len() == 64 {
                return Keypair::from_bytes(&json_bytes)
                    .map_err(|e| LichError::Wallet(format!("Invalid keypair format: {}", e)));
            }
        }
        
        Err(LichError::Wallet("Invalid private key format in environment variable".to_string()))
    }
    
    /// Load encrypted wallet file
    fn load_encrypted_file(&self, _path: &PathBuf) -> Result<Vec<u8>> {
        // TODO: Implement wallet encryption/decryption
        // For now, return an error
        Err(LichError::Wallet("Encrypted wallet support not yet implemented".to_string()))
    }
    
    /// Verify that the loaded keypair matches the configured public key
    pub fn verify_keypair(&self, keypair: &Keypair) -> Result<()> {
        if let Some(expected_pubkey_str) = &self.public_key {
            let expected_pubkey = expected_pubkey_str.parse::<Pubkey>()
                .map_err(|e| LichError::Wallet(format!("Invalid public key format: {}", e)))?;
            
            if keypair.pubkey() != expected_pubkey {
                return Err(LichError::Wallet(format!(
                    "Keypair public key mismatch: expected {}, got {}",
                    expected_pubkey,
                    keypair.pubkey()
                )));
            }
        }
        
        Ok(())
    }
}

impl Default for WalletConfig {
    fn default() -> Self {
        Self {
            wallet_type: WalletType::Environment,
            wallet_path: None,
            env_var_name: Some("LICH_PRIVATE_KEY".to_string()),
            public_key: None,
            encrypted: false,
            passphrase_env: None,
        }
    }
}

/// Wallet manager for secure keypair handling
pub struct WalletManager {
    keypair: Keypair,
    config: WalletConfig,
}

impl WalletManager {
    /// Create a new wallet manager
    pub fn new(config: WalletConfig) -> Result<Self> {
        let keypair = config.load_keypair()?;
        config.verify_keypair(&keypair)?;
        
        Ok(Self { keypair, config })
    }
    
    /// Get the public key
    pub fn pubkey(&self) -> Pubkey {
        self.keypair.pubkey()
    }
    
    /// Get a reference to the keypair (use sparingly)
    pub fn keypair(&self) -> &Keypair {
        &self.keypair
    }
    
    /// Sign data with the keypair
    pub fn sign(&self, data: &[u8]) -> Result<solana_sdk::signature::Signature> {
        Ok(self.keypair.sign_message(data))
    }
    
    /// Get wallet configuration
    pub fn config(&self) -> &WalletConfig {
        &self.config
    }
}

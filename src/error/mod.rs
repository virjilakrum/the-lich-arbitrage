//! Error handling for The Lich arbitrage bot

use thiserror::Error;

/// Main error type for The Lich bot
#[derive(Error, Debug)]
pub enum LichError {
    #[error("Configuration error: {0}")]
    Config(#[from] config::ConfigError),

    #[error("Solana client error: {0}")]
    SolanaClient(#[from] solana_client::client_error::ClientError),

    #[error("Solana SDK error: {0}")]
    SolanaSdk(String),

    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("WebSocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Wallet error: {0}")]
    Wallet(String),

    #[error("Insufficient balance: required {required}, available {available}")]
    InsufficientBalance { required: String, available: String },

    #[error("Arbitrage opportunity not profitable: expected {expected}%, actual {actual}%")]
    UnprofitableOpportunity { expected: String, actual: String },

    #[error("Slippage too high: expected {expected}%, actual {actual}%")]
    SlippageTooHigh { expected: String, actual: String },

    #[error("Transaction failed: {reason}")]
    TransactionFailed { reason: String },

    #[error("DEX error: {dex} - {message}")]
    DexError { dex: String, message: String },

    #[error("Price data stale: last update {last_update_ms}ms ago")]
    StalePriceData { last_update_ms: u64 },

    #[error("Connection lost: {endpoint}")]
    ConnectionLost { endpoint: String },

    #[error("Rate limit exceeded: {service}")]
    RateLimitExceeded { service: String },

    #[error("Invalid token pair: {token_a} / {token_b}")]
    InvalidTokenPair { token_a: String, token_b: String },

    #[error("Timeout error: {operation} took longer than {timeout_ms}ms")]
    Timeout { operation: String, timeout_ms: u64 },

    #[error("Kill switch activated: {reason}")]
    KillSwitchActivated { reason: String },

    #[error("Internal error: {0}")]
    Internal(String),
}

/// Result type alias for The Lich bot
pub type Result<T> = std::result::Result<T, LichError>;

impl From<anyhow::Error> for LichError {
    fn from(err: anyhow::Error) -> Self {
        LichError::Internal(err.to_string())
    }
}

impl From<solana_sdk::signature::SignerError> for LichError {
    fn from(err: solana_sdk::signature::SignerError) -> Self {
        LichError::Wallet(err.to_string())
    }
}

impl From<solana_sdk::pubkey::ParsePubkeyError> for LichError {
    fn from(err: solana_sdk::pubkey::ParsePubkeyError) -> Self {
        LichError::SolanaSdk(err.to_string())
    }
}

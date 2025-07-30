//! The Lich - High-Speed Solana Arbitrage Bot
//! 
//! A sophisticated arbitrage bot designed for the Solana blockchain that identifies
//! and executes profitable trading opportunities across multiple DEXs with minimal latency.

pub mod config;
pub mod network;
pub mod data;
pub mod arbitrage;
pub mod transaction;
pub mod monitoring;
pub mod error;

// Re-export commonly used types
pub use config::Config;
pub use error::{LichError, Result};

/// Bot version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const NAME: &str = env!("CARGO_PKG_NAME");

/// Default configuration values
pub mod defaults {
    use rust_decimal::Decimal;
    use std::time::Duration;

    /// Default minimum profit threshold (0.1%)
    pub const MIN_PROFIT_THRESHOLD: Decimal = Decimal::from_parts(1, 0, 0, false, 3); // 0.001 = 0.1%
    
    /// Default maximum slippage tolerance (0.2%)
    pub const MAX_SLIPPAGE_TOLERANCE: Decimal = Decimal::from_parts(2, 0, 0, false, 3); // 0.002 = 0.2%
    
    /// Default trade amount in USDC
    pub const DEFAULT_TRADE_AMOUNT: Decimal = Decimal::from_parts(1000, 0, 0, false, 0); // 1000 USDC
    
    /// Default WebSocket reconnection timeout
    pub const WS_RECONNECT_TIMEOUT: Duration = Duration::from_secs(5);
    
    /// Default RPC request timeout
    pub const RPC_TIMEOUT: Duration = Duration::from_secs(10);
    
    /// Default confirmation level for transactions
    pub const DEFAULT_COMMITMENT: &str = "confirmed";
}

//! Configuration management for The Lich arbitrage bot

use crate::error::{LichError, Result};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

pub mod wallet;
pub mod dex;
pub mod tokens;

pub use wallet::WalletConfig;
pub use dex::{DexConfig, DexType};
pub use tokens::TokenConfig;

/// Main configuration structure for The Lich bot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Bot identification and metadata
    pub bot: BotConfig,
    
    /// Network and RPC configuration
    pub network: NetworkConfig,
    
    /// Wallet and security configuration
    pub wallet: WalletConfig,
    
    /// DEX configurations
    pub dexes: HashMap<String, DexConfig>,
    
    /// Token configurations
    pub tokens: HashMap<String, TokenConfig>,
    
    /// Trading strategy configuration
    pub strategy: StrategyConfig,
    
    /// Monitoring and logging configuration
    pub monitoring: MonitoringConfig,
    
    /// Risk management configuration
    pub risk: RiskConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotConfig {
    /// Bot instance name
    pub name: String,
    
    /// Bot version
    pub version: String,
    
    /// Environment (development, staging, production)
    pub environment: String,
    
    /// Enable/disable bot execution
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// Primary RPC endpoints
    pub primary_rpc: Vec<String>,
    
    /// Backup RPC endpoints
    pub backup_rpc: Vec<String>,
    
    /// WebSocket endpoints
    pub websocket_endpoints: Vec<String>,
    
    /// RPC request timeout in milliseconds
    pub rpc_timeout_ms: u64,
    
    /// WebSocket reconnection timeout in seconds
    pub ws_reconnect_timeout_secs: u64,
    
    /// Maximum number of concurrent RPC requests
    pub max_concurrent_requests: usize,
    
    /// Commitment level for transactions
    pub commitment: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyConfig {
    /// Minimum profit threshold percentage (e.g., 0.001 = 0.1%)
    pub min_profit_threshold: Decimal,
    
    /// Maximum slippage tolerance percentage (e.g., 0.002 = 0.2%)
    pub max_slippage_tolerance: Decimal,
    
    /// Default trade amount in base currency
    pub default_trade_amount: Decimal,
    
    /// Maximum trade amount in base currency
    pub max_trade_amount: Decimal,
    
    /// Minimum trade amount in base currency
    pub min_trade_amount: Decimal,
    
    /// Enable triangular arbitrage
    pub enable_triangular_arbitrage: bool,
    
    /// Maximum number of hops for triangular arbitrage
    pub max_arbitrage_hops: u8,
    
    /// Token pairs to monitor for arbitrage
    pub monitored_pairs: Vec<TokenPair>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenPair {
    pub base: String,
    pub quote: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    /// Logging level (trace, debug, info, warn, error)
    pub log_level: String,
    
    /// Log file path
    pub log_file: Option<String>,
    
    /// Enable JSON logging
    pub json_logging: bool,
    
    /// Metrics collection interval in seconds
    pub metrics_interval_secs: u64,
    
    /// Notification configuration
    pub notifications: Option<NotificationConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationConfig {
    /// Telegram bot configuration
    pub telegram: Option<TelegramConfig>,
    
    /// Discord webhook configuration
    pub discord: Option<DiscordConfig>,
    
    /// Email notification configuration
    pub email: Option<EmailConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramConfig {
    pub bot_token: String,
    pub chat_id: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordConfig {
    pub webhook_url: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailConfig {
    pub smtp_server: String,
    pub smtp_port: u16,
    pub username: String,
    pub password: String,
    pub to_addresses: Vec<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskConfig {
    /// Maximum daily loss threshold
    pub max_daily_loss: Decimal,
    
    /// Maximum consecutive failed trades
    pub max_consecutive_failures: u32,
    
    /// Minimum SOL balance to maintain for fees
    pub min_sol_balance: Decimal,
    
    /// Enable kill switch
    pub enable_kill_switch: bool,
    
    /// Kill switch file path
    pub kill_switch_file: Option<String>,
    
    /// Maximum position size as percentage of total balance
    pub max_position_size_pct: Decimal,
}

impl Config {
    /// Load configuration from a TOML file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)
            .map_err(|e| LichError::Config(config::ConfigError::Message(e.to_string())))?;
        
        config.validate()?;
        Ok(config)
    }
    
    /// Load configuration from environment variables and file
    pub fn load() -> Result<Self> {
        let mut settings = config::Config::builder();
        
        // Start with default configuration
        settings = settings.add_source(config::File::with_name("config/default").required(false));
        
        // Add environment-specific configuration
        if let Ok(env) = std::env::var("LICH_ENV") {
            settings = settings.add_source(
                config::File::with_name(&format!("config/{}", env)).required(false)
            );
        }
        
        // Add local configuration (not committed to git)
        settings = settings.add_source(config::File::with_name("config/local").required(false));
        
        // Override with environment variables
        settings = settings.add_source(
            config::Environment::with_prefix("LICH")
                .prefix_separator("_")
                .separator("__")
        );
        
        let config: Config = settings.build()?.try_deserialize()?;
        config.validate()?;
        Ok(config)
    }
    
    /// Validate configuration values
    pub fn validate(&self) -> Result<()> {
        // Validate profit threshold
        if self.strategy.min_profit_threshold <= Decimal::ZERO {
            return Err(LichError::Config(config::ConfigError::Message(
                "Minimum profit threshold must be positive".to_string()
            )));
        }
        
        // Validate slippage tolerance
        if self.strategy.max_slippage_tolerance <= Decimal::ZERO || 
           self.strategy.max_slippage_tolerance >= Decimal::ONE {
            return Err(LichError::Config(config::ConfigError::Message(
                "Slippage tolerance must be between 0 and 1".to_string()
            )));
        }
        
        // Validate trade amounts
        if self.strategy.min_trade_amount >= self.strategy.max_trade_amount {
            return Err(LichError::Config(config::ConfigError::Message(
                "Minimum trade amount must be less than maximum trade amount".to_string()
            )));
        }
        
        // Validate RPC endpoints
        if self.network.primary_rpc.is_empty() {
            return Err(LichError::Config(config::ConfigError::Message(
                "At least one primary RPC endpoint must be configured".to_string()
            )));
        }
        
        // Validate DEX configurations
        if self.dexes.is_empty() {
            return Err(LichError::Config(config::ConfigError::Message(
                "At least one DEX must be configured".to_string()
            )));
        }
        
        // Validate monitored pairs
        if self.strategy.monitored_pairs.is_empty() {
            return Err(LichError::Config(config::ConfigError::Message(
                "At least one token pair must be monitored".to_string()
            )));
        }
        
        Ok(())
    }
    
    /// Get RPC timeout as Duration
    pub fn rpc_timeout(&self) -> Duration {
        Duration::from_millis(self.network.rpc_timeout_ms)
    }
    
    /// Get WebSocket reconnection timeout as Duration
    pub fn ws_reconnect_timeout(&self) -> Duration {
        Duration::from_secs(self.network.ws_reconnect_timeout_secs)
    }
    
    /// Get metrics collection interval as Duration
    pub fn metrics_interval(&self) -> Duration {
        Duration::from_secs(self.monitoring.metrics_interval_secs)
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bot: BotConfig {
                name: "the-lich".to_string(),
                version: crate::VERSION.to_string(),
                environment: "development".to_string(),
                enabled: true,
            },
            network: NetworkConfig {
                primary_rpc: vec![
                    "https://api.mainnet-beta.solana.com".to_string(),
                ],
                backup_rpc: vec![],
                websocket_endpoints: vec![
                    "wss://api.mainnet-beta.solana.com".to_string(),
                ],
                rpc_timeout_ms: 10000,
                ws_reconnect_timeout_secs: 5,
                max_concurrent_requests: 100,
                commitment: "confirmed".to_string(),
            },
            wallet: WalletConfig::default(),
            dexes: HashMap::new(),
            tokens: HashMap::new(),
            strategy: StrategyConfig {
                min_profit_threshold: crate::defaults::MIN_PROFIT_THRESHOLD,
                max_slippage_tolerance: crate::defaults::MAX_SLIPPAGE_TOLERANCE,
                default_trade_amount: crate::defaults::DEFAULT_TRADE_AMOUNT,
                max_trade_amount: Decimal::from(10000),
                min_trade_amount: Decimal::from(100),
                enable_triangular_arbitrage: false,
                max_arbitrage_hops: 3,
                monitored_pairs: vec![
                    TokenPair {
                        base: "SOL".to_string(),
                        quote: "USDC".to_string(),
                    },
                ],
            },
            monitoring: MonitoringConfig {
                log_level: "info".to_string(),
                log_file: Some("logs/lich.log".to_string()),
                json_logging: true,
                metrics_interval_secs: 60,
                notifications: None,
            },
            risk: RiskConfig {
                max_daily_loss: Decimal::from(1000),
                max_consecutive_failures: 5,
                min_sol_balance: Decimal::from_parts(1, 0, 0, false, 1), // 0.1 SOL
                enable_kill_switch: true,
                kill_switch_file: Some(".kill_switch".to_string()),
                max_position_size_pct: Decimal::from_parts(1, 0, 0, false, 1), // 10%
            },
        }
    }
}

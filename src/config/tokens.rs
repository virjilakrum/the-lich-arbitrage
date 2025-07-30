//! Token configuration and metadata management

use crate::error::{LichError, Result};
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use std::collections::HashMap;

/// Token configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenConfig {
    /// Token symbol (e.g., "SOL", "USDC")
    pub symbol: String,
    
    /// Token name (e.g., "Solana", "USD Coin")
    pub name: String,
    
    /// Token mint address
    pub mint: String,
    
    /// Number of decimal places
    pub decimals: u8,
    
    /// Whether this token is enabled for trading
    pub enabled: bool,
    
    /// Token type
    pub token_type: TokenType,
    
    /// Coingecko ID for price feeds
    pub coingecko_id: Option<String>,
    
    /// Pyth price feed account (if available)
    pub pyth_price_feed: Option<String>,
    
    /// Switchboard price feed account (if available)
    pub switchboard_price_feed: Option<String>,
    
    /// Minimum trade amount for this token
    pub min_trade_amount: Option<f64>,
    
    /// Maximum trade amount for this token
    pub max_trade_amount: Option<f64>,
    
    /// Token logo URL
    pub logo_url: Option<String>,
    
    /// Additional metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TokenType {
    /// Native SOL
    Native,
    /// SPL Token
    Spl,
    /// Wrapped token
    Wrapped,
    /// LP Token
    LpToken,
    /// Synthetic token
    Synthetic,
}

impl TokenConfig {
    /// Get the mint address as Pubkey
    pub fn mint_pubkey(&self) -> Result<Pubkey> {
        self.mint.parse()
            .map_err(|e| LichError::Config(config::ConfigError::Message(
                format!("Invalid mint address for {}: {}", self.symbol, e)
            )))
    }
    
    /// Get Pyth price feed as Pubkey
    pub fn pyth_price_feed_pubkey(&self) -> Result<Option<Pubkey>> {
        if let Some(feed) = &self.pyth_price_feed {
            Ok(Some(feed.parse().map_err(|e| LichError::Config(config::ConfigError::Message(
                format!("Invalid Pyth price feed for {}: {}", self.symbol, e)
            )))?))
        } else {
            Ok(None)
        }
    }
    
    /// Get Switchboard price feed as Pubkey
    pub fn switchboard_price_feed_pubkey(&self) -> Result<Option<Pubkey>> {
        if let Some(feed) = &self.switchboard_price_feed {
            Ok(Some(feed.parse().map_err(|e| LichError::Config(config::ConfigError::Message(
                format!("Invalid Switchboard price feed for {}: {}", self.symbol, e)
            )))?))
        } else {
            Ok(None)
        }
    }
    
    /// Convert raw amount to UI amount (considering decimals)
    pub fn raw_to_ui_amount(&self, raw_amount: u64) -> f64 {
        raw_amount as f64 / 10_f64.powi(self.decimals as i32)
    }
    
    /// Convert UI amount to raw amount (considering decimals)
    pub fn ui_to_raw_amount(&self, ui_amount: f64) -> u64 {
        (ui_amount * 10_f64.powi(self.decimals as i32)) as u64
    }
    
    /// Validate token configuration
    pub fn validate(&self) -> Result<()> {
        // Validate mint address
        self.mint_pubkey()?;
        
        // Validate price feeds if present
        self.pyth_price_feed_pubkey()?;
        self.switchboard_price_feed_pubkey()?;
        
        // Validate decimals (SPL tokens typically have <= 9 decimals)
        if self.decimals > 18 {
            return Err(LichError::Config(config::ConfigError::Message(
                format!("Invalid decimals for {}: {}", self.symbol, self.decimals)
            )));
        }
        
        // Validate trade amounts
        if let (Some(min), Some(max)) = (self.min_trade_amount, self.max_trade_amount) {
            if min >= max {
                return Err(LichError::Config(config::ConfigError::Message(
                    format!("Invalid trade amounts for {}: min {} >= max {}", self.symbol, min, max)
                )));
            }
        }
        
        Ok(())
    }
}

/// Token manager for handling token configurations and metadata
pub struct TokenManager {
    tokens: HashMap<String, TokenConfig>,
    mint_to_symbol: HashMap<Pubkey, String>,
}

impl TokenManager {
    /// Create a new token manager
    pub fn new(token_configs: HashMap<String, TokenConfig>) -> Result<Self> {
        let mut mint_to_symbol = HashMap::new();
        
        // Validate all token configurations and build mint lookup
        for (symbol, config) in &token_configs {
            config.validate()?;
            let mint = config.mint_pubkey()?;
            mint_to_symbol.insert(mint, symbol.clone());
        }
        
        Ok(Self {
            tokens: token_configs,
            mint_to_symbol,
        })
    }
    
    /// Get token configuration by symbol
    pub fn get_token(&self, symbol: &str) -> Option<&TokenConfig> {
        self.tokens.get(symbol)
    }
    
    /// Get token configuration by mint address
    pub fn get_token_by_mint(&self, mint: &Pubkey) -> Option<&TokenConfig> {
        self.mint_to_symbol.get(mint)
            .and_then(|symbol| self.tokens.get(symbol))
    }
    
    /// Get all enabled tokens
    pub fn enabled_tokens(&self) -> impl Iterator<Item = (&String, &TokenConfig)> {
        self.tokens.iter().filter(|(_, config)| config.enabled)
    }
    
    /// Get all token symbols
    pub fn all_symbols(&self) -> Vec<String> {
        self.tokens.keys().cloned().collect()
    }
    
    /// Get all mint addresses
    pub fn all_mints(&self) -> Vec<Pubkey> {
        self.mint_to_symbol.keys().cloned().collect()
    }
    
    /// Check if a token is supported
    pub fn is_supported(&self, symbol: &str) -> bool {
        self.tokens.contains_key(symbol)
    }
    
    /// Check if a mint is supported
    pub fn is_mint_supported(&self, mint: &Pubkey) -> bool {
        self.mint_to_symbol.contains_key(mint)
    }
    
    /// Get token pair information
    pub fn get_pair_info(&self, base: &str, quote: &str) -> Option<(TokenConfig, TokenConfig)> {
        let base_config = self.get_token(base)?.clone();
        let quote_config = self.get_token(quote)?.clone();
        Some((base_config, quote_config))
    }
}

/// Default token configurations for common Solana tokens
pub fn default_tokens() -> HashMap<String, TokenConfig> {
    let mut tokens = HashMap::new();
    
    // Native SOL
    tokens.insert("SOL".to_string(), TokenConfig {
        symbol: "SOL".to_string(),
        name: "Solana".to_string(),
        mint: "So11111111111111111111111111111111111111112".to_string(), // Wrapped SOL
        decimals: 9,
        enabled: true,
        token_type: TokenType::Native,
        coingecko_id: Some("solana".to_string()),
        pyth_price_feed: Some("H6ARHf6YXhGYeQfUzQNGk6rDNnLBQKrenN712K4AQJEG".to_string()),
        switchboard_price_feed: Some("GvDMxPzN1sCj7L26YDK2HnMRXEQmQ2aemov8YBtPS7vR".to_string()),
        min_trade_amount: Some(0.01),
        max_trade_amount: Some(1000.0),
        logo_url: Some("https://raw.githubusercontent.com/solana-labs/token-list/main/assets/mainnet/So11111111111111111111111111111111111111112/logo.png".to_string()),
        metadata: HashMap::new(),
    });
    
    // USDC
    tokens.insert("USDC".to_string(), TokenConfig {
        symbol: "USDC".to_string(),
        name: "USD Coin".to_string(),
        mint: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string(),
        decimals: 6,
        enabled: true,
        token_type: TokenType::Spl,
        coingecko_id: Some("usd-coin".to_string()),
        pyth_price_feed: Some("Gnt27xtC473ZT2Mw5u8wZ68Z3gULkSTb5DuxJy7eJotD".to_string()),
        switchboard_price_feed: Some("CZx29wKMUxaJDq6aLVQTdViPL754tTR64NAgQBUGxxHb".to_string()),
        min_trade_amount: Some(1.0),
        max_trade_amount: Some(100000.0),
        logo_url: Some("https://raw.githubusercontent.com/solana-labs/token-list/main/assets/mainnet/EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v/logo.png".to_string()),
        metadata: HashMap::new(),
    });
    
    // USDT
    tokens.insert("USDT".to_string(), TokenConfig {
        symbol: "USDT".to_string(),
        name: "Tether USD".to_string(),
        mint: "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB".to_string(),
        decimals: 6,
        enabled: true,
        token_type: TokenType::Spl,
        coingecko_id: Some("tether".to_string()),
        pyth_price_feed: Some("3vxLXJqLqF3JG5TCbYycbKWRBbCJQLxQmBGCkyqEEefL".to_string()),
        switchboard_price_feed: Some("5mp8kbkTYwWWCsKSte8rURjTuyinsqBpJ9xAQsewPDD".to_string()),
        min_trade_amount: Some(1.0),
        max_trade_amount: Some(100000.0),
        logo_url: Some("https://raw.githubusercontent.com/solana-labs/token-list/main/assets/mainnet/Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB/logo.png".to_string()),
        metadata: HashMap::new(),
    });
    
    // RAY (Raydium)
    tokens.insert("RAY".to_string(), TokenConfig {
        symbol: "RAY".to_string(),
        name: "Raydium".to_string(),
        mint: "4k3Dyjzvzp8eMZWUXbBCjEvwSkkk59S5iCNLY3QrkX6R".to_string(),
        decimals: 6,
        enabled: true,
        token_type: TokenType::Spl,
        coingecko_id: Some("raydium".to_string()),
        pyth_price_feed: Some("AnLf8tVYCM816gmBjiy8n53eXKKEDydT5piYjjQDPgTB".to_string()),
        switchboard_price_feed: None,
        min_trade_amount: Some(1.0),
        max_trade_amount: Some(10000.0),
        logo_url: Some("https://raw.githubusercontent.com/solana-labs/token-list/main/assets/mainnet/4k3Dyjzvzp8eMZWUXbBCjEvwSkkk59S5iCNLY3QrkX6R/logo.png".to_string()),
        metadata: HashMap::new(),
    });
    
    tokens
}

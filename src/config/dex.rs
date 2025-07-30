//! DEX configuration and management

use crate::error::{LichError, Result};
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use std::collections::HashMap;

/// DEX configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DexConfig {
    /// DEX type
    pub dex_type: DexType,
    
    /// DEX name
    pub name: String,
    
    /// Whether this DEX is enabled
    pub enabled: bool,
    
    /// DEX program ID
    pub program_id: String,
    
    /// DEX-specific configuration
    pub config: DexSpecificConfig,
    
    /// Supported token pairs
    pub supported_pairs: Vec<String>,
    
    /// Fee structure
    pub fees: FeeConfig,
    
    /// API endpoints (if applicable)
    pub api_endpoints: Option<ApiEndpoints>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DexType {
    Raydium,
    Orca,
    Jupiter,
    Serum,
    Saber,
    Aldrin,
    Cropper,
    Lifinity,
    Mercurial,
    Stepn,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DexSpecificConfig {
    /// AMM program ID (for Raydium)
    pub amm_program_id: Option<String>,
    /// Authority address (for Raydium)
    pub authority: Option<String>,
    /// Swap program ID (for Orca)
    pub swap_program_id: Option<String>,
    /// Aquafarm program ID (for Orca farming)
    pub aquafarm_program_id: Option<String>,
    /// Aggregator program ID (for Jupiter)
    pub aggregator_program_id: Option<String>,
    /// API base URL (for Jupiter)
    pub api_base_url: Option<String>,
    /// API version (for Jupiter)
    pub api_version: Option<String>,
    /// DEX program ID (for Serum)
    pub dex_program_id: Option<String>,
    /// Pool discovery method
    pub pool_discovery: Option<PoolDiscoveryMethod>,
    /// Market discovery method
    pub market_discovery: Option<MarketDiscoveryMethod>,
    /// Generic program IDs
    pub program_ids: Option<HashMap<String, String>>,
    /// Custom parameters
    pub parameters: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PoolDiscoveryMethod {
    /// Use static pool list
    Static { pools: Vec<PoolInfo> },
    /// Discover pools on-chain
    OnChain { scan_interval_secs: u64 },
    /// Use API endpoint
    Api { endpoint: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum MarketDiscoveryMethod {
    /// Use static market list
    Static { markets: Vec<MarketInfo> },
    /// Discover markets on-chain
    OnChain { scan_interval_secs: u64 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolInfo {
    pub pool_id: String,
    pub token_a: String,
    pub token_b: String,
    pub token_a_vault: String,
    pub token_b_vault: String,
    pub lp_token_mint: String,
    pub fee_rate: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketInfo {
    pub market_id: String,
    pub base_mint: String,
    pub quote_mint: String,
    pub base_vault: String,
    pub quote_vault: String,
    pub bids: String,
    pub asks: String,
    pub event_queue: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeeConfig {
    /// Trading fee percentage (e.g., 0.003 = 0.3%)
    pub trading_fee: f64,
    
    /// Protocol fee percentage
    pub protocol_fee: Option<f64>,
    
    /// LP fee percentage
    pub lp_fee: Option<f64>,
    
    /// Minimum fee in lamports
    pub min_fee_lamports: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiEndpoints {
    /// REST API base URL
    pub rest_api: Option<String>,
    
    /// WebSocket API URL
    pub websocket_api: Option<String>,
    
    /// Price feed URL
    pub price_feed: Option<String>,
    
    /// Pool info URL
    pub pool_info: Option<String>,
}

impl DexConfig {
    /// Get the program ID as Pubkey
    pub fn program_pubkey(&self) -> Result<Pubkey> {
        self.program_id.parse()
            .map_err(|e| LichError::DexError {
                dex: self.name.clone(),
                message: format!("Invalid program ID: {}", e),
            })
    }
    
    /// Check if a token pair is supported
    pub fn supports_pair(&self, base: &str, quote: &str) -> bool {
        let pair1 = format!("{}/{}", base, quote);
        let pair2 = format!("{}/{}", quote, base);
        
        self.supported_pairs.contains(&pair1) || self.supported_pairs.contains(&pair2)
    }
    
    /// Get total fee percentage
    pub fn total_fee_percentage(&self) -> f64 {
        let mut total = self.fees.trading_fee;
        
        if let Some(protocol_fee) = self.fees.protocol_fee {
            total += protocol_fee;
        }
        
        if let Some(lp_fee) = self.fees.lp_fee {
            total += lp_fee;
        }
        
        total
    }
    
    /// Validate DEX configuration
    pub fn validate(&self) -> Result<()> {
        // Validate program ID
        self.program_pubkey()?;
        
        // Validate fees
        if self.fees.trading_fee < 0.0 || self.fees.trading_fee > 1.0 {
            return Err(LichError::DexError {
                dex: self.name.clone(),
                message: "Trading fee must be between 0 and 1".to_string(),
            });
        }
        
        // Validate supported pairs format
        for pair in &self.supported_pairs {
            if !pair.contains('/') {
                return Err(LichError::DexError {
                    dex: self.name.clone(),
                    message: format!("Invalid pair format: {}", pair),
                });
            }
        }
        
        Ok(())
    }
}

impl Default for DexConfig {
    fn default() -> Self {
        Self {
            dex_type: DexType::Raydium,
            name: "raydium".to_string(),
            enabled: true,
            program_id: "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8".to_string(), // Raydium AMM
            config: DexSpecificConfig {
                amm_program_id: Some("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8".to_string()),
                authority: Some("5Q544fKrFoe6tsEbD7S8EmxGTJYAKtTVhAW5Q5pge4j1".to_string()),
                swap_program_id: None,
                aquafarm_program_id: None,
                aggregator_program_id: None,
                api_base_url: None,
                api_version: None,
                dex_program_id: None,
                pool_discovery: Some(PoolDiscoveryMethod::OnChain { scan_interval_secs: 300 }),
                market_discovery: None,
                program_ids: None,
                parameters: None,
            },
            supported_pairs: vec![
                "SOL/USDC".to_string(),
                "SOL/USDT".to_string(),
                "RAY/USDC".to_string(),
                "RAY/SOL".to_string(),
            ],
            fees: FeeConfig {
                trading_fee: 0.0025, // 0.25%
                protocol_fee: None,
                lp_fee: None,
                min_fee_lamports: Some(5000),
            },
            api_endpoints: None,
        }
    }
}

/// DEX manager for handling multiple DEX configurations
pub struct DexManager {
    dexes: HashMap<String, DexConfig>,
}

impl DexManager {
    /// Create a new DEX manager
    pub fn new(dex_configs: HashMap<String, DexConfig>) -> Result<Self> {
        // Validate all DEX configurations
        for (name, config) in &dex_configs {
            config.validate().map_err(|e| LichError::DexError {
                dex: name.clone(),
                message: format!("Configuration validation failed: {}", e),
            })?;
        }
        
        Ok(Self { dexes: dex_configs })
    }
    
    /// Get a DEX configuration by name
    pub fn get_dex(&self, name: &str) -> Option<&DexConfig> {
        self.dexes.get(name)
    }
    
    /// Get all enabled DEXs
    pub fn enabled_dexes(&self) -> impl Iterator<Item = (&String, &DexConfig)> {
        self.dexes.iter().filter(|(_, config)| config.enabled)
    }
    
    /// Get DEXs that support a specific token pair
    pub fn dexes_for_pair(&self, base: &str, quote: &str) -> Vec<(&String, &DexConfig)> {
        self.enabled_dexes()
            .filter(|(_, config)| config.supports_pair(base, quote))
            .collect()
    }
    
    /// Get all supported token pairs across all DEXs
    pub fn all_supported_pairs(&self) -> Vec<String> {
        let mut pairs = std::collections::HashSet::new();
        
        for (_, config) in self.enabled_dexes() {
            for pair in &config.supported_pairs {
                pairs.insert(pair.clone());
            }
        }
        
        pairs.into_iter().collect()
    }
}

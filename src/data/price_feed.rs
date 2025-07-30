//! Price feed management for real-time price data

use crate::config::TokenConfig;
use crate::error::{LichError, Result};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Price data from various sources
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceData {
    pub symbol: String,
    pub price: Decimal,
    pub confidence: Option<Decimal>,
    pub timestamp: u64,
    pub source: PriceSource,
    pub slot: Option<u64>,
}

impl PriceData {
    pub fn new(symbol: String, price: Decimal, source: PriceSource) -> Self {
        Self {
            symbol,
            price,
            confidence: None,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            source,
            slot: None,
        }
    }
    
    pub fn is_stale(&self, max_age_ms: u64) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        
        now - self.timestamp > max_age_ms
    }
}

/// Price data sources
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PriceSource {
    Pyth { feed_id: Pubkey },
    Switchboard { feed_id: Pubkey },
    Coingecko,
    Dex { dex_name: String, pool_id: Pubkey },
    Aggregated,
}

impl std::fmt::Display for PriceSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PriceSource::Pyth { .. } => write!(f, "Pyth"),
            PriceSource::Switchboard { .. } => write!(f, "Switchboard"),
            PriceSource::Coingecko => write!(f, "CoinGecko"),
            PriceSource::Dex { dex_name, .. } => write!(f, "DEX({})", dex_name),
            PriceSource::Aggregated => write!(f, "Aggregated"),
        }
    }
}

/// Price feed interface
#[async_trait::async_trait]
pub trait PriceFeed: Send + Sync {
    /// Get the latest price for a token
    async fn get_price(&self, symbol: &str) -> Result<Option<PriceData>>;
    
    /// Subscribe to price updates for a token
    async fn subscribe(&self, symbol: &str) -> Result<()>;
    
    /// Unsubscribe from price updates for a token
    async fn unsubscribe(&self, symbol: &str) -> Result<()>;
    
    /// Start the price feed
    async fn start(&self) -> Result<()>;
    
    /// Stop the price feed
    async fn stop(&self) -> Result<()>;
    
    /// Get the source name
    fn source_name(&self) -> &str;
}

/// Pyth price feed implementation
pub struct PythPriceFeed {
    token_configs: HashMap<String, TokenConfig>,
    price_cache: Arc<RwLock<HashMap<String, PriceData>>>,
    is_running: Arc<RwLock<bool>>,
}

impl PythPriceFeed {
    pub fn new(token_configs: HashMap<String, TokenConfig>) -> Self {
        Self {
            token_configs,
            price_cache: Arc::new(RwLock::new(HashMap::new())),
            is_running: Arc::new(RwLock::new(false)),
        }
    }
}

#[async_trait::async_trait]
impl PriceFeed for PythPriceFeed {
    async fn get_price(&self, symbol: &str) -> Result<Option<PriceData>> {
        let cache = self.price_cache.read().await;
        Ok(cache.get(symbol).cloned())
    }
    
    async fn subscribe(&self, symbol: &str) -> Result<()> {
        if let Some(config) = self.token_configs.get(symbol) {
            if let Some(_feed_id) = &config.pyth_price_feed {
                debug!("Subscribing to Pyth price feed for {}", symbol);
                // TODO: Implement actual Pyth subscription
                Ok(())
            } else {
                Err(LichError::Config(config::ConfigError::Message(
                    format!("No Pyth price feed configured for {}", symbol)
                )))
            }
        } else {
            Err(LichError::Config(config::ConfigError::Message(
                format!("Token {} not found in configuration", symbol)
            )))
        }
    }
    
    async fn unsubscribe(&self, symbol: &str) -> Result<()> {
        debug!("Unsubscribing from Pyth price feed for {}", symbol);
        // TODO: Implement actual Pyth unsubscription
        Ok(())
    }
    
    async fn start(&self) -> Result<()> {
        let mut is_running = self.is_running.write().await;
        if *is_running {
            return Ok(());
        }
        
        *is_running = true;
        info!("Starting Pyth price feed");
        
        // TODO: Implement actual Pyth connection and data streaming
        
        Ok(())
    }
    
    async fn stop(&self) -> Result<()> {
        let mut is_running = self.is_running.write().await;
        if !*is_running {
            return Ok(());
        }
        
        *is_running = false;
        info!("Stopping Pyth price feed");
        
        // TODO: Implement actual Pyth disconnection
        
        Ok(())
    }
    
    fn source_name(&self) -> &str {
        "Pyth"
    }
}

/// CoinGecko price feed implementation
pub struct CoinGeckoPriceFeed {
    token_configs: HashMap<String, TokenConfig>,
    price_cache: Arc<RwLock<HashMap<String, PriceData>>>,
    is_running: Arc<RwLock<bool>>,
    client: reqwest::Client,
}

impl CoinGeckoPriceFeed {
    pub fn new(token_configs: HashMap<String, TokenConfig>) -> Self {
        Self {
            token_configs,
            price_cache: Arc::new(RwLock::new(HashMap::new())),
            is_running: Arc::new(RwLock::new(false)),
            client: reqwest::Client::new(),
        }
    }
    
    async fn fetch_prices(&self) -> Result<()> {
        let mut coin_ids = Vec::new();
        let mut symbol_to_id = HashMap::new();
        
        for (symbol, config) in &self.token_configs {
            if let Some(coingecko_id) = &config.coingecko_id {
                coin_ids.push(coingecko_id.clone());
                symbol_to_id.insert(coingecko_id.clone(), symbol.clone());
            }
        }
        
        if coin_ids.is_empty() {
            return Ok(());
        }
        
        let url = format!(
            "https://api.coingecko.com/api/v3/simple/price?ids={}&vs_currencies=usd",
            coin_ids.join(",")
        );
        
        match self.client.get(&url).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    let data: HashMap<String, HashMap<String, f64>> = response.json().await?;
                    
                    let mut cache = self.price_cache.write().await;
                    for (coin_id, prices) in data {
                        if let (Some(symbol), Some(usd_price)) = (symbol_to_id.get(&coin_id), prices.get("usd")) {
                            let price_data = PriceData::new(
                                symbol.clone(),
                                Decimal::from_f64_retain(*usd_price).unwrap_or(Decimal::ZERO),
                                PriceSource::Coingecko,
                            );
                            cache.insert(symbol.clone(), price_data);
                        }
                    }
                } else {
                    warn!("CoinGecko API request failed: {}", response.status());
                }
            }
            Err(e) => {
                error!("Failed to fetch prices from CoinGecko: {}", e);
            }
        }
        
        Ok(())
    }
}

#[async_trait::async_trait]
impl PriceFeed for CoinGeckoPriceFeed {
    async fn get_price(&self, symbol: &str) -> Result<Option<PriceData>> {
        let cache = self.price_cache.read().await;
        Ok(cache.get(symbol).cloned())
    }
    
    async fn subscribe(&self, symbol: &str) -> Result<()> {
        if self.token_configs.contains_key(symbol) {
            debug!("Subscribing to CoinGecko price feed for {}", symbol);
            Ok(())
        } else {
            Err(LichError::Config(config::ConfigError::Message(
                format!("Token {} not found in configuration", symbol)
            )))
        }
    }
    
    async fn unsubscribe(&self, symbol: &str) -> Result<()> {
        debug!("Unsubscribing from CoinGecko price feed for {}", symbol);
        Ok(())
    }
    
    async fn start(&self) -> Result<()> {
        let mut is_running = self.is_running.write().await;
        if *is_running {
            return Ok(());
        }
        
        *is_running = true;
        info!("Starting CoinGecko price feed");
        
        // Start periodic price fetching
        let price_cache = self.price_cache.clone();
        let client = self.client.clone();
        let token_configs = self.token_configs.clone();
        let is_running_clone = self.is_running.clone();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(60)); // Fetch every minute
            
            while *is_running_clone.read().await {
                interval.tick().await;
                
                let feed = CoinGeckoPriceFeed {
                    token_configs: token_configs.clone(),
                    price_cache: price_cache.clone(),
                    is_running: is_running_clone.clone(),
                    client: client.clone(),
                };
                
                if let Err(e) = feed.fetch_prices().await {
                    error!("Failed to fetch CoinGecko prices: {}", e);
                }
            }
        });
        
        Ok(())
    }
    
    async fn stop(&self) -> Result<()> {
        let mut is_running = self.is_running.write().await;
        if !*is_running {
            return Ok(());
        }
        
        *is_running = false;
        info!("Stopping CoinGecko price feed");
        Ok(())
    }
    
    fn source_name(&self) -> &str {
        "CoinGecko"
    }
}

/// Price feed manager that aggregates multiple price sources
pub struct PriceFeedManager {
    feeds: Vec<Box<dyn PriceFeed>>,
    aggregated_prices: Arc<RwLock<HashMap<String, PriceData>>>,
    is_running: Arc<RwLock<bool>>,
}

impl PriceFeedManager {
    pub async fn new(token_configs: HashMap<String, TokenConfig>) -> Result<Self> {
        let mut feeds: Vec<Box<dyn PriceFeed>> = Vec::new();
        
        // Add Pyth price feed
        feeds.push(Box::new(PythPriceFeed::new(token_configs.clone())));
        
        // Add CoinGecko price feed
        feeds.push(Box::new(CoinGeckoPriceFeed::new(token_configs.clone())));
        
        Ok(Self {
            feeds,
            aggregated_prices: Arc::new(RwLock::new(HashMap::new())),
            is_running: Arc::new(RwLock::new(false)),
        })
    }
    
    /// Start all price feeds
    pub async fn start(&self) -> Result<()> {
        let mut is_running = self.is_running.write().await;
        if *is_running {
            return Ok(());
        }
        
        *is_running = true;
        info!("Starting price feed manager");
        
        // Start all feeds
        for feed in &self.feeds {
            if let Err(e) = feed.start().await {
                error!("Failed to start price feed {}: {}", feed.source_name(), e);
            }
        }
        
        // Start price aggregation loop
        self.start_aggregation_loop().await;
        
        Ok(())
    }
    
    /// Stop all price feeds
    pub async fn stop(&self) -> Result<()> {
        let mut is_running = self.is_running.write().await;
        if !*is_running {
            return Ok(());
        }
        
        *is_running = false;
        info!("Stopping price feed manager");
        
        // Stop all feeds
        for feed in &self.feeds {
            if let Err(e) = feed.stop().await {
                error!("Failed to stop price feed {}: {}", feed.source_name(), e);
            }
        }
        
        Ok(())
    }
    
    /// Start the price aggregation loop
    async fn start_aggregation_loop(&self) {
        let feed_count = self.feeds.len();
        let aggregated_prices = self.aggregated_prices.clone();
        let is_running = self.is_running.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(5)); // Aggregate every 5 seconds

            while *is_running.read().await {
                interval.tick().await;

                // TODO: Implement actual price aggregation logic
                debug!("Aggregating prices from {} sources", feed_count);
            }
        });
    }
    
    /// Get aggregated price for a token
    pub async fn get_price(&self, symbol: &str) -> Option<PriceData> {
        let prices = self.aggregated_prices.read().await;
        prices.get(symbol).cloned()
    }
    
    /// Subscribe to price updates for a token across all feeds
    pub async fn subscribe(&self, symbol: &str) -> Result<()> {
        for feed in &self.feeds {
            if let Err(e) = feed.subscribe(symbol).await {
                warn!("Failed to subscribe to {} on {}: {}", symbol, feed.source_name(), e);
            }
        }
        Ok(())
    }
    
    /// Unsubscribe from price updates for a token across all feeds
    pub async fn unsubscribe(&self, symbol: &str) -> Result<()> {
        for feed in &self.feeds {
            if let Err(e) = feed.unsubscribe(symbol).await {
                warn!("Failed to unsubscribe from {} on {}: {}", symbol, feed.source_name(), e);
            }
        }
        Ok(())
    }
}

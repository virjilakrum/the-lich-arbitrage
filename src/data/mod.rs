//! Data collection and management for arbitrage opportunities

use crate::config::{DexConfig, TokenConfig};
use crate::error::{LichError, Result};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

pub mod price_feed;
pub mod order_book;
pub mod pool_data;
pub mod market_data;

pub use price_feed::{PriceFeed, PriceData, PriceFeedManager};
pub use order_book::{OrderBook, OrderBookLevel, OrderBookManager};
pub use pool_data::{PoolData, PoolInfo, PoolManager};
pub use market_data::{MarketData, MarketDataManager};

/// Token pair identifier
#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenPair {
    pub base: String,
    pub quote: String,
}

impl TokenPair {
    pub fn new(base: String, quote: String) -> Self {
        Self { base, quote }
    }
    
    pub fn reverse(&self) -> Self {
        Self {
            base: self.quote.clone(),
            quote: self.base.clone(),
        }
    }
    
    pub fn to_string(&self) -> String {
        format!("{}/{}", self.base, self.quote)
    }
}

impl std::fmt::Display for TokenPair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.base, self.quote)
    }
}

/// DEX identifier
#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct DexId {
    pub name: String,
    pub program_id: Pubkey,
}

impl DexId {
    pub fn new(name: String, program_id: Pubkey) -> Self {
        Self { name, program_id }
    }
}

impl std::fmt::Display for DexId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

/// Market identifier combining DEX and token pair
#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketId {
    pub dex: DexId,
    pub pair: TokenPair,
}

impl MarketId {
    pub fn new(dex: DexId, pair: TokenPair) -> Self {
        Self { dex, pair }
    }
}

impl std::fmt::Display for MarketId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.dex, self.pair)
    }
}

/// Real-time price information for a market
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketPrice {
    pub market_id: MarketId,
    pub bid: Option<Decimal>,
    pub ask: Option<Decimal>,
    pub last_price: Option<Decimal>,
    pub volume_24h: Option<Decimal>,
    pub timestamp: u64,
    pub slot: Option<u64>,
}

impl MarketPrice {
    pub fn new(market_id: MarketId) -> Self {
        Self {
            market_id,
            bid: None,
            ask: None,
            last_price: None,
            volume_24h: None,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            slot: None,
        }
    }
    
    pub fn mid_price(&self) -> Option<Decimal> {
        match (self.bid, self.ask) {
            (Some(bid), Some(ask)) => Some((bid + ask) / Decimal::from(2)),
            _ => self.last_price,
        }
    }
    
    pub fn spread(&self) -> Option<Decimal> {
        match (self.bid, self.ask) {
            (Some(bid), Some(ask)) => Some(ask - bid),
            _ => None,
        }
    }
    
    pub fn spread_percentage(&self) -> Option<Decimal> {
        match (self.spread(), self.mid_price()) {
            (Some(spread), Some(mid)) if mid > Decimal::ZERO => {
                Some(spread / mid * Decimal::from(100))
            }
            _ => None,
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

/// Liquidity information for a market
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketLiquidity {
    pub market_id: MarketId,
    pub bid_liquidity: Vec<(Decimal, Decimal)>, // (price, size)
    pub ask_liquidity: Vec<(Decimal, Decimal)>, // (price, size)
    pub total_bid_liquidity: Decimal,
    pub total_ask_liquidity: Decimal,
    pub timestamp: u64,
}

impl MarketLiquidity {
    pub fn new(market_id: MarketId) -> Self {
        Self {
            market_id,
            bid_liquidity: Vec::new(),
            ask_liquidity: Vec::new(),
            total_bid_liquidity: Decimal::ZERO,
            total_ask_liquidity: Decimal::ZERO,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        }
    }
    
    pub fn get_liquidity_at_price(&self, price: Decimal, is_buy: bool) -> Decimal {
        let levels = if is_buy { &self.bid_liquidity } else { &self.ask_liquidity };
        
        levels.iter()
            .filter(|(level_price, _)| {
                if is_buy {
                    *level_price >= price
                } else {
                    *level_price <= price
                }
            })
            .map(|(_, size)| *size)
            .sum()
    }
    
    pub fn get_price_impact(&self, trade_size: Decimal, is_buy: bool) -> Option<Decimal> {
        let levels = if is_buy { &self.ask_liquidity } else { &self.bid_liquidity };
        
        if levels.is_empty() {
            return None;
        }
        
        let mut remaining_size = trade_size;
        let mut total_cost = Decimal::ZERO;
        let start_price = levels[0].0;
        
        for (price, size) in levels {
            if remaining_size <= Decimal::ZERO {
                break;
            }
            
            let trade_amount = remaining_size.min(*size);
            total_cost += trade_amount * price;
            remaining_size -= trade_amount;
        }
        
        if remaining_size > Decimal::ZERO {
            // Not enough liquidity
            return None;
        }
        
        let average_price = total_cost / trade_size;
        let impact = (average_price - start_price) / start_price * Decimal::from(100);
        
        Some(impact.abs())
    }
}

/// Data manager for collecting and managing market data
pub struct DataManager {
    price_feed_manager: Arc<PriceFeedManager>,
    order_book_manager: Arc<OrderBookManager>,
    pool_manager: Arc<PoolManager>,
    market_data_manager: Arc<MarketDataManager>,
    
    // Cached data
    market_prices: Arc<RwLock<HashMap<MarketId, MarketPrice>>>,
    market_liquidity: Arc<RwLock<HashMap<MarketId, MarketLiquidity>>>,
    
    // Configuration
    max_price_age_ms: u64,
    update_interval: Duration,
}

impl DataManager {
    pub async fn new(
        dex_configs: HashMap<String, DexConfig>,
        token_configs: HashMap<String, TokenConfig>,
        max_price_age_ms: u64,
        update_interval: Duration,
    ) -> Result<Self> {
        let price_feed_manager = Arc::new(PriceFeedManager::new(token_configs.clone()).await?);
        let order_book_manager = Arc::new(OrderBookManager::new().await?);
        let pool_manager = Arc::new(PoolManager::new(dex_configs.clone()).await?);
        let market_data_manager = Arc::new(MarketDataManager::new(
            dex_configs,
            token_configs,
        ).await?);
        
        Ok(Self {
            price_feed_manager,
            order_book_manager,
            pool_manager,
            market_data_manager,
            market_prices: Arc::new(RwLock::new(HashMap::new())),
            market_liquidity: Arc::new(RwLock::new(HashMap::new())),
            max_price_age_ms,
            update_interval,
        })
    }
    
    /// Start data collection
    pub async fn start(&self) -> Result<()> {
        info!("Starting data manager");
        
        // Start all sub-managers
        self.price_feed_manager.start().await?;
        self.order_book_manager.start().await?;
        self.pool_manager.start().await?;
        self.market_data_manager.start().await?;
        
        // Start data update loop
        self.start_update_loop().await;
        
        info!("Data manager started");
        Ok(())
    }
    
    /// Stop data collection
    pub async fn stop(&self) -> Result<()> {
        info!("Stopping data manager");
        
        // Stop all sub-managers
        self.price_feed_manager.stop().await?;
        self.order_book_manager.stop().await?;
        self.pool_manager.stop().await?;
        self.market_data_manager.stop().await?;
        
        info!("Data manager stopped");
        Ok(())
    }
    
    /// Start the data update loop
    async fn start_update_loop(&self) {
        let price_feed_manager = self.price_feed_manager.clone();
        let order_book_manager = self.order_book_manager.clone();
        let market_prices = self.market_prices.clone();
        let market_liquidity = self.market_liquidity.clone();
        let update_interval = self.update_interval;
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(update_interval);
            
            loop {
                interval.tick().await;
                
                // Update market prices and liquidity
                if let Err(e) = Self::update_market_data(
                    &price_feed_manager,
                    &order_book_manager,
                    &market_prices,
                    &market_liquidity,
                ).await {
                    error!("Failed to update market data: {}", e);
                }
            }
        });
    }
    
    /// Update market data from various sources
    async fn update_market_data(
        price_feed_manager: &Arc<PriceFeedManager>,
        order_book_manager: &Arc<OrderBookManager>,
        market_prices: &Arc<RwLock<HashMap<MarketId, MarketPrice>>>,
        market_liquidity: &Arc<RwLock<HashMap<MarketId, MarketLiquidity>>>,
    ) -> Result<()> {
        // This is a placeholder for the actual implementation
        // In a real implementation, this would:
        // 1. Fetch latest price data from price feeds
        // 2. Update order book data
        // 3. Calculate market prices and liquidity
        // 4. Update the cached data
        
        debug!("Updating market data");
        Ok(())
    }
    
    /// Get current market price for a market
    pub async fn get_market_price(&self, market_id: &MarketId) -> Option<MarketPrice> {
        let prices = self.market_prices.read().await;
        prices.get(market_id).cloned()
    }
    
    /// Get current market liquidity for a market
    pub async fn get_market_liquidity(&self, market_id: &MarketId) -> Option<MarketLiquidity> {
        let liquidity = self.market_liquidity.read().await;
        liquidity.get(market_id).cloned()
    }
    
    /// Get all current market prices
    pub async fn get_all_market_prices(&self) -> HashMap<MarketId, MarketPrice> {
        self.market_prices.read().await.clone()
    }
    
    /// Get prices for a specific token pair across all DEXs
    pub async fn get_prices_for_pair(&self, pair: &TokenPair) -> HashMap<DexId, MarketPrice> {
        let prices = self.market_prices.read().await;
        let mut result = HashMap::new();
        
        for (market_id, price) in prices.iter() {
            if market_id.pair == *pair {
                result.insert(market_id.dex.clone(), price.clone());
            }
        }
        
        result
    }
    
    /// Check if market data is fresh
    pub async fn is_data_fresh(&self, market_id: &MarketId) -> bool {
        if let Some(price) = self.get_market_price(market_id).await {
            !price.is_stale(self.max_price_age_ms)
        } else {
            false
        }
    }
    
    /// Get data freshness statistics
    pub async fn get_data_stats(&self) -> DataStats {
        let prices = self.market_prices.read().await;
        let total_markets = prices.len();
        let fresh_markets = prices.values()
            .filter(|price| !price.is_stale(self.max_price_age_ms))
            .count();
        
        DataStats {
            total_markets,
            fresh_markets,
            stale_markets: total_markets - fresh_markets,
            oldest_data_age_ms: prices.values()
                .map(|price| {
                    let now = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_millis() as u64;
                    now - price.timestamp
                })
                .max()
                .unwrap_or(0),
        }
    }
}

/// Data statistics
#[derive(Debug, Clone)]
pub struct DataStats {
    pub total_markets: usize,
    pub fresh_markets: usize,
    pub stale_markets: usize,
    pub oldest_data_age_ms: u64,
}

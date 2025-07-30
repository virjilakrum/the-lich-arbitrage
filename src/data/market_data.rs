//! Market data aggregation and management

use crate::config::{DexConfig, TokenConfig};
use crate::data::{DexId, MarketId, TokenPair};
use crate::error::{LichError, Result};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Aggregated market data for a specific market
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketData {
    pub market_id: MarketId,
    pub current_price: Option<Decimal>,
    pub bid_price: Option<Decimal>,
    pub ask_price: Option<Decimal>,
    pub volume_24h: Option<Decimal>,
    pub high_24h: Option<Decimal>,
    pub low_24h: Option<Decimal>,
    pub price_change_24h: Option<Decimal>,
    pub price_change_24h_percent: Option<Decimal>,
    pub liquidity: Option<Decimal>,
    pub spread: Option<Decimal>,
    pub spread_percent: Option<Decimal>,
    pub last_trade_price: Option<Decimal>,
    pub last_trade_size: Option<Decimal>,
    pub last_trade_time: Option<u64>,
    pub last_update: u64,
    pub data_quality: DataQuality,
}

impl MarketData {
    pub fn new(market_id: MarketId) -> Self {
        Self {
            market_id,
            current_price: None,
            bid_price: None,
            ask_price: None,
            volume_24h: None,
            high_24h: None,
            low_24h: None,
            price_change_24h: None,
            price_change_24h_percent: None,
            liquidity: None,
            spread: None,
            spread_percent: None,
            last_trade_price: None,
            last_trade_size: None,
            last_trade_time: None,
            last_update: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            data_quality: DataQuality::Unknown,
        }
    }
    
    /// Calculate mid price from bid/ask
    pub fn mid_price(&self) -> Option<Decimal> {
        match (self.bid_price, self.ask_price) {
            (Some(bid), Some(ask)) => Some((bid + ask) / Decimal::from(2)),
            _ => self.current_price,
        }
    }
    
    /// Update spread information
    pub fn update_spread(&mut self) {
        if let (Some(bid), Some(ask)) = (self.bid_price, self.ask_price) {
            self.spread = Some(ask - bid);
            
            if let Some(mid) = self.mid_price() {
                if mid > Decimal::ZERO {
                    self.spread_percent = Some(self.spread.unwrap() / mid * Decimal::from(100));
                }
            }
        }
    }
    
    /// Check if market data is stale
    pub fn is_stale(&self, max_age_ms: u64) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        
        now - self.last_update > max_age_ms
    }
    
    /// Update last update timestamp
    pub fn touch(&mut self) {
        self.last_update = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
    }
}

/// Data quality indicator
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DataQuality {
    /// High quality - recent data from multiple sources
    High,
    /// Medium quality - recent data from single source or slightly stale
    Medium,
    /// Low quality - stale data or unreliable source
    Low,
    /// Unknown quality
    Unknown,
}

/// Market data manager for aggregating data from multiple sources
pub struct MarketDataManager {
    market_data: Arc<RwLock<HashMap<MarketId, MarketData>>>,
    dex_configs: HashMap<String, DexConfig>,
    token_configs: HashMap<String, TokenConfig>,
    is_running: Arc<RwLock<bool>>,
    update_interval_ms: u64,
}

impl MarketDataManager {
    pub async fn new(
        dex_configs: HashMap<String, DexConfig>,
        token_configs: HashMap<String, TokenConfig>,
    ) -> Result<Self> {
        Ok(Self {
            market_data: Arc::new(RwLock::new(HashMap::new())),
            dex_configs,
            token_configs,
            is_running: Arc::new(RwLock::new(false)),
            update_interval_ms: 1000, // Update every second
        })
    }
    
    /// Start market data manager
    pub async fn start(&self) -> Result<()> {
        let mut is_running = self.is_running.write().await;
        if *is_running {
            return Ok(());
        }
        
        *is_running = true;
        info!("Starting market data manager");
        
        // Initialize market data for all configured markets
        self.initialize_markets().await?;
        
        // Start data aggregation loop
        self.start_aggregation_loop().await;
        
        Ok(())
    }
    
    /// Stop market data manager
    pub async fn stop(&self) -> Result<()> {
        let mut is_running = self.is_running.write().await;
        if !*is_running {
            return Ok(());
        }
        
        *is_running = false;
        info!("Stopping market data manager");
        Ok(())
    }
    
    /// Initialize market data for all configured markets
    async fn initialize_markets(&self) -> Result<()> {
        let mut market_data = self.market_data.write().await;
        
        for (dex_name, dex_config) in &self.dex_configs {
            if !dex_config.enabled {
                continue;
            }
            
            let dex_id = DexId::new(
                dex_name.clone(),
                dex_config.program_pubkey()?,
            );
            
            for pair_str in &dex_config.supported_pairs {
                if let Some((base, quote)) = pair_str.split_once('/') {
                    let token_pair = TokenPair::new(base.to_string(), quote.to_string());
                    let market_id = MarketId::new(dex_id.clone(), token_pair);
                    
                    market_data.insert(market_id.clone(), MarketData::new(market_id));
                }
            }
        }
        
        info!("Initialized {} markets", market_data.len());
        Ok(())
    }
    
    /// Start the data aggregation loop
    async fn start_aggregation_loop(&self) {
        let market_data = self.market_data.clone();
        let is_running = self.is_running.clone();
        let update_interval_ms = self.update_interval_ms;
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(
                std::time::Duration::from_millis(update_interval_ms)
            );
            
            while *is_running.read().await {
                interval.tick().await;
                
                // TODO: Implement actual data aggregation
                // This would involve:
                // 1. Collecting data from price feeds
                // 2. Collecting data from order books
                // 3. Collecting data from pool data
                // 4. Aggregating and updating market data
                
                debug!("Aggregating market data");
                
                // Update data quality for all markets
                Self::update_data_quality(&market_data).await;
            }
        });
    }
    
    /// Update data quality for all markets
    async fn update_data_quality(market_data: &Arc<RwLock<HashMap<MarketId, MarketData>>>) {
        let mut data = market_data.write().await;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        
        for market in data.values_mut() {
            let age_ms = now - market.last_update;
            
            market.data_quality = match age_ms {
                0..=5000 => DataQuality::High,      // Less than 5 seconds
                5001..=30000 => DataQuality::Medium, // 5-30 seconds
                30001..=300000 => DataQuality::Low,  // 30 seconds - 5 minutes
                _ => DataQuality::Unknown,           // Older than 5 minutes
            };
        }
    }
    
    /// Get market data for a specific market
    pub async fn get_market_data(&self, market_id: &MarketId) -> Option<MarketData> {
        let market_data = self.market_data.read().await;
        market_data.get(market_id).cloned()
    }
    
    /// Get all market data
    pub async fn get_all_market_data(&self) -> HashMap<MarketId, MarketData> {
        self.market_data.read().await.clone()
    }
    
    /// Get market data for a specific token pair across all DEXs
    pub async fn get_market_data_for_pair(&self, pair: &TokenPair) -> HashMap<DexId, MarketData> {
        let market_data = self.market_data.read().await;
        let mut result = HashMap::new();
        
        for (market_id, data) in market_data.iter() {
            if market_id.pair == *pair {
                result.insert(market_id.dex.clone(), data.clone());
            }
        }
        
        result
    }
    
    /// Update market data for a specific market
    pub async fn update_market_data(&self, market_id: MarketId, data: MarketData) {
        let mut market_data = self.market_data.write().await;
        market_data.insert(market_id, data);
    }
    
    /// Update price for a specific market
    pub async fn update_price(&self, market_id: &MarketId, price: Decimal) {
        let mut market_data = self.market_data.write().await;
        if let Some(data) = market_data.get_mut(market_id) {
            data.current_price = Some(price);
            data.touch();
        }
    }
    
    /// Update bid/ask for a specific market
    pub async fn update_bid_ask(&self, market_id: &MarketId, bid: Option<Decimal>, ask: Option<Decimal>) {
        let mut market_data = self.market_data.write().await;
        if let Some(data) = market_data.get_mut(market_id) {
            data.bid_price = bid;
            data.ask_price = ask;
            data.update_spread();
            data.touch();
        }
    }
    
    /// Update volume for a specific market
    pub async fn update_volume(&self, market_id: &MarketId, volume_24h: Decimal) {
        let mut market_data = self.market_data.write().await;
        if let Some(data) = market_data.get_mut(market_id) {
            data.volume_24h = Some(volume_24h);
            data.touch();
        }
    }
    
    /// Get markets with high quality data
    pub async fn get_high_quality_markets(&self) -> Vec<MarketId> {
        let market_data = self.market_data.read().await;
        market_data.iter()
            .filter(|(_, data)| data.data_quality == DataQuality::High)
            .map(|(market_id, _)| market_id.clone())
            .collect()
    }
    
    /// Get best price for a token pair (across all DEXs)
    pub async fn get_best_price(&self, pair: &TokenPair, is_buy: bool) -> Option<(DexId, Decimal)> {
        let pair_data = self.get_market_data_for_pair(pair).await;
        
        let mut best_price: Option<Decimal> = None;
        let mut best_dex: Option<DexId> = None;
        
        for (dex_id, data) in pair_data {
            if data.data_quality == DataQuality::Unknown {
                continue;
            }
            
            let price = if is_buy {
                data.ask_price.or(data.current_price)
            } else {
                data.bid_price.or(data.current_price)
            };
            
            if let Some(p) = price {
                match best_price {
                    None => {
                        best_price = Some(p);
                        best_dex = Some(dex_id);
                    }
                    Some(best) => {
                        if (is_buy && p < best) || (!is_buy && p > best) {
                            best_price = Some(p);
                            best_dex = Some(dex_id);
                        }
                    }
                }
            }
        }
        
        best_dex.map(|dex| (dex, best_price.unwrap()))
    }
    
    /// Clean up stale market data
    pub async fn cleanup_stale_data(&self, max_age_ms: u64) {
        let mut market_data = self.market_data.write().await;
        let mut to_remove = Vec::new();
        
        for (market_id, data) in market_data.iter() {
            if data.is_stale(max_age_ms) {
                to_remove.push(market_id.clone());
            }
        }
        
        for market_id in to_remove {
            market_data.remove(&market_id);
            warn!("Removed stale market data for: {}", market_id);
        }
    }
    
    /// Get market data statistics
    pub async fn get_stats(&self) -> MarketDataStats {
        let market_data = self.market_data.read().await;
        let total_markets = market_data.len();
        
        let mut high_quality = 0;
        let mut medium_quality = 0;
        let mut low_quality = 0;
        let mut unknown_quality = 0;
        
        for data in market_data.values() {
            match data.data_quality {
                DataQuality::High => high_quality += 1,
                DataQuality::Medium => medium_quality += 1,
                DataQuality::Low => low_quality += 1,
                DataQuality::Unknown => unknown_quality += 1,
            }
        }
        
        MarketDataStats {
            total_markets,
            high_quality_markets: high_quality,
            medium_quality_markets: medium_quality,
            low_quality_markets: low_quality,
            unknown_quality_markets: unknown_quality,
        }
    }
}

/// Market data statistics
#[derive(Debug, Clone)]
pub struct MarketDataStats {
    pub total_markets: usize,
    pub high_quality_markets: usize,
    pub medium_quality_markets: usize,
    pub low_quality_markets: usize,
    pub unknown_quality_markets: usize,
}

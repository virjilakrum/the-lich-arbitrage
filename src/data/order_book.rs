//! Order book management for DEX liquidity data

use crate::data::{MarketId, TokenPair};
use crate::error::{LichError, Result};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Order book level (price and size)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBookLevel {
    pub price: Decimal,
    pub size: Decimal,
    pub timestamp: u64,
}

impl OrderBookLevel {
    pub fn new(price: Decimal, size: Decimal) -> Self {
        Self {
            price,
            size,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        }
    }
}

/// Order book for a specific market
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBook {
    pub market_id: MarketId,
    pub bids: BTreeMap<String, OrderBookLevel>, // price as string key for ordering
    pub asks: BTreeMap<String, OrderBookLevel>, // price as string key for ordering
    pub last_update: u64,
    pub sequence: u64,
}

impl OrderBook {
    pub fn new(market_id: MarketId) -> Self {
        Self {
            market_id,
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            last_update: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            sequence: 0,
        }
    }
    
    /// Update a bid level
    pub fn update_bid(&mut self, price: Decimal, size: Decimal) {
        let price_key = format!("{:.18}", price); // Use high precision for ordering
        
        if size == Decimal::ZERO {
            self.bids.remove(&price_key);
        } else {
            self.bids.insert(price_key, OrderBookLevel::new(price, size));
        }
        
        self.last_update = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        self.sequence += 1;
    }
    
    /// Update an ask level
    pub fn update_ask(&mut self, price: Decimal, size: Decimal) {
        let price_key = format!("{:.18}", price); // Use high precision for ordering
        
        if size == Decimal::ZERO {
            self.asks.remove(&price_key);
        } else {
            self.asks.insert(price_key, OrderBookLevel::new(price, size));
        }
        
        self.last_update = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        self.sequence += 1;
    }
    
    /// Get best bid (highest buy price)
    pub fn best_bid(&self) -> Option<&OrderBookLevel> {
        self.bids.values().rev().next() // Reverse to get highest price
    }
    
    /// Get best ask (lowest sell price)
    pub fn best_ask(&self) -> Option<&OrderBookLevel> {
        self.asks.values().next() // First is lowest price
    }
    
    /// Get mid price
    pub fn mid_price(&self) -> Option<Decimal> {
        match (self.best_bid(), self.best_ask()) {
            (Some(bid), Some(ask)) => Some((bid.price + ask.price) / Decimal::from(2)),
            _ => None,
        }
    }
    
    /// Get spread
    pub fn spread(&self) -> Option<Decimal> {
        match (self.best_bid(), self.best_ask()) {
            (Some(bid), Some(ask)) => Some(ask.price - bid.price),
            _ => None,
        }
    }
    
    /// Get spread percentage
    pub fn spread_percentage(&self) -> Option<Decimal> {
        match (self.spread(), self.mid_price()) {
            (Some(spread), Some(mid)) if mid > Decimal::ZERO => {
                Some(spread / mid * Decimal::from(100))
            }
            _ => None,
        }
    }
    
    /// Get top N bid levels
    pub fn top_bids(&self, n: usize) -> Vec<&OrderBookLevel> {
        self.bids.values().rev().take(n).collect()
    }
    
    /// Get top N ask levels
    pub fn top_asks(&self, n: usize) -> Vec<&OrderBookLevel> {
        self.asks.values().take(n).collect()
    }
    
    /// Calculate total liquidity up to a certain depth
    pub fn liquidity_depth(&self, depth: usize) -> (Decimal, Decimal) {
        let bid_liquidity: Decimal = self.top_bids(depth)
            .iter()
            .map(|level| level.size)
            .sum();
        
        let ask_liquidity: Decimal = self.top_asks(depth)
            .iter()
            .map(|level| level.size)
            .sum();
        
        (bid_liquidity, ask_liquidity)
    }
    
    /// Calculate price impact for a given trade size
    pub fn price_impact(&self, trade_size: Decimal, is_buy: bool) -> Option<Decimal> {
        let levels = if is_buy {
            self.asks.values().collect::<Vec<_>>()
        } else {
            self.bids.values().rev().collect::<Vec<_>>()
        };
        
        if levels.is_empty() {
            return None;
        }
        
        let start_price = levels[0].price;
        let mut remaining_size = trade_size;
        let mut total_cost = Decimal::ZERO;
        
        for level in levels {
            if remaining_size <= Decimal::ZERO {
                break;
            }
            
            let trade_amount = remaining_size.min(level.size);
            total_cost += trade_amount * level.price;
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
    
    /// Check if order book is stale
    pub fn is_stale(&self, max_age_ms: u64) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        
        now - self.last_update > max_age_ms
    }
    
    /// Clear all levels
    pub fn clear(&mut self) {
        self.bids.clear();
        self.asks.clear();
        self.last_update = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        self.sequence += 1;
    }
}

/// Order book manager for handling multiple market order books
pub struct OrderBookManager {
    order_books: Arc<RwLock<HashMap<MarketId, OrderBook>>>,
    is_running: Arc<RwLock<bool>>,
}

impl OrderBookManager {
    pub async fn new() -> Result<Self> {
        Ok(Self {
            order_books: Arc::new(RwLock::new(HashMap::new())),
            is_running: Arc::new(RwLock::new(false)),
        })
    }
    
    /// Start order book manager
    pub async fn start(&self) -> Result<()> {
        let mut is_running = self.is_running.write().await;
        if *is_running {
            return Ok(());
        }
        
        *is_running = true;
        info!("Starting order book manager");
        
        // Start order book update loop
        self.start_update_loop().await;
        
        Ok(())
    }
    
    /// Stop order book manager
    pub async fn stop(&self) -> Result<()> {
        let mut is_running = self.is_running.write().await;
        if !*is_running {
            return Ok(());
        }
        
        *is_running = false;
        info!("Stopping order book manager");
        Ok(())
    }
    
    /// Start the order book update loop
    async fn start_update_loop(&self) {
        let order_books = self.order_books.clone();
        let is_running = self.is_running.clone();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_millis(100)); // Update every 100ms
            
            while *is_running.read().await {
                interval.tick().await;
                
                // TODO: Implement actual order book updates from DEX data
                // This would involve:
                // 1. Subscribing to DEX order book updates
                // 2. Processing incoming order book data
                // 3. Updating the cached order books
                
                debug!("Updating order books");
            }
        });
    }
    
    /// Get order book for a market
    pub async fn get_order_book(&self, market_id: &MarketId) -> Option<OrderBook> {
        let order_books = self.order_books.read().await;
        order_books.get(market_id).cloned()
    }
    
    /// Update order book for a market
    pub async fn update_order_book(&self, market_id: MarketId, order_book: OrderBook) {
        let mut order_books = self.order_books.write().await;
        order_books.insert(market_id, order_book);
    }
    
    /// Update a specific bid level
    pub async fn update_bid(&self, market_id: &MarketId, price: Decimal, size: Decimal) {
        let mut order_books = self.order_books.write().await;
        let order_book = order_books.entry(market_id.clone())
            .or_insert_with(|| OrderBook::new(market_id.clone()));
        
        order_book.update_bid(price, size);
    }
    
    /// Update a specific ask level
    pub async fn update_ask(&self, market_id: &MarketId, price: Decimal, size: Decimal) {
        let mut order_books = self.order_books.write().await;
        let order_book = order_books.entry(market_id.clone())
            .or_insert_with(|| OrderBook::new(market_id.clone()));
        
        order_book.update_ask(price, size);
    }
    
    /// Get all order books
    pub async fn get_all_order_books(&self) -> HashMap<MarketId, OrderBook> {
        self.order_books.read().await.clone()
    }
    
    /// Get order books for a specific token pair
    pub async fn get_order_books_for_pair(&self, pair: &TokenPair) -> HashMap<MarketId, OrderBook> {
        let order_books = self.order_books.read().await;
        let mut result = HashMap::new();
        
        for (market_id, order_book) in order_books.iter() {
            if market_id.pair == *pair {
                result.insert(market_id.clone(), order_book.clone());
            }
        }
        
        result
    }
    
    /// Remove stale order books
    pub async fn cleanup_stale_books(&self, max_age_ms: u64) {
        let mut order_books = self.order_books.write().await;
        let mut to_remove = Vec::new();
        
        for (market_id, order_book) in order_books.iter() {
            if order_book.is_stale(max_age_ms) {
                to_remove.push(market_id.clone());
            }
        }
        
        for market_id in to_remove {
            order_books.remove(&market_id);
            warn!("Removed stale order book for market: {}", market_id);
        }
    }
    
    /// Get order book statistics
    pub async fn get_stats(&self) -> OrderBookStats {
        let order_books = self.order_books.read().await;
        let total_books = order_books.len();
        
        let mut fresh_books = 0;
        let mut total_bid_levels = 0;
        let mut total_ask_levels = 0;
        let max_age_ms = 60000; // 1 minute
        
        for order_book in order_books.values() {
            if !order_book.is_stale(max_age_ms) {
                fresh_books += 1;
            }
            total_bid_levels += order_book.bids.len();
            total_ask_levels += order_book.asks.len();
        }
        
        OrderBookStats {
            total_books,
            fresh_books,
            stale_books: total_books - fresh_books,
            total_bid_levels,
            total_ask_levels,
        }
    }
}

/// Order book statistics
#[derive(Debug, Clone)]
pub struct OrderBookStats {
    pub total_books: usize,
    pub fresh_books: usize,
    pub stale_books: usize,
    pub total_bid_levels: usize,
    pub total_ask_levels: usize,
}

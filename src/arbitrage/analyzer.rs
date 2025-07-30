//! Arbitrage analysis algorithms and logic

use crate::arbitrage::{ArbitrageOpportunity, OpportunityType};
use crate::arbitrage::opportunity::ArbitrageStep;
use crate::config::{DexConfig, StrategyConfig, TokenConfig};
use crate::data::{DexId, MarketId, MarketPrice, TokenPair};
use crate::error::{LichError, Result};
use rust_decimal::Decimal;
use solana_sdk::pubkey::Pubkey;
use std::collections::HashMap;
use tracing::{debug, warn};

/// Analysis result for arbitrage opportunities
#[derive(Debug, Clone)]
pub struct AnalysisResult {
    pub opportunities: Vec<ArbitrageOpportunity>,
    pub analysis_time_ms: u64,
    pub markets_analyzed: usize,
}

/// Arbitrage analyzer for detecting opportunities
pub struct ArbitrageAnalyzer {
    strategy_config: StrategyConfig,
    dex_configs: HashMap<String, DexConfig>,
    token_configs: HashMap<String, TokenConfig>,
}

impl ArbitrageAnalyzer {
    /// Create a new arbitrage analyzer
    pub async fn new(
        strategy_config: StrategyConfig,
        dex_configs: HashMap<String, DexConfig>,
        token_configs: HashMap<String, TokenConfig>,
    ) -> Result<Self> {
        Ok(Self {
            strategy_config,
            dex_configs,
            token_configs,
        })
    }
    
    /// Analyze simple arbitrage opportunities for a token pair
    pub async fn analyze_simple_arbitrage(
        &self,
        token_pair: &TokenPair,
        pair_prices: &HashMap<DexId, MarketPrice>,
    ) -> Result<Option<ArbitrageOpportunity>> {
        if pair_prices.len() < 2 {
            return Ok(None);
        }
        
        let mut best_opportunity: Option<ArbitrageOpportunity> = None;
        let mut best_profit = Decimal::ZERO;
        
        // Compare all DEX pairs
        for (buy_dex, buy_price_data) in pair_prices {
            for (sell_dex, sell_price_data) in pair_prices {
                if buy_dex == sell_dex {
                    continue;
                }
                
                if let Some(opportunity) = self.calculate_simple_arbitrage(
                    token_pair,
                    buy_dex,
                    sell_dex,
                    buy_price_data,
                    sell_price_data,
                ).await? {
                    if opportunity.net_profit_percentage > best_profit {
                        best_profit = opportunity.net_profit_percentage;
                        best_opportunity = Some(opportunity);
                    }
                }
            }
        }
        
        Ok(best_opportunity)
    }
    
    /// Calculate simple arbitrage opportunity between two DEXs
    async fn calculate_simple_arbitrage(
        &self,
        token_pair: &TokenPair,
        buy_dex: &DexId,
        sell_dex: &DexId,
        buy_price_data: &MarketPrice,
        sell_price_data: &MarketPrice,
    ) -> Result<Option<ArbitrageOpportunity>> {
        // Get buy price (ask price on buy DEX)
        let buy_price = buy_price_data.ask.or(buy_price_data.last_price);
        
        // Get sell price (bid price on sell DEX)
        let sell_price = sell_price_data.bid.or(sell_price_data.last_price);
        
        let (buy_price, sell_price) = match (buy_price, sell_price) {
            (Some(bp), Some(sp)) => (bp, sp),
            _ => return Ok(None), // No valid prices
        };
        
        // Check if there's a profit opportunity
        if sell_price <= buy_price {
            return Ok(None); // No profit
        }
        
        // Calculate profit percentage
        let profit_percentage = (sell_price - buy_price) / buy_price * Decimal::from(100);
        
        // Check if it meets minimum profit threshold
        if profit_percentage < self.strategy_config.min_profit_threshold * Decimal::from(100) {
            return Ok(None);
        }
        
        // Calculate trade amounts and fees
        let trade_amount = self.strategy_config.default_trade_amount;
        let expected_profit = trade_amount * profit_percentage / Decimal::from(100);
        
        // Estimate transaction fees
        let estimated_fees = self.estimate_transaction_fees(buy_dex, sell_dex).await?;
        
        // Create opportunity
        let opportunity_type = OpportunityType::Simple {
            buy_dex: buy_dex.clone(),
            sell_dex: sell_dex.clone(),
            token_pair: token_pair.clone(),
        };
        
        let mut opportunity = ArbitrageOpportunity::new(
            opportunity_type,
            expected_profit,
            profit_percentage,
            trade_amount,
            estimated_fees,
        );
        
        // Add metadata
        opportunity.add_metadata("buy_price".to_string(), serde_json::json!(buy_price));
        opportunity.add_metadata("sell_price".to_string(), serde_json::json!(sell_price));
        opportunity.add_metadata("buy_dex".to_string(), serde_json::json!(buy_dex.name));
        opportunity.add_metadata("sell_dex".to_string(), serde_json::json!(sell_dex.name));
        
        debug!(
            "Found simple arbitrage opportunity: {} -> {} | Profit: {:.4}%",
            buy_dex.name, sell_dex.name, profit_percentage
        );
        
        Ok(Some(opportunity))
    }
    
    /// Analyze triangular arbitrage opportunities
    pub async fn analyze_triangular_arbitrage(
        &self,
        _base_pair: &TokenPair,
        _market_data: &HashMap<MarketId, MarketPrice>,
    ) -> Result<Vec<ArbitrageOpportunity>> {
        // TODO: Implement triangular arbitrage analysis
        // This would involve:
        // 1. Finding all possible triangular paths (A -> B -> C -> A)
        // 2. Calculating profit for each path
        // 3. Considering transaction fees and slippage
        // 4. Creating opportunities for profitable paths
        
        debug!("Triangular arbitrage analysis not yet implemented");
        Ok(Vec::new())
    }
    
    /// Estimate transaction fees for an arbitrage opportunity
    async fn estimate_transaction_fees(&self, buy_dex: &DexId, sell_dex: &DexId) -> Result<Decimal> {
        let mut total_fees = Decimal::ZERO;
        
        // Get DEX-specific fees
        if let Some(buy_dex_config) = self.dex_configs.get(&buy_dex.name) {
            total_fees += Decimal::from_f64_retain(buy_dex_config.fees.trading_fee)
                .unwrap_or(Decimal::ZERO);
        }
        
        if let Some(sell_dex_config) = self.dex_configs.get(&sell_dex.name) {
            total_fees += Decimal::from_f64_retain(sell_dex_config.fees.trading_fee)
                .unwrap_or(Decimal::ZERO);
        }
        
        // Add Solana network fees (estimated)
        let network_fees = Decimal::from_parts(1, 0, 0, false, 2); // 0.01 SOL estimated
        total_fees += network_fees;
        
        Ok(total_fees)
    }
    
    /// Check if a token pair has sufficient liquidity
    async fn check_liquidity(
        &self,
        _token_pair: &TokenPair,
        _trade_amount: Decimal,
    ) -> Result<bool> {
        // TODO: Implement liquidity checking
        // This would involve:
        // 1. Getting order book data for the pair
        // 2. Calculating available liquidity at current prices
        // 3. Checking if trade amount can be executed without excessive slippage
        
        Ok(true) // Placeholder
    }
    
    /// Calculate price impact for a trade
    async fn calculate_price_impact(
        &self,
        _market_id: &MarketId,
        _trade_amount: Decimal,
        _is_buy: bool,
    ) -> Result<Decimal> {
        // TODO: Implement price impact calculation
        // This would involve:
        // 1. Getting order book data
        // 2. Simulating the trade execution
        // 3. Calculating the average execution price
        // 4. Comparing with current market price
        
        Ok(Decimal::ZERO) // Placeholder
    }
    
    /// Validate an arbitrage opportunity
    pub async fn validate_opportunity(
        &self,
        opportunity: &ArbitrageOpportunity,
    ) -> Result<bool> {
        // Check if opportunity is still valid
        if !opportunity.is_valid() {
            return Ok(false);
        }
        
        // Check if it meets profit threshold
        if !opportunity.meets_profit_threshold(
            self.strategy_config.min_profit_threshold * Decimal::from(100)
        ) {
            return Ok(false);
        }
        
        // Check liquidity (if implemented)
        match &opportunity.opportunity_type {
            OpportunityType::Simple { token_pair, .. } => {
                if !self.check_liquidity(token_pair, opportunity.required_capital).await? {
                    return Ok(false);
                }
            }
            OpportunityType::Triangular { .. } => {
                // TODO: Implement triangular arbitrage liquidity checking
            }
            OpportunityType::CrossChain { .. } => {
                // TODO: Implement cross-chain arbitrage validation
            }
        }
        
        Ok(true)
    }
    
    /// Get analyzer statistics
    pub fn get_stats(&self) -> AnalyzerStats {
        AnalyzerStats {
            configured_dexes: self.dex_configs.len(),
            configured_tokens: self.token_configs.len(),
            monitored_pairs: self.strategy_config.monitored_pairs.len(),
            min_profit_threshold: self.strategy_config.min_profit_threshold,
            max_slippage_tolerance: self.strategy_config.max_slippage_tolerance,
            triangular_arbitrage_enabled: self.strategy_config.enable_triangular_arbitrage,
        }
    }
}

/// Analyzer statistics
#[derive(Debug, Clone)]
pub struct AnalyzerStats {
    pub configured_dexes: usize,
    pub configured_tokens: usize,
    pub monitored_pairs: usize,
    pub min_profit_threshold: Decimal,
    pub max_slippage_tolerance: Decimal,
    pub triangular_arbitrage_enabled: bool,
}

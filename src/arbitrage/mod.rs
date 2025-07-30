//! Arbitrage opportunity detection and analysis

use crate::config::{Config, StrategyConfig};
use crate::data::{DataManager, DexId, MarketId, TokenPair};
use crate::error::{LichError, Result};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

pub mod opportunity;
pub mod analyzer;
pub mod strategy;

pub use opportunity::{ArbitrageOpportunity, OpportunityType, OpportunityStatus};
pub use analyzer::{ArbitrageAnalyzer, AnalysisResult};
pub use strategy::{ArbitrageStrategy, StrategyType};

/// Arbitrage engine for detecting and analyzing opportunities
pub struct ArbitrageEngine {
    /// Data manager for market data
    data_manager: Arc<DataManager>,
    
    /// Arbitrage analyzer
    analyzer: Arc<ArbitrageAnalyzer>,
    
    /// Active opportunities
    opportunities: Arc<RwLock<HashMap<String, ArbitrageOpportunity>>>,
    
    /// Strategy configuration
    strategy_config: StrategyConfig,
    
    /// Engine state
    is_running: Arc<RwLock<bool>>,
    
    /// Analysis interval in milliseconds
    analysis_interval_ms: u64,
}

impl ArbitrageEngine {
    /// Create a new arbitrage engine
    pub async fn new(
        data_manager: Arc<DataManager>,
        config: &Config,
    ) -> Result<Self> {
        let analyzer = Arc::new(ArbitrageAnalyzer::new(
            config.strategy.clone(),
            config.dexes.clone(),
            config.tokens.clone(),
        ).await?);
        
        Ok(Self {
            data_manager,
            analyzer,
            opportunities: Arc::new(RwLock::new(HashMap::new())),
            strategy_config: config.strategy.clone(),
            is_running: Arc::new(RwLock::new(false)),
            analysis_interval_ms: 100, // Analyze every 100ms for high frequency
        })
    }
    
    /// Start the arbitrage engine
    pub async fn start(&self) -> Result<()> {
        let mut is_running = self.is_running.write().await;
        if *is_running {
            return Ok(());
        }
        
        *is_running = true;
        info!("Starting arbitrage engine");
        
        // Start the analysis loop
        self.start_analysis_loop().await;
        
        // Start opportunity cleanup loop
        self.start_cleanup_loop().await;
        
        info!("Arbitrage engine started");
        Ok(())
    }
    
    /// Stop the arbitrage engine
    pub async fn stop(&self) -> Result<()> {
        let mut is_running = self.is_running.write().await;
        if !*is_running {
            return Ok(());
        }
        
        *is_running = false;
        info!("Arbitrage engine stopped");
        Ok(())
    }
    
    /// Start the main analysis loop
    async fn start_analysis_loop(&self) {
        let data_manager = self.data_manager.clone();
        let analyzer = self.analyzer.clone();
        let opportunities = self.opportunities.clone();
        let is_running = self.is_running.clone();
        let analysis_interval_ms = self.analysis_interval_ms;
        let strategy_config = self.strategy_config.clone();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(
                std::time::Duration::from_millis(analysis_interval_ms)
            );
            
            while *is_running.read().await {
                interval.tick().await;
                
                match Self::analyze_opportunities(
                    &data_manager,
                    &analyzer,
                    &opportunities,
                    &strategy_config,
                ).await {
                    Ok(count) => {
                        if count > 0 {
                            debug!("Found {} arbitrage opportunities", count);
                        }
                    }
                    Err(e) => {
                        error!("Error analyzing opportunities: {}", e);
                    }
                }
            }
        });
    }
    
    /// Start the opportunity cleanup loop
    async fn start_cleanup_loop(&self) {
        let opportunities = self.opportunities.clone();
        let is_running = self.is_running.clone();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(10));
            
            while *is_running.read().await {
                interval.tick().await;
                
                Self::cleanup_stale_opportunities(&opportunities).await;
            }
        });
    }
    
    /// Analyze current market data for arbitrage opportunities
    async fn analyze_opportunities(
        data_manager: &Arc<DataManager>,
        analyzer: &Arc<ArbitrageAnalyzer>,
        opportunities: &Arc<RwLock<HashMap<String, ArbitrageOpportunity>>>,
        strategy_config: &StrategyConfig,
    ) -> Result<usize> {
        let market_data = data_manager.get_all_market_prices().await;
        
        if market_data.is_empty() {
            return Ok(0);
        }
        
        let mut new_opportunities = Vec::new();
        
        // Analyze each monitored token pair
        for pair_config in &strategy_config.monitored_pairs {
            let token_pair = TokenPair::new(pair_config.base.clone(), pair_config.quote.clone());
            
            // Get prices for this pair across all DEXs
            let pair_prices = data_manager.get_prices_for_pair(&token_pair).await;
            
            if pair_prices.len() < 2 {
                continue; // Need at least 2 DEXs for arbitrage
            }
            
            // Analyze simple arbitrage opportunities
            if let Some(opportunity) = analyzer.analyze_simple_arbitrage(&token_pair, &pair_prices).await? {
                new_opportunities.push(opportunity);
            }
            
            // Analyze triangular arbitrage if enabled
            if strategy_config.enable_triangular_arbitrage {
                let triangular_opportunities = analyzer.analyze_triangular_arbitrage(&token_pair, &market_data).await?;
                new_opportunities.extend(triangular_opportunities);
            }
        }
        
        // Update opportunities
        let mut opportunities_map = opportunities.write().await;
        let count = new_opportunities.len();
        
        for opportunity in new_opportunities {
            opportunities_map.insert(opportunity.id.clone(), opportunity);
        }
        
        Ok(count)
    }
    
    /// Clean up stale opportunities
    async fn cleanup_stale_opportunities(
        opportunities: &Arc<RwLock<HashMap<String, ArbitrageOpportunity>>>,
    ) {
        let mut opportunities_map = opportunities.write().await;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        
        let mut to_remove = Vec::new();
        
        for (id, opportunity) in opportunities_map.iter() {
            // Remove opportunities older than 30 seconds
            if now - opportunity.timestamp > 30000 {
                to_remove.push(id.clone());
            }
        }
        
        for id in to_remove {
            opportunities_map.remove(&id);
        }
    }
    
    /// Get all current opportunities
    pub async fn get_opportunities(&self) -> Vec<ArbitrageOpportunity> {
        let opportunities = self.opportunities.read().await;
        opportunities.values().cloned().collect()
    }
    
    /// Get opportunities above a certain profit threshold
    pub async fn get_profitable_opportunities(&self, min_profit: Decimal) -> Vec<ArbitrageOpportunity> {
        let opportunities = self.opportunities.read().await;
        opportunities.values()
            .filter(|opp| opp.expected_profit_percentage >= min_profit)
            .cloned()
            .collect()
    }
    
    /// Get the best opportunity (highest profit)
    pub async fn get_best_opportunity(&self) -> Option<ArbitrageOpportunity> {
        let opportunities = self.opportunities.read().await;
        opportunities.values()
            .filter(|opp| opp.status == OpportunityStatus::Active)
            .max_by(|a, b| a.expected_profit_percentage.cmp(&b.expected_profit_percentage))
            .cloned()
    }
    
    /// Mark an opportunity as executed
    pub async fn mark_opportunity_executed(&self, opportunity_id: &str) -> Result<()> {
        let mut opportunities = self.opportunities.write().await;
        
        if let Some(opportunity) = opportunities.get_mut(opportunity_id) {
            opportunity.status = OpportunityStatus::Executed;
            opportunity.execution_timestamp = Some(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64
            );
            Ok(())
        } else {
            Err(LichError::Internal(format!(
                "Opportunity not found: {}", 
                opportunity_id
            )))
        }
    }
    
    /// Mark an opportunity as failed
    pub async fn mark_opportunity_failed(&self, opportunity_id: &str, reason: String) -> Result<()> {
        let mut opportunities = self.opportunities.write().await;
        
        if let Some(opportunity) = opportunities.get_mut(opportunity_id) {
            opportunity.status = OpportunityStatus::Failed;
            opportunity.failure_reason = Some(reason);
            Ok(())
        } else {
            Err(LichError::Internal(format!(
                "Opportunity not found: {}", 
                opportunity_id
            )))
        }
    }
    
    /// Get engine statistics
    pub async fn get_stats(&self) -> ArbitrageEngineStats {
        let opportunities = self.opportunities.read().await;
        
        let total_opportunities = opportunities.len();
        let mut active_opportunities = 0;
        let mut executed_opportunities = 0;
        let mut failed_opportunities = 0;
        let mut expired_opportunities = 0;
        
        let mut total_expected_profit = Decimal::ZERO;
        let mut best_profit = Decimal::ZERO;
        
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        
        for opportunity in opportunities.values() {
            match opportunity.status {
                OpportunityStatus::Active => {
                    if now - opportunity.timestamp > 30000 {
                        expired_opportunities += 1;
                    } else {
                        active_opportunities += 1;
                    }
                }
                OpportunityStatus::Executed => executed_opportunities += 1,
                OpportunityStatus::Failed => failed_opportunities += 1,
                OpportunityStatus::Expired => expired_opportunities += 1,
            }
            
            total_expected_profit += opportunity.expected_profit_percentage;
            
            if opportunity.expected_profit_percentage > best_profit {
                best_profit = opportunity.expected_profit_percentage;
            }
        }
        
        ArbitrageEngineStats {
            total_opportunities,
            active_opportunities,
            executed_opportunities,
            failed_opportunities,
            expired_opportunities,
            average_expected_profit: if total_opportunities > 0 {
                total_expected_profit / Decimal::from(total_opportunities)
            } else {
                Decimal::ZERO
            },
            best_profit_percentage: best_profit,
            is_running: *self.is_running.read().await,
        }
    }
    
    /// Force a manual analysis
    pub async fn force_analysis(&self) -> Result<usize> {
        if !*self.is_running.read().await {
            return Err(LichError::Internal("Engine is not running".to_string()));
        }
        
        Self::analyze_opportunities(
            &self.data_manager,
            &self.analyzer,
            &self.opportunities,
            &self.strategy_config,
        ).await
    }
}

/// Arbitrage engine statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArbitrageEngineStats {
    pub total_opportunities: usize,
    pub active_opportunities: usize,
    pub executed_opportunities: usize,
    pub failed_opportunities: usize,
    pub expired_opportunities: usize,
    pub average_expected_profit: Decimal,
    pub best_profit_percentage: Decimal,
    pub is_running: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, StrategyConfig, TokenPair as ConfigTokenPair};
    
    #[tokio::test]
    async fn test_arbitrage_engine_creation() {
        // This is a placeholder test
        // In a real implementation, you would create mock data managers and configs
        assert!(true);
    }
}

//! Arbitrage trading strategies

use crate::arbitrage::ArbitrageOpportunity;
use crate::error::{LichError, Result};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Types of arbitrage strategies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StrategyType {
    /// Conservative strategy - low risk, lower returns
    Conservative,
    /// Balanced strategy - medium risk, medium returns
    Balanced,
    /// Aggressive strategy - high risk, higher returns
    Aggressive,
    /// Custom strategy with specific parameters
    Custom {
        min_profit_threshold: Decimal,
        max_risk_level: String,
        max_slippage: Decimal,
    },
}

/// Arbitrage strategy configuration and execution logic
pub struct ArbitrageStrategy {
    strategy_type: StrategyType,
    parameters: StrategyParameters,
}

/// Strategy parameters
#[derive(Debug, Clone)]
pub struct StrategyParameters {
    pub min_profit_threshold: Decimal,
    pub max_slippage_tolerance: Decimal,
    pub max_position_size: Decimal,
    pub risk_tolerance: RiskTolerance,
    pub execution_speed: ExecutionSpeed,
    pub diversification_factor: Decimal,
}

/// Risk tolerance levels
#[derive(Debug, Clone)]
pub enum RiskTolerance {
    Low,
    Medium,
    High,
}

/// Execution speed preferences
#[derive(Debug, Clone)]
pub enum ExecutionSpeed {
    Fast,      // Execute immediately when opportunity is found
    Medium,    // Wait for better opportunities within a time window
    Patient,   // Wait for optimal opportunities
}

impl ArbitrageStrategy {
    /// Create a new strategy
    pub fn new(strategy_type: StrategyType) -> Self {
        let parameters = match &strategy_type {
            StrategyType::Conservative => StrategyParameters {
                min_profit_threshold: Decimal::from_parts(5, 0, 0, false, 1), // 0.5%
                max_slippage_tolerance: Decimal::from_parts(1, 0, 0, false, 3), // 0.1%
                max_position_size: Decimal::from(1000), // $1000
                risk_tolerance: RiskTolerance::Low,
                execution_speed: ExecutionSpeed::Patient,
                diversification_factor: Decimal::from_parts(2, 0, 0, false, 1), // 0.2 (20% max per opportunity)
            },
            StrategyType::Balanced => StrategyParameters {
                min_profit_threshold: Decimal::from_parts(2, 0, 0, false, 1), // 0.2%
                max_slippage_tolerance: Decimal::from_parts(3, 0, 0, false, 3), // 0.3%
                max_position_size: Decimal::from(5000), // $5000
                risk_tolerance: RiskTolerance::Medium,
                execution_speed: ExecutionSpeed::Medium,
                diversification_factor: Decimal::from_parts(3, 0, 0, false, 1), // 0.3 (30% max per opportunity)
            },
            StrategyType::Aggressive => StrategyParameters {
                min_profit_threshold: Decimal::from_parts(1, 0, 0, false, 1), // 0.1%
                max_slippage_tolerance: Decimal::from_parts(5, 0, 0, false, 3), // 0.5%
                max_position_size: Decimal::from(10000), // $10000
                risk_tolerance: RiskTolerance::High,
                execution_speed: ExecutionSpeed::Fast,
                diversification_factor: Decimal::from_parts(5, 0, 0, false, 1), // 0.5 (50% max per opportunity)
            },
            StrategyType::Custom { min_profit_threshold, max_slippage, .. } => StrategyParameters {
                min_profit_threshold: *min_profit_threshold,
                max_slippage_tolerance: *max_slippage,
                max_position_size: Decimal::from(5000), // Default
                risk_tolerance: RiskTolerance::Medium,
                execution_speed: ExecutionSpeed::Medium,
                diversification_factor: Decimal::from_parts(3, 0, 0, false, 1),
            },
        };
        
        Self {
            strategy_type,
            parameters,
        }
    }
    
    /// Evaluate if an opportunity should be executed based on strategy
    pub fn should_execute(&self, opportunity: &ArbitrageOpportunity) -> bool {
        // Check profit threshold
        if opportunity.net_profit_percentage < self.parameters.min_profit_threshold {
            return false;
        }
        
        // Check slippage tolerance
        if opportunity.max_slippage > self.parameters.max_slippage_tolerance {
            return false;
        }
        
        // Check position size
        if opportunity.required_capital > self.parameters.max_position_size {
            return false;
        }
        
        // Check risk tolerance
        match (&self.parameters.risk_tolerance, &opportunity.risk_level) {
            (RiskTolerance::Low, crate::arbitrage::opportunity::RiskLevel::Medium) => return false,
            (RiskTolerance::Low, crate::arbitrage::opportunity::RiskLevel::High) => return false,
            (RiskTolerance::Low, crate::arbitrage::opportunity::RiskLevel::VeryHigh) => return false,
            (RiskTolerance::Medium, crate::arbitrage::opportunity::RiskLevel::High) => return false,
            (RiskTolerance::Medium, crate::arbitrage::opportunity::RiskLevel::VeryHigh) => return false,
            (RiskTolerance::High, crate::arbitrage::opportunity::RiskLevel::VeryHigh) => {
                // Even aggressive strategy might avoid very high risk
                if opportunity.confidence_score < 30 {
                    return false;
                }
            }
            _ => {} // Acceptable risk level
        }
        
        // Check confidence score based on execution speed
        let min_confidence = match self.parameters.execution_speed {
            ExecutionSpeed::Fast => 40,
            ExecutionSpeed::Medium => 60,
            ExecutionSpeed::Patient => 80,
        };
        
        if opportunity.confidence_score < min_confidence {
            return false;
        }
        
        true
    }
    
    /// Calculate optimal position size for an opportunity
    pub fn calculate_position_size(
        &self,
        opportunity: &ArbitrageOpportunity,
        available_capital: Decimal,
    ) -> Decimal {
        // Start with strategy's max position size
        let mut position_size = self.parameters.max_position_size;
        
        // Apply diversification factor
        let max_diversified = available_capital * self.parameters.diversification_factor;
        position_size = position_size.min(max_diversified);
        
        // Adjust based on risk level
        let risk_multiplier = match opportunity.risk_level {
            crate::arbitrage::opportunity::RiskLevel::Low => Decimal::ONE,
            crate::arbitrage::opportunity::RiskLevel::Medium => Decimal::from_parts(8, 0, 0, false, 1), // 0.8
            crate::arbitrage::opportunity::RiskLevel::High => Decimal::from_parts(6, 0, 0, false, 1), // 0.6
            crate::arbitrage::opportunity::RiskLevel::VeryHigh => Decimal::from_parts(3, 0, 0, false, 1), // 0.3
        };
        
        position_size *= risk_multiplier;
        
        // Adjust based on confidence score
        let confidence_multiplier = Decimal::from(opportunity.confidence_score) / Decimal::from(100);
        position_size *= confidence_multiplier;
        
        // Ensure we don't exceed available capital
        position_size = position_size.min(available_capital);
        
        // Ensure minimum viable trade size
        let min_trade_size = Decimal::from(10); // $10 minimum
        if position_size < min_trade_size {
            return Decimal::ZERO; // Not worth trading
        }
        
        position_size
    }
    
    /// Prioritize opportunities based on strategy
    pub fn prioritize_opportunities(
        &self,
        opportunities: &mut Vec<ArbitrageOpportunity>,
    ) {
        opportunities.sort_by(|a, b| {
            // Primary sort: profit percentage (descending)
            let profit_cmp = b.net_profit_percentage.cmp(&a.net_profit_percentage);
            
            if profit_cmp != std::cmp::Ordering::Equal {
                return profit_cmp;
            }
            
            // Secondary sort: confidence score (descending)
            let confidence_cmp = b.confidence_score.cmp(&a.confidence_score);
            
            if confidence_cmp != std::cmp::Ordering::Equal {
                return confidence_cmp;
            }
            
            // Tertiary sort: risk level (ascending - lower risk first)
            let risk_score_a = match a.risk_level {
                crate::arbitrage::opportunity::RiskLevel::Low => 1,
                crate::arbitrage::opportunity::RiskLevel::Medium => 2,
                crate::arbitrage::opportunity::RiskLevel::High => 3,
                crate::arbitrage::opportunity::RiskLevel::VeryHigh => 4,
            };
            
            let risk_score_b = match b.risk_level {
                crate::arbitrage::opportunity::RiskLevel::Low => 1,
                crate::arbitrage::opportunity::RiskLevel::Medium => 2,
                crate::arbitrage::opportunity::RiskLevel::High => 3,
                crate::arbitrage::opportunity::RiskLevel::VeryHigh => 4,
            };
            
            risk_score_a.cmp(&risk_score_b)
        });
    }
    
    /// Get strategy parameters
    pub fn parameters(&self) -> &StrategyParameters {
        &self.parameters
    }
    
    /// Get strategy type
    pub fn strategy_type(&self) -> &StrategyType {
        &self.strategy_type
    }
    
    /// Update strategy parameters
    pub fn update_parameters(&mut self, new_parameters: StrategyParameters) {
        self.parameters = new_parameters;
    }
    
    /// Get strategy statistics
    pub fn get_stats(&self) -> StrategyStats {
        StrategyStats {
            strategy_type: self.strategy_type.clone(),
            min_profit_threshold: self.parameters.min_profit_threshold,
            max_slippage_tolerance: self.parameters.max_slippage_tolerance,
            max_position_size: self.parameters.max_position_size,
            risk_tolerance: format!("{:?}", self.parameters.risk_tolerance),
            execution_speed: format!("{:?}", self.parameters.execution_speed),
        }
    }
}

/// Strategy statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyStats {
    pub strategy_type: StrategyType,
    pub min_profit_threshold: Decimal,
    pub max_slippage_tolerance: Decimal,
    pub max_position_size: Decimal,
    pub risk_tolerance: String,
    pub execution_speed: String,
}

/// Strategy manager for handling multiple strategies
pub struct StrategyManager {
    strategies: HashMap<String, ArbitrageStrategy>,
    active_strategy: String,
}

impl StrategyManager {
    /// Create a new strategy manager
    pub fn new() -> Self {
        let mut strategies = HashMap::new();
        
        // Add default strategies
        strategies.insert(
            "conservative".to_string(),
            ArbitrageStrategy::new(StrategyType::Conservative),
        );
        strategies.insert(
            "balanced".to_string(),
            ArbitrageStrategy::new(StrategyType::Balanced),
        );
        strategies.insert(
            "aggressive".to_string(),
            ArbitrageStrategy::new(StrategyType::Aggressive),
        );
        
        Self {
            strategies,
            active_strategy: "balanced".to_string(),
        }
    }
    
    /// Add a custom strategy
    pub fn add_strategy(&mut self, name: String, strategy: ArbitrageStrategy) {
        self.strategies.insert(name, strategy);
    }
    
    /// Set active strategy
    pub fn set_active_strategy(&mut self, name: String) -> Result<()> {
        if self.strategies.contains_key(&name) {
            self.active_strategy = name;
            Ok(())
        } else {
            Err(LichError::Internal(format!("Strategy not found: {}", name)))
        }
    }
    
    /// Get active strategy
    pub fn get_active_strategy(&self) -> Option<&ArbitrageStrategy> {
        self.strategies.get(&self.active_strategy)
    }
    
    /// Get active strategy (mutable)
    pub fn get_active_strategy_mut(&mut self) -> Option<&mut ArbitrageStrategy> {
        self.strategies.get_mut(&self.active_strategy)
    }
    
    /// Get strategy by name
    pub fn get_strategy(&self, name: &str) -> Option<&ArbitrageStrategy> {
        self.strategies.get(name)
    }
    
    /// List all available strategies
    pub fn list_strategies(&self) -> Vec<String> {
        self.strategies.keys().cloned().collect()
    }
    
    /// Get active strategy name
    pub fn active_strategy_name(&self) -> &str {
        &self.active_strategy
    }
}

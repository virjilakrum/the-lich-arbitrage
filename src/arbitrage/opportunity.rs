//! Arbitrage opportunity definitions and structures

use crate::data::{DexId, MarketId, TokenPair};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

/// Types of arbitrage opportunities
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum OpportunityType {
    /// Simple arbitrage between two DEXs
    Simple {
        buy_dex: DexId,
        sell_dex: DexId,
        token_pair: TokenPair,
    },
    /// Triangular arbitrage across multiple token pairs
    Triangular {
        path: Vec<ArbitrageStep>,
        start_token: String,
        end_token: String,
    },
    /// Cross-chain arbitrage (future implementation)
    CrossChain {
        source_chain: String,
        target_chain: String,
        token_pair: TokenPair,
    },
}

/// A step in an arbitrage path
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ArbitrageStep {
    pub dex: DexId,
    pub market_id: MarketId,
    pub input_token: String,
    pub output_token: String,
    pub input_amount: Decimal,
    pub expected_output: Decimal,
    pub price: Decimal,
    pub fee_percentage: Decimal,
}

impl ArbitrageStep {
    pub fn new(
        dex: DexId,
        market_id: MarketId,
        input_token: String,
        output_token: String,
        input_amount: Decimal,
        expected_output: Decimal,
        price: Decimal,
        fee_percentage: Decimal,
    ) -> Self {
        Self {
            dex,
            market_id,
            input_token,
            output_token,
            input_amount,
            expected_output,
            price,
            fee_percentage,
        }
    }
}

/// Status of an arbitrage opportunity
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum OpportunityStatus {
    /// Opportunity is active and can be executed
    Active,
    /// Opportunity has been executed
    Executed,
    /// Opportunity execution failed
    Failed,
    /// Opportunity has expired
    Expired,
}

/// Risk level of an arbitrage opportunity
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    VeryHigh,
}

/// Arbitrage opportunity structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArbitrageOpportunity {
    /// Unique identifier for this opportunity
    pub id: String,
    
    /// Type of arbitrage opportunity
    pub opportunity_type: OpportunityType,
    
    /// Current status
    pub status: OpportunityStatus,
    
    /// Expected profit in base currency
    pub expected_profit: Decimal,
    
    /// Expected profit percentage
    pub expected_profit_percentage: Decimal,
    
    /// Required initial investment
    pub required_capital: Decimal,
    
    /// Estimated gas/transaction fees
    pub estimated_fees: Decimal,
    
    /// Net profit after fees
    pub net_profit: Decimal,
    
    /// Net profit percentage after fees
    pub net_profit_percentage: Decimal,
    
    /// Maximum slippage tolerance for this opportunity
    pub max_slippage: Decimal,
    
    /// Estimated price impact
    pub estimated_price_impact: Decimal,
    
    /// Risk level assessment
    pub risk_level: RiskLevel,
    
    /// Confidence score (0-100)
    pub confidence_score: u8,
    
    /// Time when opportunity was detected
    pub timestamp: u64,
    
    /// Time when opportunity expires (if not executed)
    pub expiry_timestamp: u64,
    
    /// Time when opportunity was executed (if applicable)
    pub execution_timestamp: Option<u64>,
    
    /// Failure reason (if applicable)
    pub failure_reason: Option<String>,
    
    /// Additional metadata
    pub metadata: std::collections::HashMap<String, serde_json::Value>,
}

impl ArbitrageOpportunity {
    /// Create a new arbitrage opportunity
    pub fn new(
        opportunity_type: OpportunityType,
        expected_profit: Decimal,
        expected_profit_percentage: Decimal,
        required_capital: Decimal,
        estimated_fees: Decimal,
    ) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        
        let net_profit = expected_profit - estimated_fees;
        let net_profit_percentage = if required_capital > Decimal::ZERO {
            net_profit / required_capital * Decimal::from(100)
        } else {
            Decimal::ZERO
        };
        
        Self {
            id: Uuid::new_v4().to_string(),
            opportunity_type,
            status: OpportunityStatus::Active,
            expected_profit,
            expected_profit_percentage,
            required_capital,
            estimated_fees,
            net_profit,
            net_profit_percentage,
            max_slippage: Decimal::from_parts(5, 0, 0, false, 3), // 0.5% default
            estimated_price_impact: Decimal::ZERO,
            risk_level: RiskLevel::Medium,
            confidence_score: 50,
            timestamp,
            expiry_timestamp: timestamp + 30000, // 30 seconds default expiry
            execution_timestamp: None,
            failure_reason: None,
            metadata: std::collections::HashMap::new(),
        }
    }
    
    /// Check if the opportunity is still valid (not expired)
    pub fn is_valid(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        
        self.status == OpportunityStatus::Active && now < self.expiry_timestamp
    }
    
    /// Check if the opportunity is profitable after fees
    pub fn is_profitable(&self) -> bool {
        self.net_profit > Decimal::ZERO
    }
    
    /// Check if the opportunity meets minimum profit threshold
    pub fn meets_profit_threshold(&self, min_profit_percentage: Decimal) -> bool {
        self.net_profit_percentage >= min_profit_percentage
    }
    
    /// Get the age of the opportunity in milliseconds
    pub fn age_ms(&self) -> u64 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        
        now - self.timestamp
    }
    
    /// Get time until expiry in milliseconds
    pub fn time_to_expiry_ms(&self) -> i64 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        
        self.expiry_timestamp as i64 - now as i64
    }
    
    /// Mark opportunity as expired
    pub fn mark_expired(&mut self) {
        self.status = OpportunityStatus::Expired;
    }
    
    /// Update confidence score based on various factors
    pub fn update_confidence_score(&mut self, factors: &ConfidenceFactors) {
        let mut score = 50; // Base score
        
        // Profit factor (higher profit = higher confidence)
        if self.net_profit_percentage > Decimal::from(5) {
            score += 20;
        } else if self.net_profit_percentage > Decimal::from(1) {
            score += 10;
        } else if self.net_profit_percentage < Decimal::from_parts(5, 0, 0, false, 1) { // 0.05%
            score -= 20;
        }
        
        // Risk factor (lower risk = higher confidence)
        match self.risk_level {
            RiskLevel::Low => score += 15,
            RiskLevel::Medium => score += 5,
            RiskLevel::High => score -= 10,
            RiskLevel::VeryHigh => score -= 25,
        }
        
        // Price impact factor (lower impact = higher confidence)
        if self.estimated_price_impact < Decimal::from_parts(1, 0, 0, false, 3) { // 0.1%
            score += 10;
        } else if self.estimated_price_impact > Decimal::from_parts(5, 0, 0, false, 2) { // 5%
            score -= 15;
        }
        
        // Data freshness factor
        if factors.data_age_ms < 1000 {
            score += 10;
        } else if factors.data_age_ms > 10000 {
            score -= 15;
        }
        
        // Liquidity factor
        if factors.has_sufficient_liquidity {
            score += 10;
        } else {
            score -= 20;
        }
        
        // Market volatility factor
        if factors.market_volatility > Decimal::from(10) {
            score -= 10;
        } else if factors.market_volatility < Decimal::from(2) {
            score += 5;
        }
        
        self.confidence_score = (score.max(0).min(100)) as u8;
    }
    
    /// Calculate risk level based on various factors
    pub fn calculate_risk_level(&mut self, factors: &RiskFactors) {
        let mut risk_score = 0;
        
        // Profit margin risk
        if self.net_profit_percentage < Decimal::from_parts(5, 0, 0, false, 1) { // 0.05%
            risk_score += 3;
        } else if self.net_profit_percentage < Decimal::from_parts(2, 0, 0, false, 2) { // 0.2%
            risk_score += 1;
        }
        
        // Price impact risk
        if self.estimated_price_impact > Decimal::from(5) {
            risk_score += 3;
        } else if self.estimated_price_impact > Decimal::from(1) {
            risk_score += 1;
        }
        
        // Slippage risk
        if self.max_slippage > Decimal::from(2) {
            risk_score += 2;
        } else if self.max_slippage > Decimal::from_parts(5, 0, 0, false, 1) { // 0.5%
            risk_score += 1;
        }
        
        // Market volatility risk
        if factors.market_volatility > Decimal::from(20) {
            risk_score += 3;
        } else if factors.market_volatility > Decimal::from(10) {
            risk_score += 1;
        }
        
        // Liquidity risk
        if !factors.has_sufficient_liquidity {
            risk_score += 2;
        }
        
        // Data freshness risk
        if factors.data_age_ms > 10000 {
            risk_score += 2;
        } else if factors.data_age_ms > 5000 {
            risk_score += 1;
        }
        
        self.risk_level = match risk_score {
            0..=2 => RiskLevel::Low,
            3..=5 => RiskLevel::Medium,
            6..=8 => RiskLevel::High,
            _ => RiskLevel::VeryHigh,
        };
    }
    
    /// Add metadata
    pub fn add_metadata(&mut self, key: String, value: serde_json::Value) {
        self.metadata.insert(key, value);
    }
    
    /// Get metadata
    pub fn get_metadata(&self, key: &str) -> Option<&serde_json::Value> {
        self.metadata.get(key)
    }
}

/// Factors for calculating confidence score
pub struct ConfidenceFactors {
    pub data_age_ms: u64,
    pub has_sufficient_liquidity: bool,
    pub market_volatility: Decimal,
}

/// Factors for calculating risk level
pub struct RiskFactors {
    pub market_volatility: Decimal,
    pub has_sufficient_liquidity: bool,
    pub data_age_ms: u64,
}

impl std::fmt::Display for ArbitrageOpportunity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.opportunity_type {
            OpportunityType::Simple { buy_dex, sell_dex, token_pair } => {
                write!(
                    f,
                    "Simple Arbitrage: {} -> {} on {} | Profit: {:.4}% | Capital: {} | Risk: {:?}",
                    token_pair,
                    token_pair,
                    format!("{} -> {}", buy_dex, sell_dex),
                    self.net_profit_percentage,
                    self.required_capital,
                    self.risk_level
                )
            }
            OpportunityType::Triangular { start_token, end_token, path } => {
                write!(
                    f,
                    "Triangular Arbitrage: {} -> {} ({} steps) | Profit: {:.4}% | Capital: {} | Risk: {:?}",
                    start_token,
                    end_token,
                    path.len(),
                    self.net_profit_percentage,
                    self.required_capital,
                    self.risk_level
                )
            }
            OpportunityType::CrossChain { source_chain, target_chain, token_pair } => {
                write!(
                    f,
                    "Cross-Chain Arbitrage: {} on {} -> {} | Profit: {:.4}% | Capital: {} | Risk: {:?}",
                    token_pair,
                    source_chain,
                    target_chain,
                    self.net_profit_percentage,
                    self.required_capital,
                    self.risk_level
                )
            }
        }
    }
}

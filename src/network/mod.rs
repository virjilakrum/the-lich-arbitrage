//! Network layer for Solana connections and communication

use crate::config::NetworkConfig;
use crate::error::{LichError, Result};
use solana_client::rpc_client::RpcClient;
use solana_client::rpc_config::RpcSendTransactionConfig;
use solana_sdk::commitment_config::{CommitmentConfig, CommitmentLevel};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

pub mod rpc_manager;
pub mod websocket;
pub mod health_monitor;

pub use rpc_manager::RpcManager;
pub use websocket::{WebSocketManager, WebSocketEvent};
pub use health_monitor::{HealthMonitor, EndpointHealth};

/// Network manager that handles all Solana network connections
pub struct NetworkManager {
    /// RPC client manager
    rpc_manager: Arc<RpcManager>,
    
    /// WebSocket manager
    websocket_manager: Arc<WebSocketManager>,
    
    /// Health monitor
    health_monitor: Arc<HealthMonitor>,
    
    /// Network configuration
    config: NetworkConfig,
}

impl NetworkManager {
    /// Create a new network manager
    pub async fn new(config: NetworkConfig) -> Result<Self> {
        info!("Initializing network manager with {} primary RPC endpoints", config.primary_rpc.len());
        
        // Create RPC manager
        let rpc_manager = Arc::new(RpcManager::new(
            config.primary_rpc.clone(),
            config.backup_rpc.clone(),
            Duration::from_millis(config.rpc_timeout_ms),
            config.max_concurrent_requests,
        ).await?);

        // Create WebSocket manager
        let websocket_manager = Arc::new(WebSocketManager::new(
            config.websocket_endpoints.clone(),
            Duration::from_secs(config.ws_reconnect_timeout_secs),
        ).await?);
        
        // Create health monitor
        let health_monitor = Arc::new(HealthMonitor::new(
            rpc_manager.clone(),
            websocket_manager.clone(),
        ));
        
        Ok(Self {
            rpc_manager,
            websocket_manager,
            health_monitor,
            config,
        })
    }
    
    /// Start the network manager
    pub async fn start(&self) -> Result<()> {
        info!("Starting network manager");
        
        // Start health monitoring
        self.health_monitor.start().await?;
        
        // Start WebSocket connections
        self.websocket_manager.start().await?;
        
        info!("Network manager started successfully");
        Ok(())
    }
    
    /// Stop the network manager
    pub async fn stop(&self) -> Result<()> {
        info!("Stopping network manager");
        
        // Stop health monitoring
        self.health_monitor.stop().await?;
        
        // Stop WebSocket connections
        self.websocket_manager.stop().await?;
        
        info!("Network manager stopped");
        Ok(())
    }
    
    /// Get the RPC manager
    pub fn rpc_manager(&self) -> Arc<RpcManager> {
        self.rpc_manager.clone()
    }
    
    /// Get the WebSocket manager
    pub fn websocket_manager(&self) -> Arc<WebSocketManager> {
        self.websocket_manager.clone()
    }
    
    /// Get the health monitor
    pub fn health_monitor(&self) -> Arc<HealthMonitor> {
        self.health_monitor.clone()
    }
    
    /// Get network configuration
    pub fn config(&self) -> &NetworkConfig {
        &self.config
    }
    
    /// Get commitment configuration
    pub fn commitment_config(&self) -> CommitmentConfig {
        let level = match self.config.commitment.as_str() {
            "processed" => CommitmentLevel::Processed,
            "confirmed" => CommitmentLevel::Confirmed,
            "finalized" => CommitmentLevel::Finalized,
            _ => {
                warn!("Unknown commitment level '{}', using 'confirmed'", self.config.commitment);
                CommitmentLevel::Confirmed
            }
        };
        
        CommitmentConfig { commitment: level }
    }
    
    /// Check if the network is healthy
    pub async fn is_healthy(&self) -> bool {
        self.health_monitor.is_healthy().await
    }
    
    /// Get network statistics
    pub async fn get_stats(&self) -> NetworkStats {
        let rpc_stats = self.rpc_manager.get_stats().await;
        let ws_stats = self.websocket_manager.get_stats().await;
        let health_stats = self.health_monitor.get_stats().await;
        
        NetworkStats {
            rpc_stats,
            websocket_stats: ws_stats,
            health_stats,
        }
    }
}

/// Network statistics
#[derive(Debug, Clone)]
pub struct NetworkStats {
    pub rpc_stats: rpc_manager::RpcStats,
    pub websocket_stats: websocket::WebSocketStats,
    pub health_stats: health_monitor::HealthStats,
}

/// RPC send transaction configuration with optimized settings
pub fn optimized_send_config() -> RpcSendTransactionConfig {
    RpcSendTransactionConfig {
        skip_preflight: false,
        preflight_commitment: Some(CommitmentLevel::Processed),
        encoding: None,
        max_retries: Some(3),
        min_context_slot: None,
    }
}

/// Create an optimized RPC client with custom settings
pub fn create_optimized_rpc_client(endpoint: &str, timeout: Duration) -> RpcClient {
    RpcClient::new_with_timeout_and_commitment(
        endpoint.to_string(),
        timeout,
        CommitmentConfig::confirmed(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::NetworkConfig;
    
    #[tokio::test]
    async fn test_network_manager_creation() {
        let config = NetworkConfig {
            primary_rpc: vec!["https://api.mainnet-beta.solana.com".to_string()],
            backup_rpc: vec![],
            websocket_endpoints: vec!["wss://api.mainnet-beta.solana.com".to_string()],
            rpc_timeout_ms: 10000,
            ws_reconnect_timeout_secs: 5,
            max_concurrent_requests: 100,
            commitment: "confirmed".to_string(),
        };
        
        let network_manager = NetworkManager::new(config).await;
        assert!(network_manager.is_ok());
    }
    
    #[test]
    fn test_commitment_config() {
        let config = NetworkConfig {
            primary_rpc: vec!["https://api.mainnet-beta.solana.com".to_string()],
            backup_rpc: vec![],
            websocket_endpoints: vec!["wss://api.mainnet-beta.solana.com".to_string()],
            rpc_timeout_ms: 10000,
            ws_reconnect_timeout_secs: 5,
            max_concurrent_requests: 100,
            commitment: "confirmed".to_string(),
        };

        // Test commitment config parsing
        let level = match config.commitment.as_str() {
            "processed" => CommitmentLevel::Processed,
            "confirmed" => CommitmentLevel::Confirmed,
            "finalized" => CommitmentLevel::Finalized,
            _ => CommitmentLevel::Confirmed,
        };

        let commitment = CommitmentConfig { commitment: level };
        assert_eq!(commitment.commitment, CommitmentLevel::Confirmed);
    }
}

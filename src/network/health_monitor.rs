//! Health monitoring for network connections

use crate::error::{LichError, Result};
use crate::network::{RpcManager, WebSocketManager};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::time::{interval, sleep};
use tracing::{debug, error, info, warn};

/// Health status for an endpoint
#[derive(Debug, Clone)]
pub struct EndpointHealth {
    pub url: String,
    pub is_healthy: bool,
    pub last_check: Instant,
    pub response_time_ms: Option<u64>,
    pub error_message: Option<String>,
}

/// Overall health statistics
#[derive(Debug, Clone)]
pub struct HealthStats {
    pub overall_healthy: bool,
    pub healthy_rpc_count: usize,
    pub total_rpc_count: usize,
    pub healthy_ws_count: usize,
    pub total_ws_count: usize,
    pub last_health_check: Option<Instant>,
    pub uptime_seconds: u64,
}

/// Health monitor for network connections
pub struct HealthMonitor {
    rpc_manager: Arc<RpcManager>,
    websocket_manager: Arc<WebSocketManager>,
    is_running: Arc<AtomicBool>,
    start_time: Instant,
    last_health_check: Arc<RwLock<Option<Instant>>>,
    health_check_interval: Duration,
}

impl HealthMonitor {
    /// Create a new health monitor
    pub fn new(
        rpc_manager: Arc<RpcManager>,
        websocket_manager: Arc<WebSocketManager>,
    ) -> Self {
        Self {
            rpc_manager,
            websocket_manager,
            is_running: Arc::new(AtomicBool::new(false)),
            start_time: Instant::now(),
            last_health_check: Arc::new(RwLock::new(None)),
            health_check_interval: Duration::from_secs(30), // Check every 30 seconds
        }
    }
    
    /// Start health monitoring
    pub async fn start(&self) -> Result<()> {
        if self.is_running.load(Ordering::Relaxed) {
            warn!("Health monitor is already running");
            return Ok(());
        }
        
        self.is_running.store(true, Ordering::Relaxed);
        info!("Starting health monitor");
        
        // Spawn health check task
        let rpc_manager = self.rpc_manager.clone();
        let websocket_manager = self.websocket_manager.clone();
        let is_running = self.is_running.clone();
        let last_health_check = self.last_health_check.clone();
        let check_interval = self.health_check_interval;
        
        tokio::spawn(async move {
            let mut interval = interval(check_interval);
            
            while is_running.load(Ordering::Relaxed) {
                interval.tick().await;
                
                if let Err(e) = Self::perform_health_check(
                    &rpc_manager,
                    &websocket_manager,
                    &last_health_check,
                ).await {
                    error!("Health check failed: {}", e);
                }
            }
            
            debug!("Health monitor task stopped");
        });
        
        info!("Health monitor started");
        Ok(())
    }
    
    /// Stop health monitoring
    pub async fn stop(&self) -> Result<()> {
        if !self.is_running.load(Ordering::Relaxed) {
            warn!("Health monitor is not running");
            return Ok(());
        }
        
        self.is_running.store(false, Ordering::Relaxed);
        info!("Health monitor stopped");
        Ok(())
    }
    
    /// Perform a health check
    async fn perform_health_check(
        rpc_manager: &Arc<RpcManager>,
        websocket_manager: &Arc<WebSocketManager>,
        last_health_check: &Arc<RwLock<Option<Instant>>>,
    ) -> Result<()> {
        let start_time = Instant::now();
        debug!("Performing health check");
        
        // Check RPC endpoints
        let rpc_stats = rpc_manager.get_stats().await;
        let healthy_rpc_count = rpc_stats.primary_endpoints.iter()
            .chain(rpc_stats.backup_endpoints.iter())
            .filter(|endpoint| endpoint.is_healthy)
            .count();
        
        let total_rpc_count = rpc_stats.primary_endpoints.len() + rpc_stats.backup_endpoints.len();
        
        // Check WebSocket endpoints
        let ws_stats = websocket_manager.get_stats().await;
        let healthy_ws_count = ws_stats.endpoints.iter()
            .filter(|endpoint| endpoint.is_healthy)
            .count();
        
        let total_ws_count = ws_stats.endpoints.len();
        
        // Log health status
        if healthy_rpc_count == 0 {
            error!("No healthy RPC endpoints available!");
        } else if healthy_rpc_count < total_rpc_count {
            warn!("Some RPC endpoints are unhealthy: {}/{} healthy", healthy_rpc_count, total_rpc_count);
        } else {
            debug!("All RPC endpoints are healthy: {}/{}", healthy_rpc_count, total_rpc_count);
        }
        
        if healthy_ws_count == 0 {
            error!("No healthy WebSocket endpoints available!");
        } else if healthy_ws_count < total_ws_count {
            warn!("Some WebSocket endpoints are unhealthy: {}/{} healthy", healthy_ws_count, total_ws_count);
        } else {
            debug!("All WebSocket endpoints are healthy: {}/{}", healthy_ws_count, total_ws_count);
        }
        
        // Update last health check time
        *last_health_check.write().await = Some(start_time);
        
        let duration = start_time.elapsed();
        debug!("Health check completed in {:?}", duration);
        
        Ok(())
    }
    
    /// Check if the overall system is healthy
    pub async fn is_healthy(&self) -> bool {
        // Check if we have at least one healthy RPC endpoint
        let rpc_stats = self.rpc_manager.get_stats().await;
        let has_healthy_rpc = rpc_stats.primary_endpoints.iter()
            .chain(rpc_stats.backup_endpoints.iter())
            .any(|endpoint| endpoint.is_healthy);
        
        if !has_healthy_rpc {
            return false;
        }
        
        // Check if we have at least one healthy WebSocket endpoint (optional)
        let ws_stats = self.websocket_manager.get_stats().await;
        let has_healthy_ws = ws_stats.endpoints.iter()
            .any(|endpoint| endpoint.is_healthy);
        
        // For now, we only require RPC to be healthy
        // WebSocket is nice to have but not critical for basic functionality
        has_healthy_rpc
    }
    
    /// Get detailed health information for RPC endpoints
    pub async fn get_rpc_health(&self) -> Vec<EndpointHealth> {
        let mut health_info = Vec::new();
        let rpc_stats = self.rpc_manager.get_stats().await;
        
        for endpoint in rpc_stats.primary_endpoints.iter().chain(rpc_stats.backup_endpoints.iter()) {
            let response_time = if endpoint.is_healthy {
                // Perform a quick health check
                let start = Instant::now();
                match self.rpc_manager.get_slot().await {
                    Ok(_) => Some(start.elapsed().as_millis() as u64),
                    Err(_) => None,
                }
            } else {
                None
            };
            
            health_info.push(EndpointHealth {
                url: endpoint.url.clone(),
                is_healthy: endpoint.is_healthy,
                last_check: Instant::now(),
                response_time_ms: response_time,
                error_message: if !endpoint.is_healthy {
                    Some(format!("Endpoint has {} consecutive failures", endpoint.failure_count))
                } else {
                    None
                },
            });
        }
        
        health_info
    }
    
    /// Get detailed health information for WebSocket endpoints
    pub async fn get_websocket_health(&self) -> Vec<EndpointHealth> {
        let mut health_info = Vec::new();
        let ws_stats = self.websocket_manager.get_stats().await;
        
        for endpoint in &ws_stats.endpoints {
            health_info.push(EndpointHealth {
                url: endpoint.url.clone(),
                is_healthy: endpoint.is_healthy,
                last_check: Instant::now(),
                response_time_ms: None, // WebSocket doesn't have request/response timing
                error_message: if !endpoint.is_healthy {
                    Some(format!("WebSocket connection failed after {} attempts", endpoint.reconnect_attempts))
                } else {
                    None
                },
            });
        }
        
        health_info
    }
    
    /// Get overall health statistics
    pub async fn get_stats(&self) -> HealthStats {
        let rpc_stats = self.rpc_manager.get_stats().await;
        let ws_stats = self.websocket_manager.get_stats().await;
        
        let healthy_rpc_count = rpc_stats.primary_endpoints.iter()
            .chain(rpc_stats.backup_endpoints.iter())
            .filter(|endpoint| endpoint.is_healthy)
            .count();
        
        let total_rpc_count = rpc_stats.primary_endpoints.len() + rpc_stats.backup_endpoints.len();
        
        let healthy_ws_count = ws_stats.endpoints.iter()
            .filter(|endpoint| endpoint.is_healthy)
            .count();
        
        let total_ws_count = ws_stats.endpoints.len();
        
        let overall_healthy = self.is_healthy().await;
        
        HealthStats {
            overall_healthy,
            healthy_rpc_count,
            total_rpc_count,
            healthy_ws_count,
            total_ws_count,
            last_health_check: *self.last_health_check.read().await,
            uptime_seconds: self.start_time.elapsed().as_secs(),
        }
    }
    
    /// Force a health check now
    pub async fn force_health_check(&self) -> Result<()> {
        if !self.is_running.load(Ordering::Relaxed) {
            return Err(LichError::Internal("Health monitor is not running".to_string()));
        }
        
        Self::perform_health_check(
            &self.rpc_manager,
            &self.websocket_manager,
            &self.last_health_check,
        ).await
    }
    
    /// Check if health monitoring is running
    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::Relaxed)
    }
    
    /// Get uptime in seconds
    pub fn uptime_seconds(&self) -> u64 {
        self.start_time.elapsed().as_secs()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::{RpcManager, WebSocketManager};
    use std::time::Duration;
    
    #[tokio::test]
    async fn test_health_monitor_creation() {
        let rpc_manager = Arc::new(
            RpcManager::new(
                vec!["https://api.mainnet-beta.solana.com".to_string()],
                vec![],
                Duration::from_secs(10),
                100,
            ).await.unwrap()
        );
        
        let ws_manager = Arc::new(
            WebSocketManager::new(
                vec!["wss://api.mainnet-beta.solana.com".to_string()],
                Duration::from_secs(5),
            ).await.unwrap()
        );
        
        let health_monitor = HealthMonitor::new(rpc_manager, ws_manager);
        assert!(!health_monitor.is_running());
        assert_eq!(health_monitor.uptime_seconds(), 0);
    }
}

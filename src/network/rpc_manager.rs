//! RPC client management with failover and load balancing

use crate::error::{LichError, Result};
use solana_client::rpc_client::RpcClient;
use solana_client::rpc_response::Response;
use solana_sdk::account::Account;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Signature;
use solana_sdk::transaction::Transaction;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Semaphore};
use tracing::{debug, error, info, warn};

/// RPC endpoint information
#[derive(Clone)]
pub struct RpcEndpoint {
    pub url: String,
    pub client: Arc<RpcClient>,
    pub is_primary: bool,
    pub last_success: Arc<RwLock<Option<Instant>>>,
    pub last_failure: Arc<RwLock<Option<Instant>>>,
    pub failure_count: Arc<AtomicUsize>,
    pub success_count: Arc<AtomicUsize>,
}

impl RpcEndpoint {
    pub fn new(url: String, timeout: Duration, is_primary: bool) -> Self {
        let client = Arc::new(super::create_optimized_rpc_client(&url, timeout));
        
        Self {
            url,
            client,
            is_primary,
            last_success: Arc::new(RwLock::new(None)),
            last_failure: Arc::new(RwLock::new(None)),
            failure_count: Arc::new(AtomicUsize::new(0)),
            success_count: Arc::new(AtomicUsize::new(0)),
        }
    }
    
    pub async fn record_success(&self) {
        *self.last_success.write().await = Some(Instant::now());
        self.success_count.fetch_add(1, Ordering::Relaxed);
        self.failure_count.store(0, Ordering::Relaxed); // Reset failure count on success
    }
    
    pub async fn record_failure(&self) {
        *self.last_failure.write().await = Some(Instant::now());
        self.failure_count.fetch_add(1, Ordering::Relaxed);
    }
    
    pub fn get_failure_count(&self) -> usize {
        self.failure_count.load(Ordering::Relaxed)
    }
    
    pub fn get_success_count(&self) -> usize {
        self.success_count.load(Ordering::Relaxed)
    }
    
    pub async fn is_healthy(&self) -> bool {
        let failure_count = self.get_failure_count();
        let last_failure = *self.last_failure.read().await;
        
        // Consider unhealthy if more than 5 consecutive failures
        if failure_count > 5 {
            // But allow recovery if last failure was more than 5 minutes ago
            if let Some(last_fail_time) = last_failure {
                return last_fail_time.elapsed() > Duration::from_secs(300);
            }
            return false;
        }
        
        true
    }
}

/// RPC manager with load balancing and failover
pub struct RpcManager {
    primary_endpoints: Vec<Arc<RpcEndpoint>>,
    backup_endpoints: Vec<Arc<RpcEndpoint>>,
    current_primary_index: Arc<AtomicUsize>,
    current_backup_index: Arc<AtomicUsize>,
    request_semaphore: Arc<Semaphore>,
    timeout: Duration,
}

impl RpcManager {
    pub async fn new(
        primary_rpc: Vec<String>,
        backup_rpc: Vec<String>,
        timeout: Duration,
        max_concurrent_requests: usize,
    ) -> Result<Self> {
        if primary_rpc.is_empty() {
            return Err(LichError::Config(config::ConfigError::Message(
                "At least one primary RPC endpoint must be provided".to_string()
            )));
        }
        
        let primary_endpoints: Vec<Arc<RpcEndpoint>> = primary_rpc
            .into_iter()
            .map(|url| Arc::new(RpcEndpoint::new(url, timeout, true)))
            .collect();
        
        let backup_endpoints: Vec<Arc<RpcEndpoint>> = backup_rpc
            .into_iter()
            .map(|url| Arc::new(RpcEndpoint::new(url, timeout, false)))
            .collect();
        
        info!(
            "Initialized RPC manager with {} primary and {} backup endpoints",
            primary_endpoints.len(),
            backup_endpoints.len()
        );
        
        Ok(Self {
            primary_endpoints,
            backup_endpoints,
            current_primary_index: Arc::new(AtomicUsize::new(0)),
            current_backup_index: Arc::new(AtomicUsize::new(0)),
            request_semaphore: Arc::new(Semaphore::new(max_concurrent_requests)),
            timeout,
        })
    }
    
    /// Get the next available primary endpoint
    fn get_next_primary_endpoint(&self) -> Option<Arc<RpcEndpoint>> {
        if self.primary_endpoints.is_empty() {
            return None;
        }
        
        let index = self.current_primary_index.fetch_add(1, Ordering::Relaxed) % self.primary_endpoints.len();
        Some(self.primary_endpoints[index].clone())
    }
    
    /// Get the next available backup endpoint
    fn get_next_backup_endpoint(&self) -> Option<Arc<RpcEndpoint>> {
        if self.backup_endpoints.is_empty() {
            return None;
        }
        
        let index = self.current_backup_index.fetch_add(1, Ordering::Relaxed) % self.backup_endpoints.len();
        Some(self.backup_endpoints[index].clone())
    }
    
    /// Execute an RPC request with automatic failover
    async fn execute_request<T, F>(&self, operation: F) -> Result<T>
    where
        F: Fn(&RpcClient) -> std::result::Result<T, solana_client::client_error::ClientError> + Send + Sync + Clone,
        T: Send + 'static,
    {
        let _permit = self.request_semaphore.acquire().await
            .map_err(|_| LichError::Internal("Failed to acquire request semaphore".to_string()))?;
        
        // Try primary endpoints first
        for _ in 0..self.primary_endpoints.len() {
            if let Some(endpoint) = self.get_next_primary_endpoint() {
                if endpoint.is_healthy().await {
                    match operation(&endpoint.client) {
                        Ok(result) => {
                            endpoint.record_success().await;
                            return Ok(result);
                        }
                        Err(e) => {
                            endpoint.record_failure().await;
                            warn!("Primary RPC request failed on {}: {}", endpoint.url, e);
                        }
                    }
                }
            }
        }
        
        // If all primary endpoints failed, try backup endpoints
        for _ in 0..self.backup_endpoints.len() {
            if let Some(endpoint) = self.get_next_backup_endpoint() {
                if endpoint.is_healthy().await {
                    match operation(&endpoint.client) {
                        Ok(result) => {
                            endpoint.record_success().await;
                            warn!("Using backup RPC endpoint: {}", endpoint.url);
                            return Ok(result);
                        }
                        Err(e) => {
                            endpoint.record_failure().await;
                            warn!("Backup RPC request failed on {}: {}", endpoint.url, e);
                        }
                    }
                }
            }
        }
        
        Err(LichError::ConnectionLost {
            endpoint: "All RPC endpoints".to_string(),
        })
    }
    
    /// Get account information
    pub async fn get_account(&self, pubkey: &Pubkey) -> Result<Option<Account>> {
        let pubkey = *pubkey;
        self.execute_request(move |client| {
            match client.get_account(&pubkey) {
                Ok(account) => Ok(Some(account)),
                Err(_) => Ok(None), // Account not found or other error
            }
        }).await
    }
    
    /// Get account information with commitment
    pub async fn get_account_with_commitment(
        &self,
        pubkey: &Pubkey,
        commitment: CommitmentConfig,
    ) -> Result<Response<Option<Account>>> {
        let pubkey = *pubkey;
        self.execute_request(move |client| client.get_account_with_commitment(&pubkey, commitment)).await
    }
    
    /// Send transaction
    pub async fn send_transaction(&self, transaction: &Transaction) -> Result<Signature> {
        let transaction = transaction.clone();
        self.execute_request(move |client| client.send_transaction(&transaction)).await
    }
    
    /// Send transaction with config
    pub async fn send_transaction_with_config(
        &self,
        transaction: &Transaction,
        config: solana_client::rpc_config::RpcSendTransactionConfig,
    ) -> Result<Signature> {
        let transaction = transaction.clone();
        self.execute_request(move |client| client.send_transaction_with_config(&transaction, config)).await
    }
    
    /// Get transaction status
    pub async fn get_signature_status(&self, signature: &Signature) -> Result<Option<std::result::Result<(), solana_sdk::transaction::TransactionError>>> {
        let signature = *signature;
        self.execute_request(move |client| client.get_signature_status(&signature)).await
    }
    
    /// Get latest blockhash
    pub async fn get_latest_blockhash(&self) -> Result<solana_sdk::hash::Hash> {
        self.execute_request(|client| client.get_latest_blockhash()).await
    }
    
    /// Get slot
    pub async fn get_slot(&self) -> Result<u64> {
        self.execute_request(|client| client.get_slot()).await
    }
    
    /// Get balance
    pub async fn get_balance(&self, pubkey: &Pubkey) -> Result<u64> {
        let pubkey = *pubkey;
        self.execute_request(move |client| client.get_balance(&pubkey)).await
    }
    
    /// Get all healthy endpoints
    pub async fn get_healthy_endpoints(&self) -> Vec<Arc<RpcEndpoint>> {
        let mut healthy = Vec::new();
        
        for endpoint in &self.primary_endpoints {
            if endpoint.is_healthy().await {
                healthy.push(endpoint.clone());
            }
        }
        
        for endpoint in &self.backup_endpoints {
            if endpoint.is_healthy().await {
                healthy.push(endpoint.clone());
            }
        }
        
        healthy
    }
    
    /// Get RPC statistics
    pub async fn get_stats(&self) -> RpcStats {
        let mut primary_stats = Vec::new();
        let mut backup_stats = Vec::new();
        
        for endpoint in &self.primary_endpoints {
            primary_stats.push(EndpointStats {
                url: endpoint.url.clone(),
                is_healthy: endpoint.is_healthy().await,
                success_count: endpoint.get_success_count(),
                failure_count: endpoint.get_failure_count(),
                last_success: *endpoint.last_success.read().await,
                last_failure: *endpoint.last_failure.read().await,
            });
        }
        
        for endpoint in &self.backup_endpoints {
            backup_stats.push(EndpointStats {
                url: endpoint.url.clone(),
                is_healthy: endpoint.is_healthy().await,
                success_count: endpoint.get_success_count(),
                failure_count: endpoint.get_failure_count(),
                last_success: *endpoint.last_success.read().await,
                last_failure: *endpoint.last_failure.read().await,
            });
        }
        
        RpcStats {
            primary_endpoints: primary_stats,
            backup_endpoints: backup_stats,
            available_permits: self.request_semaphore.available_permits(),
        }
    }
}

/// RPC endpoint statistics
#[derive(Debug, Clone)]
pub struct EndpointStats {
    pub url: String,
    pub is_healthy: bool,
    pub success_count: usize,
    pub failure_count: usize,
    pub last_success: Option<Instant>,
    pub last_failure: Option<Instant>,
}

/// RPC manager statistics
#[derive(Debug, Clone)]
pub struct RpcStats {
    pub primary_endpoints: Vec<EndpointStats>,
    pub backup_endpoints: Vec<EndpointStats>,
    pub available_permits: usize,
}

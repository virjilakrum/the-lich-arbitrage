//! WebSocket management for real-time data streams

use crate::error::{LichError, Result};
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, RwLock};
use tokio::time::{interval, sleep};
use tokio_tungstenite::{connect_async, tungstenite::Message, WebSocketStream, MaybeTlsStream};
use tracing::{debug, error, info, warn};

/// WebSocket event types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WebSocketEvent {
    /// Account update event
    AccountUpdate {
        pubkey: String,
        account: AccountData,
        slot: u64,
    },
    /// Program account update event
    ProgramAccountUpdate {
        pubkey: String,
        account: AccountData,
        slot: u64,
    },
    /// Slot update event
    SlotUpdate {
        slot: u64,
        parent: Option<u64>,
        root: Option<u64>,
    },
    /// Signature notification
    SignatureNotification {
        signature: String,
        err: Option<String>,
        slot: u64,
    },
    /// Connection status change
    ConnectionStatus {
        endpoint: String,
        connected: bool,
        timestamp: u64,
    },
    /// Error event
    Error {
        endpoint: String,
        error: String,
        timestamp: u64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountData {
    pub lamports: u64,
    pub data: Vec<u8>,
    pub owner: String,
    pub executable: bool,
    pub rent_epoch: u64,
}

/// WebSocket subscription
#[derive(Debug, Clone)]
pub struct Subscription {
    pub id: u64,
    pub method: String,
    pub params: serde_json::Value,
    pub active: bool,
}

/// WebSocket connection manager
pub struct WebSocketConnection {
    url: String,
    stream: Option<WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>>,
    subscriptions: Arc<RwLock<HashMap<u64, Subscription>>>,
    next_subscription_id: Arc<AtomicUsize>,
    is_connected: Arc<AtomicBool>,
    last_ping: Arc<RwLock<Option<Instant>>>,
    last_pong: Arc<RwLock<Option<Instant>>>,
    reconnect_attempts: Arc<AtomicUsize>,
    event_sender: broadcast::Sender<WebSocketEvent>,
}

impl WebSocketConnection {
    pub fn new(url: String, event_sender: broadcast::Sender<WebSocketEvent>) -> Self {
        Self {
            url,
            stream: None,
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
            next_subscription_id: Arc::new(AtomicUsize::new(1)),
            is_connected: Arc::new(AtomicBool::new(false)),
            last_ping: Arc::new(RwLock::new(None)),
            last_pong: Arc::new(RwLock::new(None)),
            reconnect_attempts: Arc::new(AtomicUsize::new(0)),
            event_sender,
        }
    }
    
    /// Connect to WebSocket endpoint
    pub async fn connect(&mut self) -> Result<()> {
        info!("Connecting to WebSocket: {}", self.url);
        
        match connect_async(&self.url).await {
            Ok((ws_stream, _)) => {
                self.stream = Some(ws_stream);
                self.is_connected.store(true, Ordering::Relaxed);
                self.reconnect_attempts.store(0, Ordering::Relaxed);
                
                // Send connection status event
                let _ = self.event_sender.send(WebSocketEvent::ConnectionStatus {
                    endpoint: self.url.clone(),
                    connected: true,
                    timestamp: chrono::Utc::now().timestamp_millis() as u64,
                });
                
                info!("Connected to WebSocket: {}", self.url);
                Ok(())
            }
            Err(e) => {
                self.is_connected.store(false, Ordering::Relaxed);
                let attempts = self.reconnect_attempts.fetch_add(1, Ordering::Relaxed);
                
                error!("Failed to connect to WebSocket {}: {} (attempt {})", self.url, e, attempts + 1);
                
                // Send error event
                let _ = self.event_sender.send(WebSocketEvent::Error {
                    endpoint: self.url.clone(),
                    error: e.to_string(),
                    timestamp: chrono::Utc::now().timestamp_millis() as u64,
                });
                
                Err(LichError::WebSocket(e))
            }
        }
    }
    
    /// Disconnect from WebSocket
    pub async fn disconnect(&mut self) -> Result<()> {
        if let Some(mut stream) = self.stream.take() {
            let _ = stream.close(None).await;
        }
        
        self.is_connected.store(false, Ordering::Relaxed);
        
        // Send connection status event
        let _ = self.event_sender.send(WebSocketEvent::ConnectionStatus {
            endpoint: self.url.clone(),
            connected: false,
            timestamp: chrono::Utc::now().timestamp_millis() as u64,
        });
        
        info!("Disconnected from WebSocket: {}", self.url);
        Ok(())
    }
    
    /// Subscribe to account updates
    pub async fn subscribe_account(&mut self, pubkey: &str) -> Result<u64> {
        let subscription_id = self.next_subscription_id.fetch_add(1, Ordering::Relaxed) as u64;
        
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": subscription_id,
            "method": "accountSubscribe",
            "params": [
                pubkey,
                {
                    "encoding": "base64",
                    "commitment": "confirmed"
                }
            ]
        });
        
        self.send_message(Message::Text(request.to_string())).await?;
        
        let subscription = Subscription {
            id: subscription_id,
            method: "accountSubscribe".to_string(),
            params: serde_json::json!([pubkey]),
            active: true,
        };
        
        self.subscriptions.write().await.insert(subscription_id, subscription);
        
        debug!("Subscribed to account updates for {}: subscription_id={}", pubkey, subscription_id);
        Ok(subscription_id)
    }
    
    /// Subscribe to program account updates
    pub async fn subscribe_program_accounts(&mut self, program_id: &str) -> Result<u64> {
        let subscription_id = self.next_subscription_id.fetch_add(1, Ordering::Relaxed) as u64;
        
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": subscription_id,
            "method": "programSubscribe",
            "params": [
                program_id,
                {
                    "encoding": "base64",
                    "commitment": "confirmed"
                }
            ]
        });
        
        self.send_message(Message::Text(request.to_string())).await?;
        
        let subscription = Subscription {
            id: subscription_id,
            method: "programSubscribe".to_string(),
            params: serde_json::json!([program_id]),
            active: true,
        };
        
        self.subscriptions.write().await.insert(subscription_id, subscription);
        
        debug!("Subscribed to program account updates for {}: subscription_id={}", program_id, subscription_id);
        Ok(subscription_id)
    }
    
    /// Subscribe to signature notifications
    pub async fn subscribe_signature(&mut self, signature: &str) -> Result<u64> {
        let subscription_id = self.next_subscription_id.fetch_add(1, Ordering::Relaxed) as u64;
        
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": subscription_id,
            "method": "signatureSubscribe",
            "params": [
                signature,
                {
                    "commitment": "confirmed"
                }
            ]
        });
        
        self.send_message(Message::Text(request.to_string())).await?;
        
        let subscription = Subscription {
            id: subscription_id,
            method: "signatureSubscribe".to_string(),
            params: serde_json::json!([signature]),
            active: true,
        };
        
        self.subscriptions.write().await.insert(subscription_id, subscription);
        
        debug!("Subscribed to signature notifications for {}: subscription_id={}", signature, subscription_id);
        Ok(subscription_id)
    }
    
    /// Unsubscribe from a subscription
    pub async fn unsubscribe(&mut self, subscription_id: u64) -> Result<()> {
        let unsubscribe_method = {
            let subscriptions = self.subscriptions.read().await;
            if let Some(subscription) = subscriptions.get(&subscription_id) {
                match subscription.method.as_str() {
                    "accountSubscribe" => "accountUnsubscribe",
                    "programSubscribe" => "programUnsubscribe",
                    "signatureSubscribe" => "signatureUnsubscribe",
                    _ => return Err(LichError::Internal(format!("Unknown subscription method: {}", subscription.method))),
                }
            } else {
                return Ok(()); // Subscription not found
            }
        };

        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": subscription_id,
            "method": unsubscribe_method,
            "params": [subscription_id]
        });

        self.send_message(Message::Text(request.to_string())).await?;
        self.subscriptions.write().await.remove(&subscription_id);

        debug!("Unsubscribed from subscription_id={}", subscription_id);
        Ok(())
    }
    
    /// Send a message through the WebSocket
    async fn send_message(&mut self, message: Message) -> Result<()> {
        if let Some(stream) = &mut self.stream {
            stream.send(message).await.map_err(LichError::WebSocket)?;
        } else {
            return Err(LichError::ConnectionLost {
                endpoint: self.url.clone(),
            });
        }
        Ok(())
    }
    
    /// Handle incoming messages
    pub async fn handle_messages(&mut self) -> Result<()> {
        if self.stream.is_none() {
            return Ok(());
        }

        loop {
            let message = {
                let stream = self.stream.as_mut().unwrap();
                stream.next().await
            };

            match message {
                Some(Ok(Message::Text(text))) => {
                    if let Err(e) = self.process_text_message(&text).await {
                        warn!("Error processing WebSocket message: {}", e);
                    }
                }
                Some(Ok(Message::Pong(_))) => {
                    *self.last_pong.write().await = Some(Instant::now());
                    debug!("Received pong from {}", self.url);
                }
                Some(Ok(Message::Close(_))) => {
                    info!("WebSocket connection closed: {}", self.url);
                    break;
                }
                Some(Err(e)) => {
                    error!("WebSocket error on {}: {}", self.url, e);
                    break;
                }
                None => {
                    debug!("WebSocket stream ended: {}", self.url);
                    break;
                }
                _ => {}
            }
        }

        self.is_connected.store(false, Ordering::Relaxed);
        Ok(())
    }
    
    /// Process text messages from WebSocket
    async fn process_text_message(&self, text: &str) -> Result<()> {
        let value: serde_json::Value = serde_json::from_str(text)?;
        
        // Handle subscription notifications
        if let Some(method) = value.get("method").and_then(|m| m.as_str()) {
            match method {
                "accountNotification" => {
                    if let Some(params) = value.get("params") {
                        // Process account notification
                        debug!("Received account notification: {}", text);
                    }
                }
                "programNotification" => {
                    if let Some(params) = value.get("params") {
                        // Process program notification
                        debug!("Received program notification: {}", text);
                    }
                }
                "signatureNotification" => {
                    if let Some(params) = value.get("params") {
                        // Process signature notification
                        debug!("Received signature notification: {}", text);
                    }
                }
                _ => {
                    debug!("Unknown notification method: {}", method);
                }
            }
        }
        
        Ok(())
    }
    
    /// Send ping to keep connection alive
    pub async fn send_ping(&mut self) -> Result<()> {
        if self.is_connected.load(Ordering::Relaxed) {
            self.send_message(Message::Ping(vec![])).await?;
            *self.last_ping.write().await = Some(Instant::now());
            debug!("Sent ping to {}", self.url);
        }
        Ok(())
    }
    
    /// Check if connection is healthy
    pub async fn is_healthy(&self) -> bool {
        if !self.is_connected.load(Ordering::Relaxed) {
            return false;
        }
        
        // Check if we've received a pong recently
        if let Some(last_pong) = *self.last_pong.read().await {
            if last_pong.elapsed() > Duration::from_secs(60) {
                return false;
            }
        }
        
        true
    }
}

/// WebSocket manager for handling multiple connections
pub struct WebSocketManager {
    connections: Arc<RwLock<HashMap<String, Arc<RwLock<WebSocketConnection>>>>>,
    event_sender: broadcast::Sender<WebSocketEvent>,
    event_receiver: broadcast::Receiver<WebSocketEvent>,
    reconnect_timeout: Duration,
    is_running: Arc<AtomicBool>,
}

impl WebSocketManager {
    pub async fn new(endpoints: Vec<String>, reconnect_timeout: Duration) -> Result<Self> {
        let (event_sender, event_receiver) = broadcast::channel(1000);
        let connections = Arc::new(RwLock::new(HashMap::new()));
        
        // Create connections for each endpoint
        for endpoint in endpoints {
            let connection = Arc::new(RwLock::new(WebSocketConnection::new(
                endpoint.clone(),
                event_sender.clone(),
            )));
            connections.write().await.insert(endpoint, connection);
        }
        
        Ok(Self {
            connections,
            event_sender,
            event_receiver,
            reconnect_timeout,
            is_running: Arc::new(AtomicBool::new(false)),
        })
    }
    
    /// Start WebSocket manager
    pub async fn start(&self) -> Result<()> {
        self.is_running.store(true, Ordering::Relaxed);
        
        // Connect to all endpoints
        for (endpoint, connection) in self.connections.read().await.iter() {
            let mut conn = connection.write().await;
            if let Err(e) = conn.connect().await {
                warn!("Failed to connect to {}: {}", endpoint, e);
            }
        }
        
        info!("WebSocket manager started");
        Ok(())
    }
    
    /// Stop WebSocket manager
    pub async fn stop(&self) -> Result<()> {
        self.is_running.store(false, Ordering::Relaxed);
        
        // Disconnect from all endpoints
        for (endpoint, connection) in self.connections.read().await.iter() {
            let mut conn = connection.write().await;
            if let Err(e) = conn.disconnect().await {
                warn!("Failed to disconnect from {}: {}", endpoint, e);
            }
        }
        
        info!("WebSocket manager stopped");
        Ok(())
    }
    
    /// Get event receiver
    pub fn subscribe_events(&self) -> broadcast::Receiver<WebSocketEvent> {
        self.event_sender.subscribe()
    }
    
    /// Get WebSocket statistics
    pub async fn get_stats(&self) -> WebSocketStats {
        let mut endpoint_stats = Vec::new();
        
        for (endpoint, connection) in self.connections.read().await.iter() {
            let conn = connection.read().await;
            endpoint_stats.push(WebSocketEndpointStats {
                url: endpoint.clone(),
                is_connected: conn.is_connected.load(Ordering::Relaxed),
                is_healthy: conn.is_healthy().await,
                reconnect_attempts: conn.reconnect_attempts.load(Ordering::Relaxed),
                subscription_count: conn.subscriptions.read().await.len(),
            });
        }
        
        WebSocketStats {
            endpoints: endpoint_stats,
            is_running: self.is_running.load(Ordering::Relaxed),
        }
    }
}

/// WebSocket endpoint statistics
#[derive(Debug, Clone)]
pub struct WebSocketEndpointStats {
    pub url: String,
    pub is_connected: bool,
    pub is_healthy: bool,
    pub reconnect_attempts: usize,
    pub subscription_count: usize,
}

/// WebSocket manager statistics
#[derive(Debug, Clone)]
pub struct WebSocketStats {
    pub endpoints: Vec<WebSocketEndpointStats>,
    pub is_running: bool,
}

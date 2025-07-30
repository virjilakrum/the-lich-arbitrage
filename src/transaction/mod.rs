//! Transaction management and execution

use crate::error::{LichError, Result};
use serde::{Deserialize, Serialize};

pub mod builder;
pub mod executor;
pub mod manager;

pub use builder::TransactionBuilder;
pub use executor::TransactionExecutor;
pub use manager::TransactionManager;

/// Transaction status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TransactionStatus {
    Pending,
    Confirmed,
    Failed,
    Expired,
}

/// Placeholder transaction module
/// This will be implemented in the next phase
pub struct Transaction {
    pub id: String,
    pub status: TransactionStatus,
}

impl Transaction {
    pub fn new(id: String) -> Self {
        Self {
            id,
            status: TransactionStatus::Pending,
        }
    }
}

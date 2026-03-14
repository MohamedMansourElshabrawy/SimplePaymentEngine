use crate::types::{ClientId, TransactionId};

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("CSV error: {0}")]
    Csv(#[from] csv::Error),

    #[error("Transaction {tx_id} rejected: insufficient funds for client {client_id}")]
    InsufficientFunds {
        client_id: ClientId,
        tx_id: TransactionId,
    },

    #[error("Transaction {tx_id} rejected: account {client_id} is frozen")]
    AccountFrozen {
        client_id: ClientId,
        tx_id: TransactionId,
    },

    #[error("Dispute on transaction {tx_id}: transaction not found")]
    TransactionNotFound { tx_id: TransactionId },

    #[error("Transaction {tx_id}: not currently under dispute")]
    TransactionNotDisputed { tx_id: TransactionId },

    #[error("Transaction {tx_id}: already under dispute")]
    TransactionAlreadyDisputed { tx_id: TransactionId },

    #[error("Transaction {tx_id}: already charged back")]
    TransactionAlreadyChargedBack { tx_id: TransactionId },

    #[error("Transaction {tx_id}: client mismatch (expected {expected}, got {actual})")]
    ClientMismatch {
        tx_id: TransactionId,
        expected: ClientId,
        actual: ClientId,
    },

    #[error("Transaction {tx_id}: duplicate transaction ID")]
    DuplicateTransaction { tx_id: TransactionId },

    #[error("Transaction {tx_id}: invalid amount")]
    InvalidAmount { tx_id: TransactionId },

    #[error("Transaction {tx_id}: missing required amount")]
    MissingAmount { tx_id: TransactionId },

    #[error("Parse error at line {line}: {message}")]
    Parse { line: usize, message: String },
}

use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::error::EngineError;
use crate::model::transaction::TransactionType;

use super::event::LedgerEvent;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ErrorCategory {
    InsufficientFunds,
    AccountFrozen,
    TransactionNotFound,
    TransactionNotDisputed,
    TransactionAlreadyDisputed,
    TransactionAlreadyChargedBack,
    ClientMismatch,
    DuplicateTransaction,
    InvalidAmount,
    MissingAmount,
    ParseError,
}

impl From<&EngineError> for ErrorCategory {
    fn from(err: &EngineError) -> Self {
        match err {
            EngineError::InsufficientFunds { .. } => Self::InsufficientFunds,
            EngineError::AccountFrozen { .. } => Self::AccountFrozen,
            EngineError::TransactionNotFound { .. } => Self::TransactionNotFound,
            EngineError::TransactionNotDisputed { .. } => Self::TransactionNotDisputed,
            EngineError::TransactionAlreadyDisputed { .. } => Self::TransactionAlreadyDisputed,
            EngineError::TransactionAlreadyChargedBack { .. } => Self::TransactionAlreadyChargedBack,
            EngineError::ClientMismatch { .. } => Self::ClientMismatch,
            EngineError::DuplicateTransaction { .. } => Self::DuplicateTransaction,
            EngineError::InvalidAmount { .. } => Self::InvalidAmount,
            EngineError::MissingAmount { .. } => Self::MissingAmount,
            _ => Self::ParseError,
        }
    }
}

pub struct Aggregator {
    events: Vec<LedgerEvent>,
    errors: Vec<EngineError>,
    tx_counts: HashMap<TransactionType, u64>,
    error_counts: HashMap<ErrorCategory, u64>,
    started_at: Instant,
}

impl Aggregator {
    pub fn new() -> Self {
        Self {
            events: Vec::new(),
            errors: Vec::new(),
            tx_counts: HashMap::new(),
            error_counts: HashMap::new(),
            started_at: Instant::now(),
        }
    }

    pub fn record_event(&mut self, event: LedgerEvent) {
        let tx_type = match &event {
            LedgerEvent::Deposited { .. } => TransactionType::Deposit,
            LedgerEvent::Withdrawn { .. } => TransactionType::Withdrawal,
            LedgerEvent::DisputeOpened { .. } => TransactionType::Dispute,
            LedgerEvent::DisputeResolved { .. } => TransactionType::Resolve,
            LedgerEvent::ChargedBack { .. } => TransactionType::Chargeback,
        };
        *self.tx_counts.entry(tx_type).or_insert(0) += 1;
        self.events.push(event);
    }

    pub fn record_error(&mut self, error: EngineError) {
        let category = ErrorCategory::from(&error);
        *self.error_counts.entry(category).or_insert(0) += 1;
        tracing::warn!("{}", error);
        self.errors.push(error);
    }

    pub fn record_parse_error(&mut self, line: usize, error: EngineError) {
        tracing::warn!(line = line, "{}", error);
        *self.error_counts.entry(ErrorCategory::ParseError).or_insert(0) += 1;
        self.errors.push(error);
    }

    pub fn summarize(&self) -> AggregatorSummary {
        AggregatorSummary {
            total_processed: self.events.len() as u64,
            total_errors: self.errors.len() as u64,
            tx_counts: self.tx_counts.clone(),
            error_counts: self.error_counts.clone(),
            processing_duration: self.started_at.elapsed(),
        }
    }

    pub fn events(&self) -> &[LedgerEvent] { &self.events }
    pub fn errors(&self) -> &[EngineError] { &self.errors }
}

impl Default for Aggregator {
    fn default() -> Self { Self::new() }
}

#[derive(Debug)]
pub struct AggregatorSummary {
    pub total_processed: u64,
    pub total_errors: u64,
    pub tx_counts: HashMap<TransactionType, u64>,
    pub error_counts: HashMap<ErrorCategory, u64>,
    pub processing_duration: Duration,
}

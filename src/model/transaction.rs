use std::fmt;

use rust_decimal::Decimal;
use serde::Deserialize;

use crate::types::{ClientId, TransactionId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TransactionType {
    Deposit,
    Withdrawal,
    Dispute,
    Resolve,
    Chargeback,
}

impl fmt::Display for TransactionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Deposit => write!(f, "deposit"),
            Self::Withdrawal => write!(f, "withdrawal"),
            Self::Dispute => write!(f, "dispute"),
            Self::Resolve => write!(f, "resolve"),
            Self::Chargeback => write!(f, "chargeback"),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct RawTransaction {
    #[serde(rename = "type")]
    pub tx_type: TransactionType,
    pub client: ClientId,
    pub tx: TransactionId,
    pub amount: Option<Decimal>,
}

#[derive(Debug)]
pub struct ValidatedTransaction {
    tx_type: TransactionType,
    client: ClientId,
    tx: TransactionId,
    amount: Option<Decimal>,
}

impl ValidatedTransaction {
    pub(crate) fn new(
        tx_type: TransactionType,
        client: ClientId,
        tx: TransactionId,
        amount: Option<Decimal>,
    ) -> Self {
        Self { tx_type, client, tx, amount }
    }

    pub fn tx_type(&self) -> TransactionType { self.tx_type }
    pub fn client(&self) -> ClientId { self.client }
    pub fn tx_id(&self) -> TransactionId { self.tx }
    pub fn amount(&self) -> Option<Decimal> { self.amount }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionState {
    Active,
    Disputed,
    ChargedBack,
}

#[derive(Debug)]
pub struct TransactionRecord {
    pub client: ClientId,
    pub amount: Decimal,
    pub state: TransactionState,
}

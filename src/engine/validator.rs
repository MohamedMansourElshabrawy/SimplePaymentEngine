use rust_decimal::Decimal;

use crate::error::EngineError;
use crate::model::transaction::{RawTransaction, TransactionType, ValidatedTransaction};

pub struct TransactionValidator;

impl TransactionValidator {
    pub fn new() -> Self { Self }

    pub fn validate(&self, raw: RawTransaction) -> Result<ValidatedTransaction, EngineError> {
        match raw.tx_type {
            TransactionType::Deposit | TransactionType::Withdrawal => {
                let amount = raw.amount.ok_or(EngineError::MissingAmount { tx_id: raw.tx })?;
                if amount <= Decimal::ZERO {
                    return Err(EngineError::InvalidAmount { tx_id: raw.tx });
                }
                Ok(ValidatedTransaction::new(raw.tx_type, raw.client, raw.tx, Some(amount)))
            }
            TransactionType::Dispute | TransactionType::Resolve | TransactionType::Chargeback => {
                Ok(ValidatedTransaction::new(raw.tx_type, raw.client, raw.tx, None))
            }
        }
    }
}

impl Default for TransactionValidator {
    fn default() -> Self { Self::new() }
}

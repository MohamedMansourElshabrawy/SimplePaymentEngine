use crate::aggregator::event::LedgerEvent;
use crate::aggregator::metrics::Aggregator;
use crate::error::EngineError;
use crate::io::sink::AccountOutput;
use crate::io::source::TransactionSource;
use crate::model::transaction::{
    TransactionRecord, TransactionState, TransactionType, ValidatedTransaction,
};
use crate::repository::in_memory::{InMemoryAccountRepo, InMemoryTransactionRepo};
use crate::repository::{AccountRepository, TransactionRepository};
use crate::types::{ClientId, TransactionId};

use super::validator::TransactionValidator;

pub struct PaymentEngine {
    accounts: Box<dyn AccountRepository>,
    transactions: Box<dyn TransactionRepository>,
    validator: TransactionValidator,
}

// ---------------------------------------------------------------------------
// Builder
// ---------------------------------------------------------------------------

pub struct PaymentEngineBuilder {
    accounts: Option<Box<dyn AccountRepository>>,
    transactions: Option<Box<dyn TransactionRepository>>,
}

impl PaymentEngineBuilder {
    pub fn new() -> Self {
        Self {
            accounts: None,
            transactions: None,
        }
    }

    pub fn with_account_repo(mut self, repo: impl AccountRepository + 'static) -> Self {
        self.accounts = Some(Box::new(repo));
        self
    }

    pub fn with_transaction_repo(mut self, repo: impl TransactionRepository + 'static) -> Self {
        self.transactions = Some(Box::new(repo));
        self
    }

    pub fn build(self) -> PaymentEngine {
        PaymentEngine {
            accounts: self
                .accounts
                .unwrap_or_else(|| Box::new(InMemoryAccountRepo::new())),
            transactions: self
                .transactions
                .unwrap_or_else(|| Box::new(InMemoryTransactionRepo::new())),
            validator: TransactionValidator::new(),
        }
    }
}

impl Default for PaymentEngineBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Engine
// ---------------------------------------------------------------------------

impl PaymentEngine {
    pub fn builder() -> PaymentEngineBuilder {
        PaymentEngineBuilder::new()
    }

    /// Run the full pipeline: read from source, validate, process, record.
    pub fn process_all(
        &mut self,
        source: &mut dyn TransactionSource,
        aggregator: &mut Aggregator,
    ) {
        for (line_num, result) in source.transactions().enumerate() {
            match result {
                Ok(raw) => match self.validator.validate(raw) {
                    Ok(validated) => match self.process_transaction(&validated) {
                        Ok(event) => aggregator.record_event(event),
                        Err(e) => aggregator.record_error(e),
                    },
                    Err(e) => aggregator.record_error(e),
                },
                Err(e) => aggregator.record_parse_error(line_num, e),
            }
        }
    }

    /// Snapshot of all account states, ready for output serialization.
    pub fn account_outputs(&self) -> Vec<AccountOutput> {
        self.accounts
            .iter()
            .map(|(id, account)| AccountOutput {
                client: *id,
                available: account.available().round_dp(4),
                held: account.held().round_dp(4),
                total: account.total().round_dp(4),
                locked: account.is_locked(),
            })
            .collect()
    }

    // -- dispatch --

    fn process_transaction(
        &mut self,
        tx: &ValidatedTransaction,
    ) -> Result<LedgerEvent, EngineError> {
        match tx.tx_type() {
            TransactionType::Deposit => self.process_deposit(tx),
            TransactionType::Withdrawal => self.process_withdrawal(tx),
            TransactionType::Dispute => self.process_dispute(tx),
            TransactionType::Resolve => self.process_resolve(tx),
            TransactionType::Chargeback => self.process_chargeback(tx),
        }
    }

    // -- deposit --

    fn process_deposit(&mut self, tx: &ValidatedTransaction) -> Result<LedgerEvent, EngineError> {
        let client_id = tx.client();
        let tx_id = tx.tx_id();
        let amount = tx.amount().unwrap(); // safe: validator guarantees

        if self.transactions.contains(&tx_id) {
            return Err(EngineError::DuplicateTransaction { tx_id });
        }

        if self
            .accounts
            .get(&client_id)
            .is_some_and(|a| a.is_locked())
        {
            return Err(EngineError::AccountFrozen { client_id, tx_id });
        }

        let account = self.accounts.get_or_create(client_id);
        account.deposit(amount);

        self.transactions.store(
            tx_id,
            TransactionRecord {
                client: client_id,
                amount,
                state: TransactionState::Active,
            },
        );

        Ok(LedgerEvent::Deposited {
            client: client_id,
            tx: tx_id,
            amount,
        })
    }

    // -- withdrawal --

    fn process_withdrawal(
        &mut self,
        tx: &ValidatedTransaction,
    ) -> Result<LedgerEvent, EngineError> {
        let client_id = tx.client();
        let tx_id = tx.tx_id();
        let amount = tx.amount().unwrap();

        if self
            .accounts
            .get(&client_id)
            .is_some_and(|a| a.is_locked())
        {
            return Err(EngineError::AccountFrozen { client_id, tx_id });
        }

        let account = self.accounts.get_or_create(client_id);

        if account.available() < amount {
            return Err(EngineError::InsufficientFunds { client_id, tx_id });
        }

        account.withdraw(amount);

        Ok(LedgerEvent::Withdrawn {
            client: client_id,
            tx: tx_id,
            amount,
        })
    }

    // -- dispute --

    fn process_dispute(&mut self, tx: &ValidatedTransaction) -> Result<LedgerEvent, EngineError> {
        let client_id = tx.client();
        let tx_id = tx.tx_id();

        let (record_client, record_amount, record_state) = {
            let record = self
                .transactions
                .get(&tx_id)
                .ok_or(EngineError::TransactionNotFound { tx_id })?;
            (record.client, record.amount, record.state)
        };

        Self::verify_client_ownership(tx_id, record_client, client_id)?;

        match record_state {
            TransactionState::Active => {}
            TransactionState::Disputed => {
                return Err(EngineError::TransactionAlreadyDisputed { tx_id })
            }
            TransactionState::ChargedBack => {
                return Err(EngineError::TransactionAlreadyChargedBack { tx_id })
            }
        }

        let account = self.accounts.get_or_create(client_id);
        account.hold(record_amount);

        self.transactions.get_mut(&tx_id).unwrap().state = TransactionState::Disputed;

        Ok(LedgerEvent::DisputeOpened {
            client: client_id,
            tx: tx_id,
            held_amount: record_amount,
        })
    }

    // -- resolve --

    fn process_resolve(&mut self, tx: &ValidatedTransaction) -> Result<LedgerEvent, EngineError> {
        let client_id = tx.client();
        let tx_id = tx.tx_id();

        let (record_client, record_amount, record_state) = {
            let record = self
                .transactions
                .get(&tx_id)
                .ok_or(EngineError::TransactionNotFound { tx_id })?;
            (record.client, record.amount, record.state)
        };

        Self::verify_client_ownership(tx_id, record_client, client_id)?;

        if record_state != TransactionState::Disputed {
            return Err(EngineError::TransactionNotDisputed { tx_id });
        }

        let account = self.accounts.get_or_create(client_id);
        account.release(record_amount);

        self.transactions.get_mut(&tx_id).unwrap().state = TransactionState::Active;

        Ok(LedgerEvent::DisputeResolved {
            client: client_id,
            tx: tx_id,
            released_amount: record_amount,
        })
    }

    // -- chargeback --

    fn process_chargeback(
        &mut self,
        tx: &ValidatedTransaction,
    ) -> Result<LedgerEvent, EngineError> {
        let client_id = tx.client();
        let tx_id = tx.tx_id();

        let (record_client, record_amount, record_state) = {
            let record = self
                .transactions
                .get(&tx_id)
                .ok_or(EngineError::TransactionNotFound { tx_id })?;
            (record.client, record.amount, record.state)
        };

        Self::verify_client_ownership(tx_id, record_client, client_id)?;

        if record_state != TransactionState::Disputed {
            return Err(EngineError::TransactionNotDisputed { tx_id });
        }

        let account = self.accounts.get_or_create(client_id);
        account.chargeback(record_amount);

        self.transactions.get_mut(&tx_id).unwrap().state = TransactionState::ChargedBack;

        Ok(LedgerEvent::ChargedBack {
            client: client_id,
            tx: tx_id,
            reversed_amount: record_amount,
        })
    }

    // -- helpers --

    fn verify_client_ownership(
        tx_id: TransactionId,
        expected: ClientId,
        actual: ClientId,
    ) -> Result<(), EngineError> {
        if expected != actual {
            return Err(EngineError::ClientMismatch {
                tx_id,
                expected,
                actual,
            });
        }
        Ok(())
    }
}

impl Default for PaymentEngine {
    fn default() -> Self {
        Self::builder().build()
    }
}

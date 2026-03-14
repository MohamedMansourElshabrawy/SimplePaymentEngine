use crate::aggregator::event::LedgerEvent;
use crate::aggregator::metrics::Aggregator;
use crate::engine::processor::PaymentEngine;
use crate::engine::validator::TransactionValidator;
use crate::error::EngineError;
use crate::io::csv_sink::CsvSink;
use crate::io::csv_source::CsvTransactionSource;
use crate::io::sink::{AccountOutput, AccountSink};
use crate::io::source::TransactionSource;
use crate::model::transaction::{RawTransaction, TransactionType};
use crate::types::{ClientId, TransactionId};
use rust_decimal::Decimal;
use std::str::FromStr;

// ============================================================================
// Test Helper: VecSource for feeding raw transactions to the engine
// ============================================================================

struct VecSource {
    txs: Vec<RawTransaction>,
}

impl VecSource {
    fn new(txs: Vec<RawTransaction>) -> Self {
        Self { txs }
    }
}

impl TransactionSource for VecSource {
    fn transactions(
        &mut self,
    ) -> Box<dyn Iterator<Item = Result<RawTransaction, EngineError>> + '_> {
        Box::new(self.txs.drain(..).map(Ok))
    }
}

// ============================================================================
// Test Helper: Build raw transactions
// ============================================================================

fn raw_deposit(client: u16, tx: u32, amount: &str) -> RawTransaction {
    RawTransaction {
        tx_type: TransactionType::Deposit,
        client: ClientId::new(client),
        tx: TransactionId::new(tx),
        amount: Some(Decimal::from_str(amount).unwrap()),
    }
}

fn raw_withdrawal(client: u16, tx: u32, amount: &str) -> RawTransaction {
    RawTransaction {
        tx_type: TransactionType::Withdrawal,
        client: ClientId::new(client),
        tx: TransactionId::new(tx),
        amount: Some(Decimal::from_str(amount).unwrap()),
    }
}

fn raw_dispute(client: u16, tx: u32) -> RawTransaction {
    RawTransaction {
        tx_type: TransactionType::Dispute,
        client: ClientId::new(client),
        tx: TransactionId::new(tx),
        amount: None,
    }
}

fn raw_resolve(client: u16, tx: u32) -> RawTransaction {
    RawTransaction {
        tx_type: TransactionType::Resolve,
        client: ClientId::new(client),
        tx: TransactionId::new(tx),
        amount: None,
    }
}

fn raw_chargeback(client: u16, tx: u32) -> RawTransaction {
    RawTransaction {
        tx_type: TransactionType::Chargeback,
        client: ClientId::new(client),
        tx: TransactionId::new(tx),
        amount: None,
    }
}

// ============================================================================
// Engine Tests
// ============================================================================

#[test]
fn test_deposit_increases_available() {
    let mut engine = PaymentEngine::default();
    let mut agg = Aggregator::new();
    let mut source = VecSource::new(vec![raw_deposit(1, 1, "100.0")]);

    engine.process_all(&mut source, &mut agg);

    let outputs = engine.account_outputs();
    assert_eq!(outputs.len(), 1);
    assert_eq!(outputs[0].available, Decimal::from_str("100.0").unwrap());
    assert_eq!(outputs[0].held, Decimal::ZERO);
    assert_eq!(outputs[0].total, Decimal::from_str("100.0").unwrap());
    assert!(!outputs[0].locked);
}

#[test]
fn test_withdrawal_decreases_available() {
    let mut engine = PaymentEngine::default();
    let mut agg = Aggregator::new();
    let mut source = VecSource::new(vec![
        raw_deposit(1, 1, "100.0"),
        raw_withdrawal(1, 2, "30.0"),
    ]);

    engine.process_all(&mut source, &mut agg);

    let outputs = engine.account_outputs();
    assert_eq!(outputs[0].available, Decimal::from_str("70.0").unwrap());
    assert_eq!(outputs[0].total, Decimal::from_str("70.0").unwrap());
}

#[test]
fn test_withdrawal_insufficient_funds() {
    let mut engine = PaymentEngine::default();
    let mut agg = Aggregator::new();
    let mut source = VecSource::new(vec![
        raw_deposit(1, 1, "50.0"),
        raw_withdrawal(1, 2, "100.0"), // should fail
    ]);

    engine.process_all(&mut source, &mut agg);

    let summary = agg.summarize();
    assert_eq!(summary.total_processed, 1); // only deposit succeeded
    assert_eq!(summary.total_errors, 1);

    let outputs = engine.account_outputs();
    assert_eq!(outputs[0].available, Decimal::from_str("50.0").unwrap());
}

#[test]
fn test_dispute_holds_funds() {
    let mut engine = PaymentEngine::default();
    let mut agg = Aggregator::new();
    let mut source = VecSource::new(vec![
        raw_deposit(1, 1, "100.0"),
        raw_dispute(1, 1), // dispute the deposit
    ]);

    engine.process_all(&mut source, &mut agg);

    let outputs = engine.account_outputs();
    assert_eq!(outputs[0].available, Decimal::ZERO);
    assert_eq!(outputs[0].held, Decimal::from_str("100.0").unwrap());
    assert_eq!(outputs[0].total, Decimal::from_str("100.0").unwrap());
    assert!(!outputs[0].locked);
}

#[test]
fn test_resolve_releases_held_funds() {
    let mut engine = PaymentEngine::default();
    let mut agg = Aggregator::new();
    let mut source = VecSource::new(vec![
        raw_deposit(1, 1, "100.0"),
        raw_dispute(1, 1),
        raw_resolve(1, 1), // resolve the dispute
    ]);

    engine.process_all(&mut source, &mut agg);

    let outputs = engine.account_outputs();
    assert_eq!(outputs[0].available, Decimal::from_str("100.0").unwrap());
    assert_eq!(outputs[0].held, Decimal::ZERO);
    assert_eq!(outputs[0].total, Decimal::from_str("100.0").unwrap());
    assert!(!outputs[0].locked);
}

#[test]
fn test_chargeback_reverses_and_freezes() {
    let mut engine = PaymentEngine::default();
    let mut agg = Aggregator::new();
    let mut source = VecSource::new(vec![
        raw_deposit(1, 1, "100.0"),
        raw_dispute(1, 1),
        raw_chargeback(1, 1), // chargeback freezes account
    ]);

    engine.process_all(&mut source, &mut agg);

    let outputs = engine.account_outputs();
    assert_eq!(outputs[0].available, Decimal::ZERO);
    assert_eq!(outputs[0].held, Decimal::ZERO);
    assert_eq!(outputs[0].total, Decimal::ZERO);
    assert!(outputs[0].locked); // FROZEN
}

#[test]
fn test_frozen_account_rejects_deposit() {
    let mut engine = PaymentEngine::default();
    let mut agg = Aggregator::new();
    let mut source = VecSource::new(vec![
        raw_deposit(1, 1, "100.0"),
        raw_dispute(1, 1),
        raw_chargeback(1, 1), // freeze account
        raw_deposit(1, 2, "50.0"), // should be rejected
    ]);

    engine.process_all(&mut source, &mut agg);

    let summary = agg.summarize();
    assert_eq!(summary.total_processed, 3); // deposit, dispute, chargeback
    assert_eq!(summary.total_errors, 1); // frozen deposit rejected

    let outputs = engine.account_outputs();
    assert_eq!(outputs[0].total, Decimal::ZERO); // new deposit didn't go through
}

#[test]
fn test_frozen_account_rejects_withdrawal() {
    let mut engine = PaymentEngine::default();
    let mut agg = Aggregator::new();
    let mut source = VecSource::new(vec![
        raw_deposit(1, 1, "100.0"),
        raw_deposit(1, 2, "50.0"),
        raw_dispute(1, 1),
        raw_chargeback(1, 1), // freeze account, available=50, held=0
        raw_withdrawal(1, 3, "10.0"), // should be rejected
    ]);

    engine.process_all(&mut source, &mut agg);

    let summary = agg.summarize();
    assert_eq!(summary.total_errors, 1); // withdrawal rejected
}

#[test]
fn test_duplicate_transaction_rejected() {
    let mut engine = PaymentEngine::default();
    let mut agg = Aggregator::new();
    let mut source = VecSource::new(vec![
        raw_deposit(1, 1, "100.0"),
        raw_deposit(1, 1, "50.0"), // same tx_id — should fail
    ]);

    engine.process_all(&mut source, &mut agg);

    let summary = agg.summarize();
    assert_eq!(summary.total_processed, 1);
    assert_eq!(summary.total_errors, 1);

    let outputs = engine.account_outputs();
    assert_eq!(outputs[0].total, Decimal::from_str("100.0").unwrap()); // only first deposit
}

#[test]
fn test_dispute_nonexistent_tx() {
    let mut engine = PaymentEngine::default();
    let mut agg = Aggregator::new();
    let mut source = VecSource::new(vec![
        raw_deposit(1, 1, "100.0"),
        raw_dispute(1, 999), // tx 999 doesn't exist
    ]);

    engine.process_all(&mut source, &mut agg);

    let summary = agg.summarize();
    assert_eq!(summary.total_processed, 1);
    assert_eq!(summary.total_errors, 1);
}

#[test]
fn test_client_mismatch_dispute() {
    let mut engine = PaymentEngine::default();
    let mut agg = Aggregator::new();
    let mut source = VecSource::new(vec![
        raw_deposit(1, 1, "100.0"),
        raw_dispute(2, 1), // client 2 trying to dispute client 1's tx
    ]);

    engine.process_all(&mut source, &mut agg);

    let summary = agg.summarize();
    assert_eq!(summary.total_errors, 1); // client mismatch rejected
}

#[test]
fn test_re_dispute_after_resolve() {
    // After a resolve, the tx goes back to Active — we allow re-disputes
    let mut engine = PaymentEngine::default();
    let mut agg = Aggregator::new();
    let mut source = VecSource::new(vec![
        raw_deposit(1, 1, "100.0"),
        raw_dispute(1, 1),
        raw_resolve(1, 1),
        raw_dispute(1, 1), // re-dispute — allowed
    ]);

    engine.process_all(&mut source, &mut agg);

    let summary = agg.summarize();
    assert_eq!(summary.total_processed, 4); // all 4 succeeded
    assert_eq!(summary.total_errors, 0);

    let outputs = engine.account_outputs();
    assert_eq!(outputs[0].available, Decimal::ZERO);
    assert_eq!(outputs[0].held, Decimal::from_str("100.0").unwrap());
}

#[test]
fn test_chargeback_on_non_disputed_rejected() {
    let mut engine = PaymentEngine::default();
    let mut agg = Aggregator::new();
    let mut source = VecSource::new(vec![
        raw_deposit(1, 1, "100.0"),
        raw_chargeback(1, 1), // tx not disputed — should fail
    ]);

    engine.process_all(&mut source, &mut agg);

    let summary = agg.summarize();
    assert_eq!(summary.total_processed, 1);
    assert_eq!(summary.total_errors, 1);
}

#[test]
fn test_total_invariant_always_holds() {
    // total = available + held, always
    let mut engine = PaymentEngine::default();
    let mut agg = Aggregator::new();
    let mut source = VecSource::new(vec![
        raw_deposit(1, 1, "100.0"),
        raw_deposit(1, 2, "50.0"),
        raw_dispute(1, 1), // holds 100
        raw_withdrawal(1, 3, "30.0"), // withdraws 30 from available
    ]);

    engine.process_all(&mut source, &mut agg);

    let outputs = engine.account_outputs();
    assert_eq!(
        outputs[0].total,
        outputs[0].available + outputs[0].held,
        "invariant violated: total != available + held"
    );
}

#[test]
fn test_multiple_clients_independent() {
    let mut engine = PaymentEngine::default();
    let mut agg = Aggregator::new();
    let mut source = VecSource::new(vec![
        raw_deposit(1, 1, "100.0"),
        raw_deposit(2, 2, "200.0"),
        raw_withdrawal(1, 3, "50.0"),
        raw_dispute(2, 2),
    ]);

    engine.process_all(&mut source, &mut agg);

    let outputs = engine.account_outputs();
    assert_eq!(outputs.len(), 2);

    // Find each client
    let client1 = outputs.iter().find(|o| o.client == ClientId::new(1)).unwrap();
    let client2 = outputs.iter().find(|o| o.client == ClientId::new(2)).unwrap();

    assert_eq!(client1.available, Decimal::from_str("50.0").unwrap());
    assert_eq!(client1.held, Decimal::ZERO);

    assert_eq!(client2.available, Decimal::ZERO);
    assert_eq!(client2.held, Decimal::from_str("200.0").unwrap());
}

// ============================================================================
// Validator Tests
// ============================================================================

#[test]
fn test_valid_deposit() {
    let validator = TransactionValidator::new();
    let raw = raw_deposit(1, 1, "100.0");
    let result = validator.validate(raw);
    assert!(result.is_ok());
}

#[test]
fn test_deposit_missing_amount_rejected() {
    let validator = TransactionValidator::new();
    let raw = RawTransaction {
        tx_type: TransactionType::Deposit,
        client: ClientId::new(1),
        tx: TransactionId::new(1),
        amount: None, // missing!
    };
    let result = validator.validate(raw);
    assert!(matches!(result, Err(EngineError::MissingAmount { .. })));
}

#[test]
fn test_deposit_zero_amount_rejected() {
    let validator = TransactionValidator::new();
    let raw = RawTransaction {
        tx_type: TransactionType::Deposit,
        client: ClientId::new(1),
        tx: TransactionId::new(1),
        amount: Some(Decimal::ZERO),
    };
    let result = validator.validate(raw);
    assert!(matches!(result, Err(EngineError::InvalidAmount { .. })));
}

#[test]
fn test_deposit_negative_amount_rejected() {
    let validator = TransactionValidator::new();
    let raw = RawTransaction {
        tx_type: TransactionType::Deposit,
        client: ClientId::new(1),
        tx: TransactionId::new(1),
        amount: Some(Decimal::from_str("-10.0").unwrap()),
    };
    let result = validator.validate(raw);
    assert!(matches!(result, Err(EngineError::InvalidAmount { .. })));
}

#[test]
fn test_dispute_ignores_amount_field() {
    let validator = TransactionValidator::new();
    let raw = RawTransaction {
        tx_type: TransactionType::Dispute,
        client: ClientId::new(1),
        tx: TransactionId::new(1),
        amount: Some(Decimal::from_str("999.99").unwrap()), // ignored
    };
    let result = validator.validate(raw);
    assert!(result.is_ok());
    assert_eq!(result.unwrap().amount(), None);
}

// ============================================================================
// CSV Round-trip Tests
// ============================================================================

#[test]
fn test_csv_source_parses_correctly() {
    let csv_data = b"type, client, tx, amount\ndeposit, 1, 1, 100.5\nwithdrawal, 2, 2, 50.25";
    let mut source = CsvTransactionSource::from_reader(&csv_data[..]);

    let txs: Vec<_> = source.transactions().collect();
    assert_eq!(txs.len(), 2);

    let tx1 = txs[0].as_ref().unwrap();
    assert_eq!(tx1.tx_type, TransactionType::Deposit);
    assert_eq!(tx1.client, ClientId::new(1));
    assert_eq!(tx1.tx, TransactionId::new(1));
    assert_eq!(tx1.amount, Some(Decimal::from_str("100.5").unwrap()));

    let tx2 = txs[1].as_ref().unwrap();
    assert_eq!(tx2.tx_type, TransactionType::Withdrawal);
    assert_eq!(tx2.client, ClientId::new(2));
}

#[test]
fn test_csv_sink_output_format() {
    let mut buffer = Vec::new();
    let mut sink = CsvSink::new(&mut buffer);

    let outputs = vec![
        AccountOutput {
            client: ClientId::new(1),
            available: Decimal::from_str("1.5000").unwrap(),
            held: Decimal::ZERO,
            total: Decimal::from_str("1.5000").unwrap(),
            locked: false,
        },
        AccountOutput {
            client: ClientId::new(2),
            available: Decimal::from_str("2.0000").unwrap(),
            held: Decimal::ZERO,
            total: Decimal::from_str("2.0000").unwrap(),
            locked: false,
        },
    ];

    sink.write_accounts(&outputs).unwrap();

    let output = String::from_utf8(buffer).unwrap();
    assert!(output.contains("client,available,held,total,locked"));
    assert!(output.contains("1,1.5000,0,1.5000,false") || output.contains("1,1.5,0,1.5,false"));
    assert!(output.contains("2,2.0000,0,2.0000,false") || output.contains("2,2,0,2,false"));
}

// ============================================================================
// Aggregator Test
// ============================================================================

#[test]
fn test_aggregator_counts_events_and_errors() {
    let mut engine = PaymentEngine::default();
    let mut agg = Aggregator::new();
    let mut source = VecSource::new(vec![
        raw_deposit(1, 1, "100.0"),      // success
        raw_withdrawal(1, 2, "500.0"),   // error: insufficient funds
        raw_dispute(1, 1),               // success
        raw_chargeback(1, 999),          // error: tx not found
    ]);

    engine.process_all(&mut source, &mut agg);

    let summary = agg.summarize();
    assert_eq!(summary.total_processed, 2); // deposit + dispute
    assert_eq!(summary.total_errors, 2);    // insufficient + not found

    // Verify transaction breakdown
    assert_eq!(summary.tx_counts.get(&TransactionType::Deposit), Some(&1));
    assert_eq!(summary.tx_counts.get(&TransactionType::Dispute), Some(&1));
}

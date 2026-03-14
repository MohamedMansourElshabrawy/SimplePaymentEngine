use std::fmt::Write as FmtWrite;

use criterion::{
    black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput,
};
use rust_decimal::Decimal;
use std::str::FromStr;

use SimplePaymentEngine::aggregator::metrics::Aggregator;
use SimplePaymentEngine::engine::processor::PaymentEngine;
use SimplePaymentEngine::error::EngineError;
use SimplePaymentEngine::io::csv_sink::CsvSink;
use SimplePaymentEngine::io::csv_source::CsvTransactionSource;
use SimplePaymentEngine::io::sink::{AccountOutput, AccountSink};
use SimplePaymentEngine::io::source::TransactionSource;
use SimplePaymentEngine::model::transaction::RawTransaction;
use SimplePaymentEngine::types::{ClientId, TransactionId};

// ---------------------------------------------------------------------------
// Synthetic data generators
// ---------------------------------------------------------------------------

/// Generates a CSV string with `n` deposit transactions spread across `clients` clients.
fn generate_deposits_csv(n: usize, clients: u16) -> String {
    let mut csv = String::from("type,client,tx,amount\n");
    for i in 0..n {
        let client = (i as u16 % clients) + 1;
        let _ = writeln!(csv, "deposit,{},{},{:.4}", client, i + 1, 100.0);
    }
    csv
}

/// Generates a CSV with a realistic mixed workload:
///   50% deposits, 25% withdrawals, 15% disputes, 5% resolves, 5% chargebacks.
/// Ensures temporal ordering so disputes reference prior deposits, etc.
fn generate_mixed_csv(n: usize, clients: u16) -> String {
    let mut csv = String::from("type,client,tx,amount\n");
    let deposit_count = n / 2;
    let withdrawal_count = n / 4;
    let dispute_count = (n * 15) / 100;
    let resolve_count = (n * 5) / 100;
    let chargeback_count = n - deposit_count - withdrawal_count - dispute_count - resolve_count;

    let mut tx_id: u32 = 1;

    // Phase 1: deposits (build up balances)
    for i in 0..deposit_count {
        let client = (i as u16 % clients) + 1;
        let _ = writeln!(csv, "deposit,{},{},{:.4}", client, tx_id, 1000.0);
        tx_id += 1;
    }

    // Phase 2: withdrawals (small amounts so they succeed)
    for i in 0..withdrawal_count {
        let client = (i as u16 % clients) + 1;
        let _ = writeln!(csv, "withdrawal,{},{},{:.4}", client, tx_id, 1.0);
        tx_id += 1;
    }

    // Phase 3: disputes (reference deposit tx IDs: 1..deposit_count)
    for i in 0..dispute_count {
        let disputed_tx = (i as u32 % deposit_count as u32) + 1;
        let client = ((disputed_tx - 1) as u16 % clients) + 1;
        let _ = writeln!(csv, "dispute,{},{},", client, disputed_tx);
    }

    // Phase 4: resolves (reference the first `resolve_count` disputed txs)
    for i in 0..resolve_count {
        let resolved_tx = (i as u32 % dispute_count as u32) + 1;
        let client = ((resolved_tx - 1) as u16 % clients) + 1;
        let _ = writeln!(csv, "resolve,{},{},", client, resolved_tx);
    }

    // Phase 5: chargebacks (dispute fresh txs then chargeback them)
    // We dispute some txs that were resolved, then chargeback
    for i in 0..chargeback_count {
        let target_tx = (i as u32 % resolve_count.max(1) as u32) + 1;
        let client = ((target_tx - 1) as u16 % clients) + 1;
        let _ = writeln!(csv, "dispute,{},{},", client, target_tx);
        let _ = writeln!(csv, "chargeback,{},{},", client, target_tx);
    }

    csv
}

/// Creates a VecSource for engine-only benchmarks (no CSV parsing overhead).
struct VecSource {
    txs: Vec<RawTransaction>,
}

impl VecSource {
    fn deposits(n: usize, clients: u16) -> Self {
        let txs = (0..n)
            .map(|i| RawTransaction {
                tx_type: SimplePaymentEngine::model::transaction::TransactionType::Deposit,
                client: ClientId::new((i as u16 % clients) + 1),
                tx: TransactionId::new(i as u32 + 1),
                amount: Some(Decimal::from_str("100.0000").unwrap()),
            })
            .collect();
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

// ---------------------------------------------------------------------------
// Benchmarks
// ---------------------------------------------------------------------------

/// Measures raw engine throughput with deposit-only workload (no CSV overhead).
fn bench_engine_deposits(c: &mut Criterion) {
    let mut group = c.benchmark_group("engine_deposits");

    for &size in &[1_000u64, 10_000, 100_000, 1_000_000] {
        group.throughput(Throughput::Elements(size));
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &size| {
            b.iter(|| {
                let mut engine = PaymentEngine::default();
                let mut agg = Aggregator::new();
                let mut source = VecSource::deposits(size as usize, 100);
                engine.process_all(black_box(&mut source), &mut agg);
                black_box(engine.account_outputs())
            });
        });
    }
    group.finish();
}

/// Measures full pipeline throughput: CSV parse → validate → process → serialize.
fn bench_full_pipeline(c: &mut Criterion) {
    let mut group = c.benchmark_group("full_pipeline");

    for &size in &[1_000u64, 10_000, 100_000] {
        let csv_data = generate_deposits_csv(size as usize, 100);

        group.throughput(Throughput::Elements(size));
        group.bench_with_input(BenchmarkId::from_parameter(size), &csv_data, |b, data| {
            b.iter(|| {
                let mut source = CsvTransactionSource::from_reader(data.as_bytes());
                let mut engine = PaymentEngine::default();
                let mut agg = Aggregator::new();
                engine.process_all(black_box(&mut source), &mut agg);
                let outputs = engine.account_outputs();
                let mut sink = CsvSink::new(std::io::sink());
                sink.write_accounts(black_box(&outputs)).unwrap();
            });
        });
    }
    group.finish();
}

/// Measures mixed-workload throughput (deposits, withdrawals, disputes, resolves, chargebacks).
fn bench_mixed_workload(c: &mut Criterion) {
    let mut group = c.benchmark_group("mixed_workload");

    for &size in &[1_000u64, 10_000, 100_000] {
        let csv_data = generate_mixed_csv(size as usize, 50);

        group.throughput(Throughput::Elements(size));
        group.bench_with_input(BenchmarkId::from_parameter(size), &csv_data, |b, data| {
            b.iter(|| {
                let mut source = CsvTransactionSource::from_reader(data.as_bytes());
                let mut engine = PaymentEngine::default();
                let mut agg = Aggregator::new();
                engine.process_all(black_box(&mut source), &mut agg);
                black_box(engine.account_outputs())
            });
        });
    }
    group.finish();
}

/// Measures CSV parsing throughput in isolation (no engine processing).
fn bench_csv_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("csv_parsing");

    for &size in &[1_000u64, 10_000, 100_000] {
        let csv_data = generate_deposits_csv(size as usize, 100);

        group.throughput(Throughput::Bytes(csv_data.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &csv_data, |b, data| {
            b.iter(|| {
                let mut source = CsvTransactionSource::from_reader(data.as_bytes());
                let count: usize = source
                    .transactions()
                    .filter(|r| r.is_ok())
                    .count();
                black_box(count)
            });
        });
    }
    group.finish();
}

/// Measures CSV serialization throughput in isolation.
fn bench_csv_serialization(c: &mut Criterion) {
    let accounts: Vec<AccountOutput> = (0..1000u16)
        .map(|i| AccountOutput {
            client: ClientId::new(i + 1),
            available: Decimal::from_str("12345.6789").unwrap(),
            held: Decimal::from_str("100.0000").unwrap(),
            total: Decimal::from_str("12445.6789").unwrap(),
            locked: false,
        })
        .collect();

    c.bench_function("csv_serialize_1000_accounts", |b| {
        b.iter(|| {
            let mut sink = CsvSink::new(std::io::sink());
            sink.write_accounts(black_box(&accounts)).unwrap();
        });
    });
}

/// Measures scaling with number of unique clients (HashMap pressure).
fn bench_client_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("client_scaling");
    let tx_count = 100_000u64;

    for &clients in &[10u16, 100, 1_000, 10_000, 50_000] {
        group.throughput(Throughput::Elements(tx_count));
        group.bench_with_input(
            BenchmarkId::new("clients", clients),
            &clients,
            |b, &clients| {
                b.iter(|| {
                    let mut engine = PaymentEngine::default();
                    let mut agg = Aggregator::new();
                    let mut source = VecSource::deposits(tx_count as usize, clients);
                    engine.process_all(black_box(&mut source), &mut agg);
                    black_box(engine.account_outputs())
                });
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_engine_deposits,
    bench_full_pipeline,
    bench_mixed_workload,
    bench_csv_parsing,
    bench_csv_serialization,
    bench_client_scaling,
);
criterion_main!(benches);

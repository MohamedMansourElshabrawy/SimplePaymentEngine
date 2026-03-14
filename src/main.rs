use std::env;
use std::process;

use SimplePaymentEngine::aggregator::metrics::Aggregator;
use SimplePaymentEngine::aggregator::sink::{AggregatorSink, StderrAggregatorSink};
use SimplePaymentEngine::engine::processor::PaymentEngine;
use SimplePaymentEngine::io::csv_sink::CsvSink;
use SimplePaymentEngine::io::csv_source::CsvTransactionSource;
use SimplePaymentEngine::io::sink::{AccountSink, CompositeSink};

fn main() {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .init();

    let args: Vec<String> = env::args().collect();
    let filepath = match args.get(1) {
        Some(path) => path,
        None => {
            eprintln!("Usage: cargo run -- <transactions.csv>");
            process::exit(1);
        }
    };

    let mut source = match CsvTransactionSource::from_file(filepath) {
        Ok(source) => source,
        Err(e) => {
            eprintln!("Error opening file: {}", e);
            process::exit(1);
        }
    };

    let mut engine = PaymentEngine::default();
    let mut aggregator = Aggregator::new();

    engine.process_all(&mut source, &mut aggregator);

    let outputs = engine.account_outputs();
    let mut sink = CompositeSink::new(vec![Box::new(CsvSink::stdout())]);

    if let Err(e) = sink.write_accounts(&outputs) {
        eprintln!("Error writing output: {}", e);
        process::exit(1);
    }

    let summary = aggregator.summarize();
    let _ = StderrAggregatorSink.flush(&summary);
}

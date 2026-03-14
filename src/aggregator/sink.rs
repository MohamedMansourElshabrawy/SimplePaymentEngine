use crate::error::EngineError;

use super::metrics::AggregatorSummary;

pub trait AggregatorSink {
    fn flush(&self, summary: &AggregatorSummary) -> Result<(), EngineError>;
}

pub struct StderrAggregatorSink;

impl AggregatorSink for StderrAggregatorSink {
    fn flush(&self, summary: &AggregatorSummary) -> Result<(), EngineError> {
        eprintln!("--- Processing Summary ---");
        eprintln!("Transactions processed: {}", summary.total_processed);
        eprintln!("Errors encountered:    {}", summary.total_errors);
        eprintln!("Duration:              {:?}", summary.processing_duration);

        if !summary.tx_counts.is_empty() {
            eprintln!("Transaction breakdown:");
            for (tx_type, count) in &summary.tx_counts {
                eprintln!("  {}: {}", tx_type, count);
            }
        }

        if !summary.error_counts.is_empty() {
            eprintln!("Error breakdown:");
            for (category, count) in &summary.error_counts {
                eprintln!("  {:?}: {}", category, count);
            }
        }

        Ok(())
    }
}

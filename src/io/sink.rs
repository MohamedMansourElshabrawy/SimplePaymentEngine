use rust_decimal::Decimal;
use serde::Serialize;

use crate::error::EngineError;
use crate::types::ClientId;

#[derive(Debug, Serialize)]
pub struct AccountOutput {
    pub client: ClientId,
    pub available: Decimal,
    pub held: Decimal,
    pub total: Decimal,
    pub locked: bool,
}

pub trait AccountSink {
    fn write_accounts(&mut self, accounts: &[AccountOutput]) -> Result<(), EngineError>;
}

pub struct CompositeSink {
    sinks: Vec<Box<dyn AccountSink>>,
}

impl CompositeSink {
    pub fn new(sinks: Vec<Box<dyn AccountSink>>) -> Self { Self { sinks } }
}

impl AccountSink for CompositeSink {
    fn write_accounts(&mut self, accounts: &[AccountOutput]) -> Result<(), EngineError> {
        for sink in &mut self.sinks {
            sink.write_accounts(accounts)?;
        }
        Ok(())
    }
}

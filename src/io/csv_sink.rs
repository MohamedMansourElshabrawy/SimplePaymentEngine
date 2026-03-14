use std::io::{self, Write};

use crate::error::EngineError;

use super::sink::{AccountOutput, AccountSink};

pub struct CsvSink<W: Write> {
    writer: W,
}

impl CsvSink<io::Stdout> {
    pub fn stdout() -> Self { Self { writer: io::stdout() } }
}

impl CsvSink<std::fs::File> {
    pub fn file(path: &str) -> Result<Self, EngineError> {
        let file = std::fs::File::create(path)?;
        Ok(Self { writer: file })
    }
}

impl<W: Write> CsvSink<W> {
    pub fn new(writer: W) -> Self { Self { writer } }
}

impl<W: Write> AccountSink for CsvSink<W> {
    fn write_accounts(&mut self, accounts: &[AccountOutput]) -> Result<(), EngineError> {
        let mut csv_writer = csv::Writer::from_writer(&mut self.writer);
        for account in accounts {
            csv_writer.serialize(account)?;
        }
        csv_writer.flush()?;
        Ok(())
    }
}

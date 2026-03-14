use std::io::Read;

use csv::Trim;

use crate::error::EngineError;
use crate::model::transaction::RawTransaction;

use super::source::TransactionSource;

pub struct CsvTransactionSource<R: Read> {
    reader: csv::Reader<R>,
}

impl CsvTransactionSource<std::fs::File> {
    pub fn from_file(path: &str) -> Result<Self, EngineError> {
        let reader = csv::ReaderBuilder::new()
            .trim(Trim::All)
            .flexible(true)
            .from_path(path)?;
        Ok(Self { reader })
    }
}

impl<R: Read> CsvTransactionSource<R> {
    pub fn from_reader(reader: R) -> Self {
        let csv_reader = csv::ReaderBuilder::new()
            .trim(Trim::All)
            .flexible(true)
            .from_reader(reader);
        Self { reader: csv_reader }
    }
}

impl<R: Read> TransactionSource for CsvTransactionSource<R> {
    fn transactions(&mut self) -> Box<dyn Iterator<Item = Result<RawTransaction, EngineError>> + '_> {
        Box::new(
            self.reader
                .deserialize()
                .map(|result| result.map_err(EngineError::from)),
        )
    }
}

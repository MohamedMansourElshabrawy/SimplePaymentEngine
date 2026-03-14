use crate::error::EngineError;
use crate::model::transaction::RawTransaction;

pub trait TransactionSource {
    fn transactions(&mut self) -> Box<dyn Iterator<Item = Result<RawTransaction, EngineError>> + '_>;
}

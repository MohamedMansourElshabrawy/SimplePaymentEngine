pub mod in_memory;

use crate::model::account::ClientAccount;
use crate::model::transaction::TransactionRecord;
use crate::types::{ClientId, TransactionId};

pub trait AccountRepository {
    fn get(&self, id: &ClientId) -> Option<&ClientAccount>;
    fn get_mut(&mut self, id: &ClientId) -> Option<&mut ClientAccount>;
    fn get_or_create(&mut self, id: ClientId) -> &mut ClientAccount;
    fn iter(&self) -> Box<dyn Iterator<Item = (&ClientId, &ClientAccount)> + '_>;
}

pub trait TransactionRepository {
    fn store(&mut self, id: TransactionId, record: TransactionRecord);
    fn get(&self, id: &TransactionId) -> Option<&TransactionRecord>;
    fn get_mut(&mut self, id: &TransactionId) -> Option<&mut TransactionRecord>;
    fn contains(&self, id: &TransactionId) -> bool;
}

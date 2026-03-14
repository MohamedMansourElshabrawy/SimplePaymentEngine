use std::collections::HashMap;

use crate::model::account::ClientAccount;
use crate::model::transaction::TransactionRecord;
use crate::types::{ClientId, TransactionId};

use super::{AccountRepository, TransactionRepository};

#[derive(Debug, Default)]
pub struct InMemoryAccountRepo {
    accounts: HashMap<ClientId, ClientAccount>,
}

impl InMemoryAccountRepo {
    pub fn new() -> Self { Self::default() }
}

impl AccountRepository for InMemoryAccountRepo {
    fn get(&self, id: &ClientId) -> Option<&ClientAccount> { self.accounts.get(id) }
    fn get_mut(&mut self, id: &ClientId) -> Option<&mut ClientAccount> { self.accounts.get_mut(id) }
    fn get_or_create(&mut self, id: ClientId) -> &mut ClientAccount {
        self.accounts.entry(id).or_insert_with(ClientAccount::new)
    }
    fn iter(&self) -> Box<dyn Iterator<Item = (&ClientId, &ClientAccount)> + '_> {
        Box::new(self.accounts.iter())
    }
}

#[derive(Debug, Default)]
pub struct InMemoryTransactionRepo {
    transactions: HashMap<TransactionId, TransactionRecord>,
}

impl InMemoryTransactionRepo {
    pub fn new() -> Self { Self::default() }
}

impl TransactionRepository for InMemoryTransactionRepo {
    fn store(&mut self, id: TransactionId, record: TransactionRecord) {
        self.transactions.insert(id, record);
    }
    fn get(&self, id: &TransactionId) -> Option<&TransactionRecord> { self.transactions.get(id) }
    fn get_mut(&mut self, id: &TransactionId) -> Option<&mut TransactionRecord> { self.transactions.get_mut(id) }
    fn contains(&self, id: &TransactionId) -> bool { self.transactions.contains_key(id) }
}

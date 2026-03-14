use rust_decimal::Decimal;

use crate::types::{ClientId, TransactionId};

#[derive(Debug, Clone)]
pub enum LedgerEvent {
    Deposited { client: ClientId, tx: TransactionId, amount: Decimal },
    Withdrawn { client: ClientId, tx: TransactionId, amount: Decimal },
    DisputeOpened { client: ClientId, tx: TransactionId, held_amount: Decimal },
    DisputeResolved { client: ClientId, tx: TransactionId, released_amount: Decimal },
    ChargedBack { client: ClientId, tx: TransactionId, reversed_amount: Decimal },
}

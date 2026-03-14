use rust_decimal::Decimal;

#[derive(Debug, Clone)]
pub struct ClientAccount {
    available: Decimal,
    held: Decimal,
    locked: bool,
}

impl ClientAccount {
    pub fn new() -> Self {
        Self {
            available: Decimal::ZERO,
            held: Decimal::ZERO,
            locked: false,
        }
    }

    pub fn available(&self) -> Decimal { self.available }
    pub fn held(&self) -> Decimal { self.held }
    pub fn total(&self) -> Decimal { self.available + self.held }
    pub fn is_locked(&self) -> bool { self.locked }

    pub fn deposit(&mut self, amount: Decimal) { self.available += amount; }
    pub fn withdraw(&mut self, amount: Decimal) { self.available -= amount; }

    pub fn hold(&mut self, amount: Decimal) {
        self.available -= amount;
        self.held += amount;
    }

    pub fn release(&mut self, amount: Decimal) {
        self.held -= amount;
        self.available += amount;
    }

    pub fn chargeback(&mut self, amount: Decimal) {
        self.held -= amount;
        self.locked = true;
    }
}

impl Default for ClientAccount {
    fn default() -> Self { Self::new() }
}

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// A mock Stellar/Soroban address.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MockAddress(pub String);

impl MockAddress {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    pub fn account(id: u32) -> Self {
        Self(format!("GA{:055}", id))
    }

    pub fn contract(id: u32) -> Self {
        Self(format!("C{:062X}", id))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for MockAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Storage key type used in a mock storage map.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StorageKey {
    pub scope: String,
    pub key: String,
}

impl StorageKey {
    pub fn persistent(key: impl Into<String>) -> Self {
        Self {
            scope: "persistent".into(),
            key: key.into(),
        }
    }

    pub fn temporary(key: impl Into<String>) -> Self {
        Self {
            scope: "temporary".into(),
            key: key.into(),
        }
    }

    pub fn instance(key: impl Into<String>) -> Self {
        Self {
            scope: "instance".into(),
            key: key.into(),
        }
    }
}

/// A mock storage that simulates Soroban's contract storage.
#[derive(Debug, Clone, Default)]
pub struct MockStorage {
    entries: HashMap<StorageKey, serde_json::Value>,
}

impl MockStorage {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&mut self, key: StorageKey, value: serde_json::Value) {
        self.entries.insert(key, value);
    }

    pub fn get(&self, key: &StorageKey) -> Option<&serde_json::Value> {
        self.entries.get(key)
    }

    pub fn has(&self, key: &StorageKey) -> bool {
        self.entries.contains_key(key)
    }

    pub fn remove(&mut self, key: &StorageKey) -> Option<serde_json::Value> {
        self.entries.remove(key)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn entries_by_scope(&self, scope: &str) -> Vec<(&StorageKey, &serde_json::Value)> {
        self.entries
            .iter()
            .filter(|(k, _)| k.scope == scope)
            .collect()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

/// A single emitted contract event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockEvent {
    pub contract: MockAddress,
    pub topics: Vec<serde_json::Value>,
    pub data: serde_json::Value,
    pub ledger_sequence: u32,
}

impl MockEvent {
    pub fn new(
        contract: MockAddress,
        topics: Vec<serde_json::Value>,
        data: serde_json::Value,
    ) -> Self {
        Self {
            contract,
            topics,
            data,
            ledger_sequence: 1,
        }
    }

    pub fn topic_str(&self, index: usize) -> Option<&str> {
        self.topics.get(index)?.as_str()
    }
}

/// Event log that collects all events emitted during a mock contract execution.
#[derive(Debug, Default, Clone)]
pub struct MockEventLog {
    events: Vec<MockEvent>,
}

impl MockEventLog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn emit(&mut self, event: MockEvent) {
        self.events.push(event);
    }

    pub fn all(&self) -> &[MockEvent] {
        &self.events
    }

    pub fn by_topic(&self, topic: &str) -> Vec<&MockEvent> {
        self.events
            .iter()
            .filter(|e| e.topics.first().and_then(|t| t.as_str()) == Some(topic))
            .collect()
    }

    pub fn count(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub fn clear(&mut self) {
        self.events.clear();
    }
}

/// Simulated ledger state for a mock environment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockLedger {
    pub sequence: u32,
    pub timestamp: u64,
    pub network_passphrase: String,
    pub base_fee: u32,
    pub min_temp_entry_expiration: u32,
    pub min_persistent_entry_expiration: u32,
    pub max_entries_per_invoke: u32,
}

impl Default for MockLedger {
    fn default() -> Self {
        Self {
            sequence: 100,
            timestamp: 1_700_000_000,
            network_passphrase: "Test SDF Network ; September 2015".into(),
            base_fee: 100,
            min_temp_entry_expiration: 16,
            min_persistent_entry_expiration: 4096,
            max_entries_per_invoke: 64,
        }
    }
}

impl MockLedger {
    pub fn advance(&mut self, ledgers: u32) {
        self.sequence += ledgers;
        self.timestamp += u64::from(ledgers) * 5;
    }

    pub fn at_sequence(mut self, sequence: u32) -> Self {
        self.sequence = sequence;
        self
    }
}

/// Mock authorization record — tracks which addresses authorised which calls.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockAuthRecord {
    pub address: MockAddress,
    pub contract: MockAddress,
    pub function: String,
    pub approved: bool,
}

/// Mock authentication context that records and validates authorization.
#[derive(Debug, Default, Clone)]
pub struct MockAuthContext {
    records: Vec<MockAuthRecord>,
    /// Addresses that automatically approve all auth requests.
    auto_approve: Vec<MockAddress>,
}

impl MockAuthContext {
    pub fn new() -> Self {
        Self::default()
    }

    /// Mark an address as auto-approving (i.e. it never triggers auth failure).
    pub fn auto_approve(&mut self, address: MockAddress) {
        self.auto_approve.push(address);
    }

    pub fn require_auth(
        &mut self,
        address: &MockAddress,
        contract: &MockAddress,
        function: &str,
    ) -> bool {
        let approved = self.auto_approve.contains(address);
        self.records.push(MockAuthRecord {
            address: address.clone(),
            contract: contract.clone(),
            function: function.to_string(),
            approved,
        });
        approved
    }

    pub fn was_authorised(&self, address: &MockAddress, function: &str) -> bool {
        self.records
            .iter()
            .any(|r| &r.address == address && r.function == function && r.approved)
    }

    pub fn auth_count(&self) -> usize {
        self.records.len()
    }

    pub fn records(&self) -> &[MockAuthRecord] {
        &self.records
    }

    pub fn clear(&mut self) {
        self.records.clear();
    }
}

/// Simulated token balance map.
#[derive(Debug, Default, Clone)]
pub struct MockTokenBalances {
    balances: HashMap<(String, String), i128>,
}

impl MockTokenBalances {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&mut self, token: impl Into<String>, address: impl Into<String>, amount: i128) {
        self.balances.insert((token.into(), address.into()), amount);
    }

    pub fn get(&self, token: &str, address: &str) -> i128 {
        *self
            .balances
            .get(&(token.to_string(), address.to_string()))
            .unwrap_or(&0)
    }

    pub fn transfer(
        &mut self,
        token: &str,
        from: &str,
        to: &str,
        amount: i128,
    ) -> Result<(), String> {
        let from_bal = self.get(token, from);
        if from_bal < amount {
            return Err(format!("Insufficient balance: {} < {}", from_bal, amount));
        }
        *self
            .balances
            .entry((token.to_string(), from.to_string()))
            .or_default() -= amount;
        *self
            .balances
            .entry((token.to_string(), to.to_string()))
            .or_default() += amount;
        Ok(())
    }

    pub fn mint(&mut self, token: &str, to: &str, amount: i128) {
        *self
            .balances
            .entry((token.to_string(), to.to_string()))
            .or_default() += amount;
    }
}

/// A recorded call to a mock contract function.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockCall {
    pub contract: MockAddress,
    pub function: String,
    pub args: Vec<serde_json::Value>,
    pub caller: Option<MockAddress>,
    pub ledger_sequence: u32,
    pub return_value: Option<serde_json::Value>,
    pub error: Option<String>,
}

/// A mock contract client that records invocations and returns pre-configured responses.
#[derive(Debug, Clone)]
pub struct MockContractClient {
    pub address: MockAddress,
    call_log: Arc<Mutex<Vec<MockCall>>>,
    responses: Arc<Mutex<HashMap<String, serde_json::Value>>>,
    errors: Arc<Mutex<HashMap<String, String>>>,
}

impl MockContractClient {
    pub fn new(address: MockAddress) -> Self {
        Self {
            address,
            call_log: Arc::new(Mutex::new(Vec::new())),
            responses: Arc::new(Mutex::new(HashMap::new())),
            errors: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Pre-configure a return value for a specific function name.
    pub fn mock_return(&self, function: impl Into<String>, value: serde_json::Value) {
        self.responses
            .lock()
            .unwrap()
            .insert(function.into(), value);
    }

    /// Pre-configure an error for a specific function name.
    pub fn mock_error(&self, function: impl Into<String>, error: impl Into<String>) {
        self.errors
            .lock()
            .unwrap()
            .insert(function.into(), error.into());
    }

    /// Invoke a function on the mock contract.
    pub fn invoke(
        &self,
        function: &str,
        args: Vec<serde_json::Value>,
        caller: Option<MockAddress>,
        ledger_sequence: u32,
    ) -> Result<serde_json::Value, String> {
        let error = self.errors.lock().unwrap().get(function).cloned();
        let return_value = self.responses.lock().unwrap().get(function).cloned();

        let call = MockCall {
            contract: self.address.clone(),
            function: function.to_string(),
            args,
            caller,
            ledger_sequence,
            return_value: return_value.clone(),
            error: error.clone(),
        };
        self.call_log.lock().unwrap().push(call);

        if let Some(err) = error {
            Err(err)
        } else {
            Ok(return_value.unwrap_or(serde_json::Value::Null))
        }
    }

    pub fn call_count(&self, function: &str) -> usize {
        self.call_log
            .lock()
            .unwrap()
            .iter()
            .filter(|c| c.function == function)
            .count()
    }

    pub fn total_calls(&self) -> usize {
        self.call_log.lock().unwrap().len()
    }

    pub fn calls_for(&self, function: &str) -> Vec<MockCall> {
        self.call_log
            .lock()
            .unwrap()
            .iter()
            .filter(|c| c.function == function)
            .cloned()
            .collect()
    }

    pub fn all_calls(&self) -> Vec<MockCall> {
        self.call_log.lock().unwrap().clone()
    }

    pub fn reset(&self) {
        self.call_log.lock().unwrap().clear();
        self.responses.lock().unwrap().clear();
        self.errors.lock().unwrap().clear();
    }
}

/// A complete mock Soroban execution environment.
pub struct MockEnvironment {
    pub ledger: MockLedger,
    pub storage: MockStorage,
    pub events: MockEventLog,
    pub auth: MockAuthContext,
    pub balances: MockTokenBalances,
    clients: HashMap<String, MockContractClient>,
}

impl Default for MockEnvironment {
    fn default() -> Self {
        Self::new()
    }
}

impl MockEnvironment {
    pub fn new() -> Self {
        Self {
            ledger: MockLedger::default(),
            storage: MockStorage::new(),
            events: MockEventLog::new(),
            auth: MockAuthContext::new(),
            balances: MockTokenBalances::new(),
            clients: HashMap::new(),
        }
    }

    /// Register a mock contract client with the environment.
    pub fn register_contract(&mut self, client: MockContractClient) {
        self.clients.insert(client.address.0.clone(), client);
    }

    /// Look up a registered mock client.
    pub fn contract(&self, address: &MockAddress) -> Option<&MockContractClient> {
        self.clients.get(&address.0)
    }

    /// Advance the mock ledger by `n` ledgers.
    pub fn advance_ledger(&mut self, n: u32) {
        self.ledger.advance(n);
    }

    /// Emit an event into the environment's event log.
    pub fn emit_event(
        &mut self,
        contract: MockAddress,
        topics: Vec<serde_json::Value>,
        data: serde_json::Value,
    ) {
        self.events.emit(MockEvent {
            contract,
            topics,
            data,
            ledger_sequence: self.ledger.sequence,
        });
    }

    /// Reset the environment to a clean state (preserves ledger configuration).
    pub fn reset(&mut self) {
        self.storage.clear();
        self.events.clear();
        self.auth.clear();
        for client in self.clients.values() {
            client.reset();
        }
    }
}

/// Build a standard mock environment with a counter contract pre-registered.
pub fn counter_env() -> MockEnvironment {
    let mut env = MockEnvironment::new();
    let contract_addr = MockAddress::contract(1);

    let client = MockContractClient::new(contract_addr.clone());
    client.mock_return("get_count", serde_json::json!(0u64));
    client.mock_return("increment", serde_json::json!(1u64));
    client.mock_return("reset", serde_json::Value::Null);

    env.storage
        .set(StorageKey::instance("count"), serde_json::json!(0u64));
    env.auth.auto_approve(MockAddress::account(1));
    env.register_contract(client);
    env
}

/// Build a standard mock environment with a token contract pre-registered.
pub fn token_env(initial_supply: i128) -> MockEnvironment {
    let mut env = MockEnvironment::new();
    let token_addr = MockAddress::contract(10);
    let admin = MockAddress::account(10);
    let user_a = MockAddress::account(12);

    let client = MockContractClient::new(token_addr.clone());
    client.mock_return("total_supply", serde_json::json!(initial_supply));
    client.mock_return("decimals", serde_json::json!(7u32));
    client.mock_return("name", serde_json::json!("TestToken"));
    client.mock_return("symbol", serde_json::json!("TST"));
    client.mock_return("balance", serde_json::json!(0i128));
    client.mock_return("mint", serde_json::Value::Null);
    client.mock_return("transfer", serde_json::Value::Null);

    env.storage.set(
        StorageKey::persistent("total_supply"),
        serde_json::json!(initial_supply),
    );
    env.storage
        .set(StorageKey::instance("decimals"), serde_json::json!(7u32));
    env.balances.mint(&token_addr.0, &admin.0, initial_supply);
    env.auth.auto_approve(admin.clone());
    env.auth.auto_approve(user_a);
    env.register_contract(client);
    env
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_storage_set_get_remove() {
        let mut storage = MockStorage::new();
        let key = StorageKey::persistent("balance");
        storage.set(key.clone(), serde_json::json!(1000u64));
        assert_eq!(storage.get(&key), Some(&serde_json::json!(1000u64)));
        assert!(storage.has(&key));
        storage.remove(&key);
        assert!(!storage.has(&key));
    }

    #[test]
    fn mock_event_log_filter_by_topic() {
        let mut log = MockEventLog::new();
        log.emit(MockEvent::new(
            MockAddress::contract(1),
            vec![serde_json::json!("transfer")],
            serde_json::json!({"amount": 100}),
        ));
        log.emit(MockEvent::new(
            MockAddress::contract(1),
            vec![serde_json::json!("mint")],
            serde_json::json!({"amount": 500}),
        ));
        assert_eq!(log.by_topic("transfer").len(), 1);
        assert_eq!(log.by_topic("mint").len(), 1);
        assert_eq!(log.by_topic("burn").len(), 0);
    }

    #[test]
    fn mock_auth_context_records_approvals() {
        let mut auth = MockAuthContext::new();
        let admin = MockAddress::account(1);
        let contract = MockAddress::contract(1);
        auth.auto_approve(admin.clone());
        let approved = auth.require_auth(&admin, &contract, "mint");
        assert!(approved);
        assert!(auth.was_authorised(&admin, "mint"));
    }

    #[test]
    fn mock_auth_context_rejects_unknown() {
        let mut auth = MockAuthContext::new();
        let user = MockAddress::account(99);
        let contract = MockAddress::contract(1);
        let approved = auth.require_auth(&user, &contract, "mint");
        assert!(!approved);
    }

    #[test]
    fn mock_token_balances_transfer_and_insufficient() {
        let mut balances = MockTokenBalances::new();
        balances.mint("TST", "alice", 1000);
        assert!(balances.transfer("TST", "alice", "bob", 400).is_ok());
        assert_eq!(balances.get("TST", "alice"), 600);
        assert_eq!(balances.get("TST", "bob"), 400);
        assert!(balances.transfer("TST", "alice", "bob", 700).is_err());
    }

    #[test]
    fn mock_contract_client_records_calls() {
        let client = MockContractClient::new(MockAddress::contract(1));
        client.mock_return("increment", serde_json::json!(1u64));
        let result = client.invoke("increment", vec![], Some(MockAddress::account(1)), 100);
        assert_eq!(result.unwrap(), serde_json::json!(1u64));
        assert_eq!(client.call_count("increment"), 1);
    }

    #[test]
    fn mock_contract_client_returns_error() {
        let client = MockContractClient::new(MockAddress::contract(1));
        client.mock_error("restricted", "unauthorized");
        let result = client.invoke("restricted", vec![], None, 100);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "unauthorized");
    }

    #[test]
    fn counter_env_factory() {
        let env = counter_env();
        assert_eq!(
            env.storage.get(&StorageKey::instance("count")),
            Some(&serde_json::json!(0u64))
        );
        let client = env.contract(&MockAddress::contract(1)).unwrap();
        let val = client.invoke("increment", vec![], Some(MockAddress::account(1)), 100);
        assert_eq!(val.unwrap(), serde_json::json!(1u64));
    }

    #[test]
    fn token_env_factory() {
        let env = token_env(1_000_000);
        assert_eq!(
            env.balances
                .get(&MockAddress::contract(10).0, &MockAddress::account(10).0),
            1_000_000
        );
        let client = env.contract(&MockAddress::contract(10)).unwrap();
        let sym = client.invoke("symbol", vec![], None, 100).unwrap();
        assert_eq!(sym, serde_json::json!("TST"));
    }

    #[test]
    fn mock_ledger_advance() {
        let mut ledger = MockLedger::default();
        let initial_seq = ledger.sequence;
        let initial_ts = ledger.timestamp;
        ledger.advance(10);
        assert_eq!(ledger.sequence, initial_seq + 10);
        assert_eq!(ledger.timestamp, initial_ts + 50);
    }
}

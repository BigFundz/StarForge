# Testing Binding Generator with Real Contracts

## Prerequisites:
1. Install Soroban CLI: `cargo install --locked soroban-cli`
2. Create a simple Soroban contract

## Example Test Contract:

Create `contracts/test_token/src/lib.rs`:
```rust
#![no_std]
use soroban_sdk::{contract, contractimpl, token, Env, Address};

#[contract]
pub struct TestToken;

#[contractimpl]
impl TestToken {
    pub fn balance(env: Env, account: Address) -> i128 {
        let client = token::Client::new(&env, &account);
        client.balance(&account)
    }
    
    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
        let client = token::Client::new(&env, &from);
        client.transfer(&from, &to, &amount);
    }
}
```

## Build the contract:
```bash
cd contracts/test_token
cargo build --target wasm32-unknown-unknown --release
```

## Generate Bindings:
```bash
cd ../..
starforge contract generate-bindings \
  target/wasm32-unknown-unknown/release/test_token.wasm \
  --lang rust > test_token_client.rs
```

## Expected Generated Code:
The binding generator should create:
- Type-safe `balance()` and `transfer()` methods
- Proper parameter types (`Address`, `i128`)
- Event types if defined in contract
- Serialization helpers

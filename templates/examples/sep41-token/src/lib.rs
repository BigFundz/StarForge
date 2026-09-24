#![no_std]
//! SEP-41 fungible token contract for Soroban.
//!
//! Implements the standard fungible-token interface described in SEP-41:
//! initialize, mint (admin-only), transfer, approve/transfer_from allowance
//! flow, burn, and balance/allowance read helpers.
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, String};

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    Decimals,
    Name,
    Symbol,
    Balance(Address),
    Allowance(Address, Address),
}

#[contract]
pub struct {{PROJECT_NAME_PASCAL}};

#[contractimpl]
impl {{PROJECT_NAME_PASCAL}} {
    /// Initialize the token. Can only be called once.
    pub fn initialize(env: Env, admin: Address, decimals: u32, name: String, symbol: String) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Decimals, &decimals);
        env.storage().instance().set(&DataKey::Name, &name);
        env.storage().instance().set(&DataKey::Symbol, &symbol);
    }

    /// Mint `amount` tokens to `to`. Admin only.
    pub fn mint(env: Env, to: Address, amount: i128) {
        let admin: Address = env.storage().instance().get(&DataKey::Admin).expect("not initialized");
        admin.require_auth();
        let bal = Self::balance(env.clone(), to.clone());
        env.storage().persistent().set(&DataKey::Balance(to), &(bal + amount));
    }

    /// Transfer `amount` tokens from `from` to `to`.
    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
        from.require_auth();
        let from_bal = Self::balance(env.clone(), from.clone());
        if from_bal < amount {
            panic!("insufficient balance");
        }
        let to_bal = Self::balance(env.clone(), to.clone());
        env.storage().persistent().set(&DataKey::Balance(from), &(from_bal - amount));
        env.storage().persistent().set(&DataKey::Balance(to), &(to_bal + amount));
    }

    /// Return the token balance of `addr`.
    pub fn balance(env: Env, addr: Address) -> i128 {
        env.storage().persistent().get(&DataKey::Balance(addr)).unwrap_or(0)
    }

    /// Approve `spender` to spend `amount` on behalf of `from`.
    pub fn approve(env: Env, from: Address, spender: Address, amount: i128) {
        from.require_auth();
        env.storage().persistent().set(&DataKey::Allowance(from, spender), &amount);
    }

    /// Return the amount `spender` is allowed to spend on behalf of `from`.
    pub fn allowance(env: Env, from: Address, spender: Address) -> i128 {
        env.storage().persistent().get(&DataKey::Allowance(from, spender)).unwrap_or(0)
    }

    /// Transfer `amount` from `from` to `to` using `spender`'s allowance.
    pub fn transfer_from(env: Env, spender: Address, from: Address, to: Address, amount: i128) {
        spender.require_auth();
        let allowance = Self::allowance(env.clone(), from.clone(), spender.clone());
        if allowance < amount {
            panic!("insufficient allowance");
        }
        env.storage()
            .persistent()
            .set(&DataKey::Allowance(from.clone(), spender), &(allowance - amount));
        Self::transfer(env, from, to, amount);
    }

    /// Burn `amount` tokens from `from`.
    pub fn burn(env: Env, from: Address, amount: i128) {
        from.require_auth();
        let bal = Self::balance(env.clone(), from.clone());
        if bal < amount {
            panic!("insufficient balance");
        }
        env.storage().persistent().set(&DataKey::Balance(from), &(bal - amount));
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::testutils::Address as _;

    #[test]
    fn test_mint_transfer_burn() {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let alice = Address::generate(&env);
        let bob = Address::generate(&env);

        let id = env.register_contract(None, {{PROJECT_NAME_PASCAL}});
        let client = {{PROJECT_NAME_PASCAL}}Client::new(&env, &id);

        client.initialize(&admin, &7u32, &String::from_str(&env, "MyToken"), &String::from_str(&env, "MTK"));
        client.mint(&alice, &1000);
        assert_eq!(client.balance(&alice), 1000);

        client.transfer(&alice, &bob, &400);
        assert_eq!(client.balance(&alice), 600);
        assert_eq!(client.balance(&bob), 400);

        client.burn(&alice, &100);
        assert_eq!(client.balance(&alice), 500);
    }

    #[test]
    fn test_approve_transfer_from() {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let alice = Address::generate(&env);
        let bob = Address::generate(&env);
        let carol = Address::generate(&env);

        let id = env.register_contract(None, {{PROJECT_NAME_PASCAL}});
        let client = {{PROJECT_NAME_PASCAL}}Client::new(&env, &id);

        client.initialize(&admin, &7u32, &String::from_str(&env, "MyToken"), &String::from_str(&env, "MTK"));
        client.mint(&alice, &500);
        client.approve(&alice, &bob, &200);
        assert_eq!(client.allowance(&alice, &bob), 200);

        client.transfer_from(&bob, &alice, &carol, &150);
        assert_eq!(client.balance(&carol), 150);
        assert_eq!(client.allowance(&alice, &bob), 50);
    }
}

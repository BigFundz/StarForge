use async_graphql::*;
use uuid::Uuid;

#[derive(SimpleObject, Clone)]
pub struct Wallet {
    pub id: String,
    pub public_key: String,
    pub name: String,
    pub balance: f64,
    pub network: String,
    pub created_at: String,
    pub funded: bool,
}

#[derive(SimpleObject, Clone)]
pub struct Contract {
    pub id: String,
    pub address: String,
    pub name: String,
    pub owner: String,
    pub network: String,
    pub created_at: String,
    pub version: String,
    pub language: String,
}

#[derive(SimpleObject, Clone)]
pub struct Template {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub tags: Vec<String>,
    pub downloads: u32,
    pub verified: bool,
    pub rating: f32,
    pub created_at: String,
}

#[derive(SimpleObject, Clone)]
pub struct Transaction {
    pub id: String,
    pub source: String,
    pub destination: String,
    pub amount: f64,
    pub fee: f64,
    pub status: String,
    pub created_at: String,
    pub confirmed_at: Option<String>,
    pub hash: Option<String>,
}

#[derive(SimpleObject, Clone)]
pub struct Account {
    pub id: String,
    pub public_key: String,
    pub balance: f64,
    pub sequence: u64,
    pub native_balance: f64,
    pub created_at: String,
}

#[derive(SimpleObject, Clone)]
pub struct Network {
    pub id: String,
    pub name: String,
    pub network_type: String,
    pub horizon_url: String,
    pub rpc_url: String,
}

#[derive(SimpleObject, Clone)]
pub struct User {
    pub id: String,
    pub email: String,
    pub username: String,
    pub created_at: String,
    pub wallets_count: i32,
}

#[derive(InputObject)]
pub struct CreateWalletInput {
    pub name: String,
    pub network: String,
}

#[derive(InputObject)]
pub struct CreateContractInput {
    pub name: String,
    pub address: String,
    pub language: String,
    pub network: String,
}

#[derive(InputObject)]
pub struct CreateTransactionInput {
    pub source: String,
    pub destination: String,
    pub amount: f64,
    pub network: String,
}

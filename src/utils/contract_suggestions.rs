//! AI Contract Function Suggestions
//!
//! Provides context-aware function suggestions for Soroban smart contracts
//! based on contract type, best practices, and common patterns.
//!
//! This module implements:
//! - Context-aware function suggestions
//! - Real-time auto-completion in CLI
//! - Best practice recommendations
//! - Parameter type suggestions
//! - Error handling patterns
//! - Integration with existing contract code

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Represents a suggestion category for contract functions
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum SuggestionCategory {
    /// Standard functions (initialize, mint, transfer)
    Standard,
    /// Access control functions (admin, owner, permissions)
    AccessControl,
    /// Storage pattern suggestions (get, set, has, remove)
    Storage,
    /// Event emission patterns (publish, emit)
    Events,
    /// Error handling functions (validate, check, assert)
    ErrorHandling,
    /// Query functions (read-only, getters)
    Queries,
    /// Initialization functions (constructor, setup)
    Initialization,
    /// Token-related functions (mint, burn, transfer, approve)
    Token,
    /// Governance functions (propose, vote, execute)
    Governance,
}

impl std::fmt::Display for SuggestionCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Standard => write!(f, "Standard"),
            Self::AccessControl => write!(f, "Access Control"),
            Self::Storage => write!(f, "Storage"),
            Self::Events => write!(f, "Events"),
            Self::ErrorHandling => write!(f, "Error Handling"),
            Self::Queries => write!(f, "Queries"),
            Self::Initialization => write!(f, "Initialization"),
            Self::Token => write!(f, "Token"),
            Self::Governance => write!(f, "Governance"),
        }
    }
}

/// Represents the priority of a suggestion
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum SuggestionPriority {
    Critical,
    High,
    Medium,
    Low,
}

impl std::fmt::Display for SuggestionPriority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Critical => write!(f, "Critical"),
            Self::High => write!(f, "High"),
            Self::Medium => write!(f, "Medium"),
            Self::Low => write!(f, "Low"),
        }
    }
}

/// Represents a function suggestion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionSuggestion {
    /// Name of the suggested function
    pub name: String,
    /// Category of the suggestion
    pub category: SuggestionCategory,
    /// Priority of the suggestion
    pub priority: SuggestionPriority,
    /// Description of what the function does
    pub description: String,
    /// Function signature with parameters
    pub signature: String,
    /// Default implementation body
    pub implementation: String,
    /// Required imports for this function
    pub imports: Vec<String>,
    /// Best practice notes
    pub best_practices: Vec<String>,
    /// Confidence score (0-100)
    pub confidence: u8,
    /// Context in which this suggestion is relevant
    pub context: String,
}

/// Represents the context of a contract
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractContext {
    /// Type of contract (token, governance, defi, nft, etc.)
    pub contract_type: ContractType,
    /// Existing functions in the contract
    pub existing_functions: Vec<String>,
    /// Contract name
    pub contract_name: String,
    /// Storage keys already in use
    pub storage_keys: Vec<String>,
    /// Events already defined
    pub events: Vec<String>,
    /// Error variants already defined
    pub errors: Vec<String>,
    /// Whether the contract has an initialize function
    pub has_initialize: bool,
    /// Whether the contract has admin functions
    pub has_admin: bool,
    /// Whether the contract has token functions
    pub has_token: bool,
}

/// Types of Soroban contracts
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ContractType {
    /// Token contract (SEP-41 compliant)
    Token,
    /// Non-fungible token contract
    Nft,
    /// Governance contract
    Governance,
    /// DeFi contract (DEX, lending, etc.)
    Defi,
    /// Access control contract
    AccessControl,
    /// Storage contract
    Storage,
    /// Generic contract
    Generic,
    /// Custom contract type
    Custom(String),
}

impl std::fmt::Display for ContractType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Token => write!(f, "Token"),
            Self::Nft => write!(f, "NFT"),
            Self::Governance => write!(f, "Governance"),
            Self::Defi => write!(f, "DeFi"),
            Self::AccessControl => write!(f, "Access Control"),
            Self::Storage => write!(f, "Storage"),
            Self::Generic => write!(f, "Generic"),
            Self::Custom(name) => write!(f, "{}", name),
        }
    }
}

/// The main suggestion engine
pub struct ContractSuggestionEngine {
    /// Templates for different contract types
    templates: HashMap<ContractType, Vec<FunctionSuggestion>>,
    /// Best practices database
    best_practices: HashMap<String, Vec<String>>,
}

impl ContractSuggestionEngine {
    /// Create a new suggestion engine with built-in templates
    pub fn new() -> Self {
        let mut engine = Self {
            templates: HashMap::new(),
            best_practices: HashMap::new(),
        };
        engine.init_templates();
        engine.init_best_practices();
        engine
    }

    /// Initialize function templates for different contract types
    fn init_templates(&mut self) {
        // Token contract templates
        self.templates.insert(
            ContractType::Token,
            vec![
                FunctionSuggestion {
                    name: "initialize".to_string(),
                    category: SuggestionCategory::Initialization,
                    priority: SuggestionPriority::Critical,
                    description: "Initialize the token contract with admin and metadata".to_string(),
                    signature: "pub fn initialize(env: Env, admin: Address, name: String, symbol: String, decimals: u32)".to_string(),
                    implementation: r#"{
    admin.require_auth();
    env.storage().instance().set(&symbol_short!("ADMIN"), &admin);
    env.storage().instance().set(&symbol_short!("NAME"), &name);
    env.storage().instance().set(&symbol_short!("SYMBOL"), &symbol);
    env.storage().instance().set(&symbol_short!("DECIMALS"), &decimals);
}"#.to_string(),
                    imports: vec!["soroban_sdk::{symbol_short, Address, Env, String}".to_string()],
                    best_practices: vec![
                        "Use require_auth() for admin".to_string(),
                        "Store metadata in instance storage".to_string(),
                        "Follow SEP-41 standard".to_string(),
                    ],
                    confidence: 95,
                    context: "Token contract initialization".to_string(),
                },
                FunctionSuggestion {
                    name: "transfer".to_string(),
                    category: SuggestionCategory::Token,
                    priority: SuggestionPriority::High,
                    description: "Transfer tokens between addresses".to_string(),
                    signature: "pub fn transfer(env: Env, from: Address, to: Address, amount: i128) -> Result<(), Error>".to_string(),
                    implementation: r#"{
    from.require_auth();
    if amount <= 0 {
        return Err(Error::InvalidAmount);
    }
    let from_balance = env.storage().persistent().get(&from_key).unwrap_or(0);
    if from_balance < amount {
        return Err(Error::InsufficientBalance);
    }
    env.storage().persistent().set(&from_key, &(from_balance - amount));
    env.storage().persistent().set(&to_key, &(env.storage().persistent().get(&to_key).unwrap_or(0) + amount));
    Ok(())
}"#.to_string(),
                    imports: vec!["soroban_sdk::{Address, Env}".to_string()],
                    best_practices: vec![
                        "Always check authorization".to_string(),
                        "Validate amount is positive".to_string(),
                        "Check sufficient balance".to_string(),
                        "Emit transfer event".to_string(),
                    ],
                    confidence: 90,
                    context: "Token transfer function".to_string(),
                },
                FunctionSuggestion {
                    name: "balance".to_string(),
                    category: SuggestionCategory::Queries,
                    priority: SuggestionPriority::Medium,
                    description: "Get the balance of an address".to_string(),
                    signature: "pub fn balance(env: Env, id: Address) -> i128".to_string(),
                    implementation: "{\n    env.storage().persistent().get(&key_from_address(&id)).unwrap_or(0)\n}".to_string(),
                    imports: vec!["soroban_sdk::{Address, Env}".to_string()],
                    best_practices: vec![
                        "Return 0 for non-existent accounts".to_string(),
                        "Use persistent storage for balances".to_string(),
                    ],
                    confidence: 85,
                    context: "Token balance query".to_string(),
                },
            ],
        );

        // NFT contract templates
        self.templates.insert(
            ContractType::Nft,
            vec![
                FunctionSuggestion {
                    name: "initialize".to_string(),
                    category: SuggestionCategory::Initialization,
                    priority: SuggestionPriority::Critical,
                    description: "Initialize the NFT contract".to_string(),
                    signature:
                        "pub fn initialize(env: Env, admin: Address, name: String, symbol: String)"
                            .to_string(),
                    implementation: r#"{
    admin.require_auth();
    env.storage().instance().set(&symbol_short!("ADMIN"), &admin);
    env.storage().instance().set(&symbol_short!("NAME"), &name);
    env.storage().instance().set(&symbol_short!("SYMBOL"), &symbol);
    env.storage().instance().set(&symbol_short!("NEXT_ID"), &0u64);
}"#
                    .to_string(),
                    imports: vec!["soroban_sdk::{symbol_short, Address, Env, String}".to_string()],
                    best_practices: vec![
                        "Use instance storage for admin".to_string(),
                        "Initialize token ID counter".to_string(),
                    ],
                    confidence: 95,
                    context: "NFT contract initialization".to_string(),
                },
                FunctionSuggestion {
                    name: "mint".to_string(),
                    category: SuggestionCategory::Token,
                    priority: SuggestionPriority::High,
                    description: "Mint a new NFT to an address".to_string(),
                    signature: "pub fn mint(env: Env, to: Address) -> u64".to_string(),
                    implementation: r#"{
    let admin: Address = env.storage().instance().get(&symbol_short!("ADMIN")).unwrap();
    admin.require_auth();
    let next_id: u64 = env.storage().instance().get(&symbol_short!("NEXT_ID")).unwrap();
    env.storage().persistent().set(&key_from_id(next_id), &to);
    env.storage().instance().set(&symbol_short!("NEXT_ID"), &(next_id + 1));
    next_id
}"#
                    .to_string(),
                    imports: vec!["soroban_sdk::{symbol_short, Address, Env}".to_string()],
                    best_practices: vec![
                        "Only admin can mint".to_string(),
                        "Auto-increment token IDs".to_string(),
                        "Emit mint event".to_string(),
                    ],
                    confidence: 90,
                    context: "NFT minting function".to_string(),
                },
            ],
        );

        // Governance contract templates
        self.templates.insert(
            ContractType::Governance,
            vec![
                FunctionSuggestion {
                    name: "propose".to_string(),
                    category: SuggestionCategory::Governance,
                    priority: SuggestionPriority::High,
                    description: "Create a new governance proposal".to_string(),
                    signature: "pub fn propose(env: Env, proposer: Address, description: String, contract_id: Address, function_name: String, args: Vec<Val>) -> u64".to_string(),
                    implementation: r#"{
    proposer.require_auth();
    let proposal_id: u64 = env.storage().instance().get(&symbol_short!("NEXT_ID")).unwrap_or(0);
    let proposal = Proposal {
        id: proposal_id,
        proposer: proposer.clone(),
        description,
        contract_id,
        function_name,
        args,
        votes_for: 0,
        votes_against: 0,
        status: ProposalStatus::Active,
    };
    env.storage().persistent().set(&key_from_id(proposal_id), &proposal);
    env.storage().instance().set(&symbol_short!("NEXT_ID"), &(proposal_id + 1));
    proposal_id
}"#.to_string(),
                    imports: vec!["soroban_sdk::{symbol_short, Address, Env, String, Vec, Val}".to_string()],
                    best_practices: vec![
                        "Require proposer authentication".to_string(),
                        "Auto-increment proposal IDs".to_string(),
                        "Store proposal metadata".to_string(),
                    ],
                    confidence: 85,
                    context: "Governance proposal creation".to_string(),
                },
            ],
        );

        // Generic contract templates
        self.templates.insert(
            ContractType::Generic,
            vec![
                FunctionSuggestion {
                    name: "initialize".to_string(),
                    category: SuggestionCategory::Initialization,
                    priority: SuggestionPriority::High,
                    description: "Initialize the contract with an admin".to_string(),
                    signature: "pub fn initialize(env: Env, admin: Address)".to_string(),
                    implementation: r#"{
    admin.require_auth();
    env.storage().instance().set(&symbol_short!("ADMIN"), &admin);
}"#
                    .to_string(),
                    imports: vec!["soroban_sdk::{symbol_short, Address, Env}".to_string()],
                    best_practices: vec![
                        "Use require_auth() for admin".to_string(),
                        "Store admin in instance storage".to_string(),
                    ],
                    confidence: 90,
                    context: "Generic contract initialization".to_string(),
                },
                FunctionSuggestion {
                    name: "get_admin".to_string(),
                    category: SuggestionCategory::AccessControl,
                    priority: SuggestionPriority::Medium,
                    description: "Get the current admin address".to_string(),
                    signature: "pub fn get_admin(env: Env) -> Address".to_string(),
                    implementation:
                        "{\n    env.storage().instance().get(&symbol_short!(\"ADMIN\")).unwrap()\n}"
                            .to_string(),
                    imports: vec!["soroban_sdk::{symbol_short, Address, Env}".to_string()],
                    best_practices: vec!["Provide admin getter for transparency".to_string()],
                    confidence: 80,
                    context: "Admin getter function".to_string(),
                },
            ],
        );
    }

    /// Initialize best practices database
    fn init_best_practices(&mut self) {
        self.best_practices.insert(
            "authorization".to_string(),
            vec![
                "Always use require_auth() for state-modifying functions".to_string(),
                "Validate caller permissions before state changes".to_string(),
                "Use Address::require_auth() for individual user operations".to_string(),
            ],
        );

        self.best_practices.insert(
            "storage".to_string(),
            vec![
                "Use instance storage for configuration and admin".to_string(),
                "Use persistent storage for user data and balances".to_string(),
                "Use temporary storage for caching".to_string(),
                "Use symbolic keys for readability".to_string(),
            ],
        );

        self.best_practices.insert(
            "error_handling".to_string(),
            vec![
                "Use Result<T, E> for fallible operations".to_string(),
                "Define custom error types with #[contracterror]".to_string(),
                "Return errors instead of panicking".to_string(),
                "Validate inputs early in functions".to_string(),
            ],
        );

        self.best_practices.insert(
            "events".to_string(),
            vec![
                "Emit events for all state-changing operations".to_string(),
                "Include relevant parameters in events".to_string(),
                "Use symbolic topics for event filtering".to_string(),
            ],
        );

        self.best_practices.insert(
            "token".to_string(),
            vec![
                "Follow SEP-41 standard for fungible tokens".to_string(),
                "Implement transfer, transfer_from, balance, and allowance".to_string(),
                "Use i128 for token amounts".to_string(),
                "Validate amounts are positive".to_string(),
            ],
        );
    }

    /// Analyze contract source code and detect its type
    pub fn detect_contract_type(source_code: &str) -> ContractType {
        let lower = source_code.to_lowercase();

        // Check for token patterns
        if lower.contains("sep-41")
            || lower.contains("fungible")
            || (lower.contains("transfer")
                && lower.contains("balance")
                && lower.contains("allowance"))
        {
            return ContractType::Token;
        }

        // Check for NFT patterns
        if lower.contains("nft")
            || lower.contains("non-fungible")
            || (lower.contains("mint") && lower.contains("owner_of") && lower.contains("token_id"))
        {
            return ContractType::Nft;
        }

        // Check for governance patterns
        if lower.contains("proposal")
            || lower.contains("voting")
            || lower.contains("governance")
            || (lower.contains("propose") && lower.contains("vote") && lower.contains("execute"))
        {
            return ContractType::Governance;
        }

        // Check for DeFi patterns
        if lower.contains("swap")
            || lower.contains("liquidity")
            || lower.contains("pool")
            || lower.contains("amm")
            || lower.contains("lending")
        {
            return ContractType::Defi;
        }

        // Check for access control patterns
        if lower.contains("role")
            || lower.contains("permission")
            || (lower.contains("grant") && lower.contains("revoke"))
        {
            return ContractType::AccessControl;
        }

        ContractType::Generic
    }

    /// Analyze existing contract code and build context
    pub fn analyze_context(source_code: &str, contract_name: &str) -> ContractContext {
        let contract_type = Self::detect_contract_type(source_code);
        let existing_functions = Self::extract_function_names(source_code);
        let storage_keys = Self::extract_storage_keys(source_code);
        let events = Self::extract_events(source_code);
        let errors = Self::extract_error_variants(source_code);

        ContractContext {
            contract_type,
            existing_functions: existing_functions.clone(),
            contract_name: contract_name.to_string(),
            storage_keys,
            events,
            errors,
            has_initialize: existing_functions.iter().any(|f| f == "initialize"),
            has_admin: existing_functions.iter().any(|f| f.contains("admin")),
            has_token: existing_functions
                .iter()
                .any(|f| matches!(f.as_str(), "transfer" | "balance" | "allowance" | "approve")),
        }
    }

    /// Extract function names from source code
    fn extract_function_names(source_code: &str) -> Vec<String> {
        let mut functions = Vec::new();
        for line in source_code.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("pub fn ") || trimmed.starts_with("fn ") {
                if let Some(name_end) = trimmed.find('(') {
                    let after_fn = &trimmed[..name_end];
                    if let Some(fn_pos) = after_fn.rfind("fn ") {
                        let name = after_fn[fn_pos + 3..].trim();
                        if !name.is_empty() {
                            functions.push(name.to_string());
                        }
                    }
                }
            }
        }
        functions
    }

    /// Extract storage keys from source code
    fn extract_storage_keys(source_code: &str) -> Vec<String> {
        let mut keys = Vec::new();
        for line in source_code.lines() {
            let trimmed = line.trim();
            if trimmed.contains("symbol_short!") {
                if let Some(start) = trimmed.find("symbol_short!(\"") {
                    let key_start = start + 15;
                    if let Some(end) = trimmed[key_start..].find("\")") {
                        let key = &trimmed[key_start..key_start + end];
                        if !keys.contains(&key.to_string()) {
                            keys.push(key.to_string());
                        }
                    }
                }
            }
        }
        keys
    }

    /// Extract event names from source code
    fn extract_events(source_code: &str) -> Vec<String> {
        let mut events = Vec::new();
        for line in source_code.lines() {
            let trimmed = line.trim();
            if trimmed.contains("env.events().publish") {
                if let Some(start) = trimmed.find("symbol_short!(\"") {
                    let key_start = start + 15;
                    if let Some(end) = trimmed[key_start..].find("\")") {
                        let event = &trimmed[key_start..key_start + end];
                        if !events.contains(&event.to_string()) {
                            events.push(event.to_string());
                        }
                    }
                }
            }
        }
        events
    }

    /// Extract error variants from source code
    fn extract_error_variants(source_code: &str) -> Vec<String> {
        let mut errors = Vec::new();
        let mut in_error_enum = false;

        for line in source_code.lines() {
            let trimmed = line.trim();
            if trimmed.contains("#[contracterror]") {
                in_error_enum = true;
                continue;
            }
            if in_error_enum && trimmed.starts_with("pub enum") {
                continue;
            }
            if in_error_enum && (trimmed.starts_with('}') || trimmed == "}") {
                in_error_enum = false;
                continue;
            }
            if in_error_enum && trimmed.contains('=') {
                if let Some(name) = trimmed.split('=').next() {
                    let name = name.trim().to_string();
                    if !name.is_empty() && !errors.contains(&name) {
                        errors.push(name);
                    }
                }
            }
        }
        errors
    }

    /// Generate suggestions based on contract context
    pub fn suggest(&self, context: &ContractContext) -> Vec<FunctionSuggestion> {
        let mut suggestions = Vec::new();

        // Get templates for the contract type
        if let Some(templates) = self.templates.get(&context.contract_type) {
            for template in templates {
                // Only suggest functions that don't already exist
                if !context.existing_functions.contains(&template.name) {
                    suggestions.push(template.clone());
                }
            }
        }

        // Add generic suggestions if not present
        if let Some(generic_templates) = self.templates.get(&ContractType::Generic) {
            for template in generic_templates {
                if !context.existing_functions.contains(&template.name)
                    && !suggestions.iter().any(|s| s.name == template.name)
                {
                    suggestions.push(template.clone());
                }
            }
        }

        // Add context-specific suggestions
        suggestions.extend(self.generate_context_suggestions(context));

        // Sort by priority
        suggestions.sort_by(|a, b| b.priority.cmp(&a.priority));

        suggestions
    }

    /// Generate suggestions based on specific context
    fn generate_context_suggestions(&self, context: &ContractContext) -> Vec<FunctionSuggestion> {
        let mut suggestions = Vec::new();

        // If no initialize function, suggest one
        if !context.has_initialize {
            suggestions.push(FunctionSuggestion {
                name: "initialize".to_string(),
                category: SuggestionCategory::Initialization,
                priority: SuggestionPriority::Critical,
                description: "Initialize the contract".to_string(),
                signature: "pub fn initialize(env: Env, admin: Address)".to_string(),
                implementation: r#"{
    admin.require_auth();
    env.storage().instance().set(&symbol_short!("ADMIN"), &admin);
}"#
                .to_string(),
                imports: vec!["soroban_sdk::{symbol_short, Address, Env}".to_string()],
                best_practices: vec![
                    "Use require_auth() for admin".to_string(),
                    "Store admin in instance storage".to_string(),
                ],
                confidence: 95,
                context: "Contract initialization".to_string(),
            });
        }

        // If no admin function, suggest get_admin
        if !context.has_admin {
            suggestions.push(FunctionSuggestion {
                name: "get_admin".to_string(),
                category: SuggestionCategory::AccessControl,
                priority: SuggestionPriority::Medium,
                description: "Get the current admin address".to_string(),
                signature: "pub fn get_admin(env: Env) -> Address".to_string(),
                implementation:
                    "{\n    env.storage().instance().get(&symbol_short!(\"ADMIN\")).unwrap()\n}"
                        .to_string(),
                imports: vec!["soroban_sdk::{symbol_short, Address, Env}".to_string()],
                best_practices: vec!["Provide admin getter for transparency".to_string()],
                confidence: 80,
                context: "Admin getter function".to_string(),
            });
        }

        // Suggest error handling if no errors defined
        if context.errors.is_empty() {
            suggestions.push(FunctionSuggestion {
                name: "Error enum".to_string(),
                category: SuggestionCategory::ErrorHandling,
                priority: SuggestionPriority::High,
                description: "Define custom error types for the contract".to_string(),
                signature: "#[contracterror]\n#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]\n#[repr(u32)]\npub enum Error {\n    NotInitialized = 1,\n    AlreadyInitialized = 2,\n    Unauthorized = 3,\n    InsufficientBalance = 4,\n    InvalidAmount = 5,\n}".to_string(),
                implementation: String::new(),
                imports: vec![],
                best_practices: vec![
                    "Use Result<T, Error> for fallible operations".to_string(),
                    "Define meaningful error variants".to_string(),
                    "Use #[repr(u32)] for error codes".to_string(),
                ],
                confidence: 85,
                context: "Error type definition".to_string(),
            });
        }

        // Suggest events if none defined
        if context.events.is_empty() {
            suggestions.push(FunctionSuggestion {
                name: "Transfer event".to_string(),
                category: SuggestionCategory::Events,
                priority: SuggestionPriority::Medium,
                description: "Emit a transfer event when tokens are transferred".to_string(),
                signature: "let topics = (symbol_short!(\"transfer\"), from.clone(), to.clone());\nenv.events().publish(topics, amount);".to_string(),
                implementation: String::new(),
                imports: vec!["soroban_sdk::symbol_short".to_string()],
                best_practices: vec![
                    "Emit events for all state-changing operations".to_string(),
                    "Include sender, receiver, and amount in transfer events".to_string(),
                ],
                confidence: 75,
                context: "Transfer event emission".to_string(),
            });
        }

        suggestions
    }

    /// Get best practices for a specific category
    pub fn get_best_practices(&self, category: &str) -> Vec<String> {
        self.best_practices
            .get(category)
            .cloned()
            .unwrap_or_default()
    }

    /// Get all available categories with best practices
    pub fn list_best_practice_categories(&self) -> Vec<String> {
        self.best_practices.keys().cloned().collect()
    }

    /// Generate a complete contract scaffold based on contract type
    pub fn generate_scaffold(&self, contract_type: &ContractType, name: &str) -> String {
        match contract_type {
            ContractType::Token => format!(
                r#"#![no_std]

use soroban_sdk::{{contract, contractimpl, symbol_short, Address, Env, String}};

const ADMIN: Symbol = symbol_short!("ADMIN");
const NAME: Symbol = symbol_short!("NAME");
const SYMBOL: Symbol = symbol_short!("SYMBOL");
const DECIMALS: Symbol = symbol_short!("DECIMALS");

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {{
    NotInitialized = 1,
    AlreadyInitialized = 2,
    Unauthorized = 3,
    InsufficientBalance = 4,
    InvalidAmount = 5,
}}

#[contract]
pub struct {name};

#[contractimpl]
impl {name} {{
    /// Initialize the token contract
    pub fn initialize(env: Env, admin: Address, name: String, symbol: String, decimals: u32) {{
        admin.require_auth();
        env.storage().instance().set(&ADMIN, &admin);
        env.storage().instance().set(&NAME, &name);
        env.storage().instance().set(&SYMBOL, &symbol);
        env.storage().instance().set(&DECIMALS, &decimals);
    }}

    /// Get the token name
    pub fn name(env: Env) -> String {{
        env.storage().instance().get(&NAME).unwrap()
    }}

    /// Get the token symbol
    pub fn symbol(env: Env) -> String {{
        env.storage().instance().get(&SYMBOL).unwrap()
    }}

    /// Get the token decimals
    pub fn decimals(env: Env) -> u32 {{
        env.storage().instance().get(&DECIMALS).unwrap()
    }}

    /// Get the balance of an address
    pub fn balance(env: Env, id: Address) -> i128 {{
        env.storage().persistent().get(&id).unwrap_or(0)
    }}

    /// Transfer tokens from one address to another
    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) -> Result<(), Error> {{
        from.require_auth();

        if amount <= 0 {{
            return Err(Error::InvalidAmount);
        }}

        let from_balance = env.storage().persistent().get(&from).unwrap_or(0);
        if from_balance < amount {{
            return Err(Error::InsufficientBalance);
        }}

        env.storage().persistent().set(&from, &(from_balance - amount));
        let to_balance = env.storage().persistent().get(&to).unwrap_or(0);
        env.storage().persistent().set(&to, &(to_balance + amount));

        // Emit transfer event
        let topics = (symbol_short!("transfer"), from.clone(), to.clone());
        env.events().publish(topics, amount);

        Ok(())
    }}

    /// Get the admin address
    pub fn get_admin(env: Env) -> Address {{
        env.storage().instance().get(&ADMIN).unwrap()
    }}
}}

#[cfg(test)]
mod tests {{
    use super::*;
    use soroban_sdk::testutils::Address as _;

    #[test]
    fn test_initialize() {{
        let env = Env::default();
        let contract_id = env.register_contract(None, {name});
        let client = {name}Client::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let name = String::from_str(&env, "Token");
        let symbol = String::from_str(&env, "TKN");

        client.initialize(&admin, &name, &symbol, &18);

        assert_eq!(client.name(), name);
        assert_eq!(client.symbol(), symbol);
        assert_eq!(client.decimals(), 18);
    }}

    #[test]
    fn test_transfer() {{
        let env = Env::default();
        let contract_id = env.register_contract(None, {name});
        let client = {name}Client::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let from = Address::generate(&env);
        let to = Address::generate(&env);

        client.initialize(&admin, &String::from_str(&env, "Token"), &String::from_str(&env, "TKN"), &18);

        // Note: In a real test, you'd need to mint tokens first
        // This is a simplified example
    }}
}}
"#,
                name = name
            ),
            ContractType::Nft => format!(
                r#"#![no_std]

use soroban_sdk::{{contract, contractimpl, symbol_short, Address, Env}};

const ADMIN: Symbol = symbol_short!("ADMIN");
const NAME: Symbol = symbol_short!("NAME");
const SYMBOL: Symbol = symbol_short!("SYMBOL");
const NEXT_ID: Symbol = symbol_short!("NEXT_ID");

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {{
    NotInitialized = 1,
    AlreadyInitialized = 2,
    Unauthorized = 3,
    NotFound = 4,
}}

#[contract]
pub struct {name};

#[contractimpl]
impl {name} {{
    /// Initialize the NFT contract
    pub fn initialize(env: Env, admin: Address, name: String, symbol: String) {{
        admin.require_auth();
        env.storage().instance().set(&ADMIN, &admin);
        env.storage().instance().set(&NAME, &name);
        env.storage().instance().set(&SYMBOL, &symbol);
        env.storage().instance().set(&NEXT_ID, &0u64);
    }}

    /// Mint a new NFT
    pub fn mint(env: Env, to: Address) -> u64 {{
        let admin: Address = env.storage().instance().get(&ADMIN).unwrap();
        admin.require_auth();

        let next_id: u64 = env.storage().instance().get(&NEXT_ID).unwrap();
        env.storage().persistent().set(&next_id.to_string(), &to);
        env.storage().instance().set(&NEXT_ID, &(next_id + 1));

        // Emit mint event
        let topics = (symbol_short!("mint"), to.clone());
        env.events().publish(topics, next_id);

        next_id
    }}

    /// Get the owner of an NFT
    pub fn owner_of(env: Env, token_id: u64) -> Result<Address, Error> {{
        env.storage()
            .persistent()
            .get(&token_id.to_string())
            .ok_or(Error::NotFound)
    }}

    /// Get the total supply
    pub fn total_supply(env: Env) -> u64 {{
        env.storage().instance().get(&NEXT_ID).unwrap_or(0)
    }}
}}

#[cfg(test)]
mod tests {{
    use super::*;
    use soroban_sdk::testutils::Address as _;

    #[test]
    fn test_initialize_and_mint() {{
        let env = Env::default();
        let contract_id = env.register_contract(None, {name});
        let client = {name}Client::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let to = Address::generate(&env);

        client.initialize(&admin, &String::from_str(&env, "NFT"), &String::from_str(&env, "NFT"));

        let token_id = client.mint(&to);
        assert_eq!(token_id, 0);
        assert_eq!(client.owner_of(&token_id), Ok(to));
        assert_eq!(client.total_supply(), 1);
    }}
}}
"#,
                name = name
            ),
            _ => format!(
                r#"#![no_std]

use soroban_sdk::{{contract, contractimpl, symbol_short, Address, Env}};

const ADMIN: Symbol = symbol_short!("ADMIN");

#[contract]
pub struct {name};

#[contractimpl]
impl {name} {{
    /// Initialize the contract
    pub fn initialize(env: Env, admin: Address) {{
        admin.require_auth();
        env.storage().instance().set(&ADMIN, &admin);
    }}

    /// Get the admin address
    pub fn get_admin(env: Env) -> Address {{
        env.storage().instance().get(&ADMIN).unwrap()
    }}
}}

#[cfg(test)]
mod tests {{
    use super::*;
    use soroban_sdk::testutils::Address as _;

    #[test]
    fn test_initialize() {{
        let env = Env::default();
        let contract_id = env.register_contract(None, {name});
        let client = {name}Client::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.initialize(&admin);

        assert_eq!(client.get_admin(), admin);
    }}
}}
"#,
                name = name
            ),
        }
    }
}

impl Default for ContractSuggestionEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_contract_type_token() {
        let source = r#"
            pub fn transfer(from: Address, to: Address, amount: i128) {}
            pub fn balance(id: Address) -> i128 { 0 }
            pub fn allowance(spender: Address) -> i128 { 0 }
        "#;
        assert_eq!(
            ContractSuggestionEngine::detect_contract_type(source),
            ContractType::Token
        );
    }

    #[test]
    fn test_detect_contract_type_nft() {
        let source = r#"
            pub fn mint(to: Address) -> u64 { 0 }
            pub fn owner_of(token_id: u64) -> Address { todo!() }
        "#;
        assert_eq!(
            ContractSuggestionEngine::detect_contract_type(source),
            ContractType::Nft
        );
    }

    #[test]
    fn test_detect_contract_type_generic() {
        let source = r#"
            pub fn do_something() {}
        "#;
        assert_eq!(
            ContractSuggestionEngine::detect_contract_type(source),
            ContractType::Generic
        );
    }

    #[test]
    fn test_analyze_context() {
        let source = r#"
            pub fn initialize(env: Env, admin: Address) {
                admin.require_auth();
                env.storage().instance().set(&symbol_short!("ADMIN"), &admin);
            }
            pub fn get_admin(env: Env) -> Address {
                env.storage().instance().get(&symbol_short!("ADMIN")).unwrap()
            }
        "#;
        let context = ContractSuggestionEngine::analyze_context(source, "MyContract");
        assert!(context.has_initialize);
        assert!(context.has_admin);
        assert_eq!(context.existing_functions.len(), 2);
    }

    #[test]
    fn test_suggest_missing_initialize() {
        let engine = ContractSuggestionEngine::new();
        let context = ContractContext {
            contract_type: ContractType::Generic,
            existing_functions: vec![],
            contract_name: "Test".to_string(),
            storage_keys: vec![],
            events: vec![],
            errors: vec![],
            has_initialize: false,
            has_admin: false,
            has_token: false,
        };
        let suggestions = engine.suggest(&context);
        assert!(suggestions.iter().any(|s| s.name == "initialize"));
    }

    #[test]
    fn test_get_best_practices() {
        let engine = ContractSuggestionEngine::new();
        let practices = engine.get_best_practices("authorization");
        assert!(!practices.is_empty());
    }

    #[test]
    fn test_generate_scaffold_token() {
        let engine = ContractSuggestionEngine::new();
        let scaffold = engine.generate_scaffold(&ContractType::Token, "MyToken");
        assert!(scaffold.contains("pub struct MyToken;"));
        assert!(scaffold.contains("#[contractimpl]"));
        assert!(scaffold.contains("pub fn initialize"));
        assert!(scaffold.contains("pub fn transfer"));
        assert!(scaffold.contains("pub fn balance"));
    }
}

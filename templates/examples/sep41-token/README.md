# SEP-41 Token

A SEP-41 compliant fungible token contract for Soroban.

## Functions

| Function | Description |
|----------|-------------|
| `initialize(admin, decimals, name, symbol)` | Set up the token (once only) |
| `mint(to, amount)` | Mint tokens to an address — admin only |
| `transfer(from, to, amount)` | Move tokens between accounts |
| `balance(addr)` | Query token balance |
| `approve(from, spender, amount)` | Authorise a spender |
| `allowance(from, spender)` | Query remaining allowance |
| `transfer_from(spender, from, to, amount)` | Spend an allowance |
| `burn(from, amount)` | Destroy tokens |

## Usage

```bash
# Scaffold a new project from this template
starforge new contract my-token --template sep41-token

# Build
cargo build --target wasm32-unknown-unknown --release

# Test
cargo test
```

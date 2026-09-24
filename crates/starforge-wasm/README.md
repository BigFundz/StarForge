# starforge-wasm

WebAssembly API surface for StarForge — run core Stellar wallet operations
directly in the browser (web-based IDEs, playgrounds, dev environments) without
the native CLI.

The crate is self-contained and depends only on pure-Rust crypto primitives, so
it compiles cleanly to `wasm32-unknown-unknown` while the native CLI keeps its
full feature set.

## Build

```bash
# install once
cargo install wasm-pack

# build the browser bundle
wasm-pack build crates/starforge-wasm --target web
```

The generated `pkg/` directory contains the `.wasm` module and JS bindings.

## Usage

```js
import init, {
  generateKeypair,
  generateMnemonic,
  keypairFromMnemonic,
  validateAddress,
  configSet,
  configGet,
} from "./pkg/starforge_wasm.js";

await init();

// Browser-based wallet management
const kp = generateKeypair();
console.log(kp.publicKey, kp.secretKey);

const phrase = generateMnemonic(12);
const derived = keypairFromMnemonic(phrase, "", 0);

validateAddress(kp.publicKey); // true

// Browser storage for configuration (localStorage)
configSet("network", "testnet");
configGet("network"); // "testnet"
```

## API

| Function | Description |
| --- | --- |
| `version()` | Library version string. |
| `generateKeypair()` | Random Stellar ed25519 keypair (`{ publicKey, secretKey }`). |
| `generateMnemonic(wordCount)` | BIP39 English phrase (`12` or `24` words). |
| `keypairFromMnemonic(phrase, passphrase, accountIndex)` | SEP-0005 derivation (`m/44'/148'/account'`). |
| `validateAddress(address)` | Validate a Stellar `G...` public key. |
| `configSet(key, value)` / `configGet(key)` / `configRemove(key)` | Browser-backed config storage. |

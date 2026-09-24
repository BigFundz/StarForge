# Requirements Document

## Introduction

The Contract State Explorer extends StarForge's existing `inspect` command infrastructure with an interactive, persistent, and debuggable contract state experience for Soroban developers. Rather than one-shot queries, the explorer allows developers to take named state snapshots, compare snapshots across ledgers, search storage keys interactively, and export/import state data — all from the terminal CLI. It builds on the existing `soroban::inspect_contract()` RPC client and the `rusqlite`-backed `Database` layer that already powers wallets, networks, and config persistence.

## Glossary

- **Explorer**: The `starforge explore` CLI subcommand and its sub-commands that power the contract state explorer feature.
- **Snapshot**: A point-in-time capture of a contract's `ContractInspectResult` (executable, WASM hash, ledger metadata, and all instance storage entries) stored persistently in the SQLite database.
- **Snapshot_Store**: The SQLite-backed persistence layer responsible for saving, loading, listing, and deleting snapshots.
- **StateEntry**: A single key-value pair from contract instance storage (`ContractStorageEntry` with `key: String` and `value: String`).
- **Diff**: The computed difference between two Snapshots for the same contract, categorised as added keys, removed keys, or changed values.
- **Diff_Renderer**: The CLI component that formats and prints a Diff to the terminal using colour-coded output.
- **Key_Search**: A case-insensitive substring filter applied over all `StateEntry` keys within a Snapshot.
- **Export**: A serialised JSON representation of one or more Snapshots written to a file or stdout.
- **Import**: The process of reading an Export file and inserting its Snapshots into the Snapshot_Store.
- **Interactive_Mode**: A `rustyline`-powered REPL loop entered via `starforge explore interactive <contract_id>` that accepts sub-commands without re-specifying the contract ID.
- **Network**: A named Soroban RPC endpoint already configured in StarForge (e.g., `testnet`, `mainnet`).
- **Contract_ID**: A Stellar contract strkey (56-character string starting with `C`) identifying a deployed Soroban contract.

---

## Requirements

### Requirement 1: State Snapshot Capture

**User Story:** As a Soroban developer, I want to capture and persist the full state of a contract at the current ledger, so that I can track how storage evolves over time.

#### Acceptance Criteria

1. WHEN the developer runs `starforge explore snapshot <contract_id> --network <network>`, THE Explorer SHALL validate that `<contract_id>` is a 56-character string starting with `C`, then call `soroban::inspect_contract()` to fetch the current `ContractInspectResult` and persist it as a new Snapshot in the Snapshot_Store with a UTC timestamp and the current `latest_ledger` value.
2. IF `<contract_id>` does not pass the format validation described in criterion 1, THEN THE Explorer SHALL print an error message stating the contract ID is invalid and return a non-zero exit code without calling `soroban::inspect_contract()`.
3. WHEN a snapshot is saved successfully, THE Explorer SHALL print the assigned snapshot ID, the contract ID, the ledger number, and the count of instance storage entries in the snapshot to the terminal.
4. WHEN a snapshot is saved successfully, THE Explorer SHALL return exit code 0.
5. IF `soroban::inspect_contract()` returns an error, THEN THE Explorer SHALL print the error message returned by the RPC layer and return a non-zero exit code without writing any rows to the Snapshot_Store.
6. IF the database write fails after a successful RPC call, THEN THE Explorer SHALL print a descriptive error message including the database error and return a non-zero exit code; no partial snapshot rows SHALL remain in the Snapshot_Store after the failure.
7. THE Explorer SHALL accept an optional `--label <text>` flag on the `snapshot` sub-command; WHEN `--label` is provided with a non-empty string of at most 255 characters, THE Snapshot_Store SHALL store the label alongside the snapshot record.
8. IF `--label` is provided with an empty string or a string exceeding 255 characters, THEN THE Explorer SHALL print a validation error and return a non-zero exit code without creating the snapshot.
9. THE Snapshot_Store SHALL assign each Snapshot a unique ID using UUID v4.
10. WHERE the `contract_snapshots` table does not yet exist in the StarForge SQLite database, THE Snapshot_Store SHALL create it before executing any write for that snapshot.

---

### Requirement 2: Historical State Viewing

**User Story:** As a Soroban developer, I want to list and view previously captured snapshots for a contract, so that I can inspect how storage looked at earlier ledger heights.

#### Acceptance Criteria

1. WHEN the developer runs `starforge explore list <contract_id>`, THE Explorer SHALL query the Snapshot_Store and display a paginated list of all snapshots for that contract, ordered by ledger number descending, showing snapshot ID, label, ledger number, entry count, and timestamp.
2. WHEN the developer runs `starforge explore show <snapshot_id>`, THE Explorer SHALL retrieve the snapshot from the Snapshot_Store and render all its `StateEntry` records in a table with the columns: key, entry type, value, and ledger number.
3. WHEN no snapshots exist for the given contract, THE Explorer SHALL display an informational message stating that no snapshots have been taken yet, and SHALL return exit code 0.
4. THE Explorer SHALL support a `--limit <n>` flag on `list` (default 20, maximum 200) accepting a non-negative integer offset, and a `--cursor <offset>` flag for pagination; WHEN results exceed the limit, THE Explorer SHALL display the next-page cursor value as a trailing line in the output.
5. WHEN the developer passes `--json` to `starforge explore show`, THE Explorer SHALL serialise the snapshot to JSON, print to stdout, write nothing to stderr, and return exit code 0.
6. IF a `show` command references a snapshot ID that does not exist in the Snapshot_Store, THEN THE Explorer SHALL print an error message including the unknown ID and return a non-zero exit code.

---

### Requirement 3: State Diff Visualisation

**User Story:** As a Soroban developer, I want to compare two snapshots of the same contract side-by-side, so that I can understand exactly which storage keys changed between two ledger heights.

#### Acceptance Criteria

1. WHEN the developer runs `starforge explore diff <snapshot_id_a> <snapshot_id_b>`, THE Explorer SHALL compute the Diff between the two Snapshots and render it via the Diff_Renderer, then return exit code 0.
2. THE Diff_Renderer SHALL display added keys in green, removed keys in red, and changed values in yellow, using the `colored` crate.
3. THE Diff_Renderer SHALL label each changed entry with `[+] Added`, `[-] Removed`, or `[~] Changed`, followed by the key name and, for changed entries, both the old and new values on separate lines.
4. WHEN the two snapshots have identical storage contents, THE Explorer SHALL print a message stating no differences were found and return exit code 0.
5. WHEN the developer passes `--json` to `starforge explore diff`, THE Explorer SHALL serialise the Diff as a JSON object with `added`, `removed`, and `changed` arrays and print to stdout. Each element of `added` and `removed` SHALL have `key` and `value` fields; each element of `changed` SHALL have `key`, `old_value`, and `new_value` fields.
6. IF either snapshot ID provided to `diff` does not exist in the Snapshot_Store, THEN THE Explorer SHALL print a descriptive error identifying which ID was not found and return a non-zero exit code.
7. IF the two snapshots belong to different contract IDs, THEN THE Explorer SHALL print an error stating that cross-contract diffs are not supported and return a non-zero exit code without computing any diff.

---

### Requirement 4: Storage Key Search

**User Story:** As a Soroban developer, I want to search for specific storage keys across a contract's snapshots, so that I can quickly locate and track a particular entry without scrolling through all stored data.

#### Acceptance Criteria

1. WHEN the developer runs `starforge explore search <contract_id> --key <pattern>` and `<pattern>` is a non-empty string, THE Explorer SHALL perform a case-insensitive substring match against all `StateEntry` keys within the most recent snapshot for that contract.
2. WHEN matching entries are found, THE Explorer SHALL display them in a table with the columns: key, value, snapshot ID, and ledger number, consistent with the format used by `starforge inspect storage`.
3. WHEN the developer additionally passes `--all-snapshots`, THE Explorer SHALL perform the Key_Search across every snapshot for the contract and group results by snapshot ID and ledger number.
4. IF both `--snapshot <snapshot_id>` and `--all-snapshots` are provided in the same invocation, THEN THE Explorer SHALL print an error stating these flags are mutually exclusive and return a non-zero exit code.
5. WHEN no matching keys are found, THE Explorer SHALL display an informational message and return exit code 0.
6. THE Explorer SHALL accept a `--snapshot <snapshot_id>` flag to restrict the search to a specific snapshot rather than the most recent one.
7. IF `starforge explore search` is run for a contract that has no snapshots at all in the Snapshot_Store, THEN THE Explorer SHALL print an informational message stating no snapshots exist for the contract and return exit code 0.
8. IF `--key` is provided with an empty string, THEN THE Explorer SHALL print a validation error and return a non-zero exit code.

---

### Requirement 5: State Export and Import

**User Story:** As a Soroban developer, I want to export contract snapshots to a portable file and re-import them later, so that I can share state data with colleagues or restore historical state in a different environment.

#### Acceptance Criteria

1. WHEN the developer runs `starforge explore export <contract_id> --out <file_path>`, THE Explorer SHALL serialise all snapshots for that contract from the Snapshot_Store to a JSON file at the specified path. Each snapshot record in the file SHALL include: snapshot ID, contract ID, label, ledger number, UTC timestamp, and a list of StateEntry key-value pairs.
2. WHEN the developer passes `--snapshot <snapshot_id>` to `export`, THE Explorer SHALL serialise only that single snapshot to the output file; IF `<snapshot_id>` does not exist in the Snapshot_Store, THEN THE Explorer SHALL print an error and return a non-zero exit code without creating any file.
3. WHEN an export file is written successfully, THE Explorer SHALL print the number of snapshots exported and the output file path, and return exit code 0.
4. IF the output file path is not writable, THEN THE Explorer SHALL print a descriptive error and return a non-zero exit code without creating a partial file.
5. WHEN the developer runs `starforge explore import <file_path>`, THE Explorer SHALL parse the JSON export file and insert each snapshot into the Snapshot_Store, skipping any snapshot whose ID already exists.
6. WHEN import completes, THE Explorer SHALL print the number of snapshots imported and the number skipped as duplicates.
7. IF the import file cannot be read, is not valid JSON, or does not contain the required fields (snapshot ID, contract ID, ledger number, UTC timestamp, and StateEntry list), THEN THE Explorer SHALL print a descriptive parse or I/O error and return a non-zero exit code without inserting any partial data.
8. WHEN a valid export file produced by `starforge explore export` is subsequently passed to `starforge explore import`, THE Snapshot_Store SHALL contain snapshot records with matching snapshot ID, contract ID, label, ledger number, UTC timestamp, and identical StateEntry key-value pairs for every snapshot in the file.

---

### Requirement 6: Interactive Exploration Mode

**User Story:** As a Soroban developer, I want an interactive REPL for a contract so that I can iteratively inspect, diff, and search without repeating the contract ID and network flags on every command.

#### Acceptance Criteria

1. WHEN the developer runs `starforge explore interactive <contract_id> --network <network>`, THE Explorer SHALL enter Interactive_Mode, displaying a prompt of the form `explore[<first_8_chars_of_contract_id>]> `.
2. WHILE in Interactive_Mode, THE Explorer SHALL accept the sub-commands `snapshot`, `list`, `show <id>`, `diff <id_a> <id_b>`, `search <pattern>`, `export --out <path>`, and `help` without requiring the contract ID or network to be re-specified.
3. WHILE in Interactive_Mode, THE Explorer SHALL print command output to the terminal after each command and return to the prompt.
4. WHEN a sub-command executed in Interactive_Mode returns an error, THE Explorer SHALL print the error message and return to the prompt without exiting Interactive_Mode.
5. WHEN the developer enters `exit` or `quit` in Interactive_Mode, or presses Ctrl-C, THE Explorer SHALL flush all pending output and exit the REPL cleanly, returning exit code 0.
6. IF an unrecognised command is entered in Interactive_Mode, THEN THE Explorer SHALL print a usage hint listing available commands and return to the prompt without exiting.
7. THE Explorer SHALL use `rustyline` for Interactive_Mode input to provide in-memory readline history (up to 500 entries per session) and basic line editing.

---

### Requirement 7: Snapshot Store Persistence and Schema

**User Story:** As a Soroban developer, I want snapshot data to be reliably persisted in the existing StarForge database so that snapshots survive process restarts and integrate with other database operations.

#### Acceptance Criteria

1. WHERE the `contract_snapshots` table does not exist in the StarForge SQLite database, THE Snapshot_Store SHALL create it using `CREATE TABLE IF NOT EXISTS` without dropping or modifying any other existing tables.
2. WHERE the `contract_snapshot_entries` table does not exist, THE Snapshot_Store SHALL create it using `CREATE TABLE IF NOT EXISTS`, with a foreign key referencing `contract_snapshots(id)`.
3. THE Snapshot_Store SHALL execute `PRAGMA foreign_keys = ON` on every database connection before performing any read or write operation.
4. WHEN the database schema version exactly matches the current expected version, THE Snapshot_Store SHALL complete initialisation without error and without executing any DDL statements.
5. THE Snapshot_Store SHALL expose the following typed Rust methods, all returning `anyhow::Result`: `save_snapshot(snapshot: &Snapshot) -> Result<()>`, `load_snapshot(id: &str) -> Result<Snapshot>`, `list_snapshots(contract_id: &str, limit: usize, offset: usize) -> Result<Vec<SnapshotMeta>>`, `delete_snapshot(id: &str) -> Result<()>`, `list_entries(snapshot_id: &str) -> Result<Vec<StateEntry>>`, and `search_entries(snapshot_id: &str, pattern: &str) -> Result<Vec<StateEntry>>` where `pattern` is matched case-insensitively as a substring of the entry key.
6. IF `load_snapshot` is called with a snapshot ID that does not exist in the Snapshot_Store, THEN it SHALL return an `Err` containing a message that includes the unknown ID.
7. WHEN `delete_snapshot` is called with an existing snapshot ID, THE Snapshot_Store SHALL remove the snapshot record and all associated `contract_snapshot_entries` rows within a single atomic transaction.

---

### Requirement 8: Debugging Aids

**User Story:** As a Soroban developer, I want to view the raw XDR bytes and type information for storage values, so that I can debug serialisation issues and verify contract state at a low level.

#### Acceptance Criteria

1. WHEN the developer passes `--raw` to `starforge explore show` or `starforge explore search`, THE Explorer SHALL display the verbatim string stored in each `StateEntry.value` field without applying any `pretty_value()` formatting; IF `--json` is also passed, THEN `--json` takes precedence and `--raw` is ignored.
2. WHEN the developer runs `starforge explore debug <contract_id> --network <network>`, THE Explorer SHALL call `soroban::inspect_contract()`, display the full state table with each entry annotated by its inferred ScVal type label (`[String]`, `[Address]`, `[Bytes]`, `[Numeric]`, or `[Other]`) using the same heuristics as `pretty_value()`, and SHALL NOT persist the fetched state to the Snapshot_Store.
3. IF `soroban::inspect_contract()` returns an error during `starforge explore debug`, THEN THE Explorer SHALL print the error and return a non-zero exit code.
4. WHEN the developer passes `--verbose` to a sub-command that performs a live Soroban RPC call (i.e., `snapshot`, `debug`, or `interactive`), THE Explorer SHALL print the Soroban RPC URL being used and the raw ledger metadata (latest ledger, last modified ledger, live-until ledger) to stdout before the primary output; for sub-commands that only read from the Snapshot_Store, `--verbose` SHALL have no effect.
5. WHERE `live_until_ledger` is available for a contract entry AND `(live_until_ledger - latest_ledger) >= 0` AND `(live_until_ledger - latest_ledger) < 1000`, THE Explorer SHALL display an inline warning per affected entry stating that the contract entry is expiring soon.

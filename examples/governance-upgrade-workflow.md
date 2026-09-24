# Governance Upgrade Workflow Example

This example walks through a team-governed contract upgrade on testnet.

## Prerequisites

- StarForge installed and a funded testnet wallet
- A compiled Soroban WASM file
- Contract already deployed on testnet

## Step 1 — Initialize governance config

```bash
starforge governance config set \
  --timelock 3600 \
  --threshold 2 \
  --emergency-quorum 1

starforge governance config set --guardian GYOUR_GUARDIAN_KEY...
starforge governance config show
```

## Step 2 — Propose an upgrade

```bash
starforge governance propose \
  --contract-id CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAD2KM \
  --wasm ./target/wasm32v1-none/release/my_contract.wasm \
  --description "Add pause functionality" \
  --network testnet \
  --wallet deployer
```

Note the returned proposal ID (e.g. `gov-a1b2c3d4e5f6`).

## Step 3 — Team voting

```bash
# Alice votes in favor
starforge governance vote \
  --proposal-id gov-a1b2c3d4e5f6 \
  --for \
  --wallet alice \
  --network testnet

# Bob casts the deciding vote
starforge governance vote \
  --proposal-id gov-a1b2c3d4e5f6 \
  --for \
  --wallet bob \
  --network testnet
```

After Bob's vote the proposal status becomes `passed` and the 1-hour timelock starts.

## Step 4 — Dashboard check

```bash
starforge governance dashboard --network testnet
starforge governance show --proposal-id gov-a1b2c3d4e5f6 --network testnet
```

## Step 5 — Execute after timelock

Wait for the timelock to expire, then:

```bash
starforge governance execute \
  --proposal-id gov-a1b2c3d4e5f6 \
  --wallet alice \
  --network testnet
```

Run the printed `stellar contract upload` and `stellar contract invoke` commands to apply the upgrade on-chain.

## Step 6 — Review audit trail

```bash
starforge governance audit --proposal-id gov-a1b2c3d4e5f6
```

Expected actions: `propose`, `vote`, `threshold_reached`, `execute`.

## Emergency scenario

If a critical vulnerability is discovered before the timelock expires:

```bash
starforge governance emergency \
  --contract-id CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAD2KM \
  --wasm ./target/wasm32v1-none/release/hotfix.wasm \
  --description "Patch critical overflow" \
  --wallet guardian \
  --network testnet \
  --yes
```

The emergency path skips the timelock when the guardian quorum is satisfied.

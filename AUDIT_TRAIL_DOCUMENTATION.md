# Audit Trail Documentation

This document explains how audit trails work in StarForge and how to use them for security review, compliance, and incident investigation.

## Table of Contents

1. [Audit Trail Overview](#audit-trail-overview)
2. [Sensitive Operations Audit Log](#sensitive-operations-audit-log)
3. [Log Format and Examples](#log-format-and-examples)
4. [Reviewing Audit Logs](#reviewing-audit-logs)
5. [Log Security and Retention](#log-security-and-retention)
6. [Integration with Monitoring](#integration-with-monitoring)
7. [Incident Investigation](#incident-investigation)

---

## Audit Trail Overview

### What Audit Trails Track

An audit trail is a sequential record of security-relevant events. In StarForge, audit trails track:

| Category | Examples |
|----------|----------|
| **Wallet Operations** | Create, encrypt, fund, show, remove, rotate |
| **Plugin Management** | Load, execute, unload, version check |
| **Contract Deployment** | Validate, deploy, inspect, invoke |
| **Network Operations** | Switch network, test connectivity, add custom network |
| **Authentication** | Key operations, passphrase attempts, decryption |

### When Logs Are Created

Audit logs are created for:
- **Every successful security operation** - Operation, timestamp, status
- **Every failed security operation** - What failed and why (without secrets)
- **Every administrative action** - Who did what and when
- **Configuration changes** - Network settings, wallet management, plugin installation

### Log Retention

**Recommended retention policies:**

| Log Type | Recommended Retention | Reason |
|----------|---------------------|--------|
| Security operations | 90 days | Compliance and incident investigation |
| Deployment records | 1 year | Audit trail for contracts |
| Plugin operations | 90 days | Security and compatibility tracking |
| Network operations | 30 days | Troubleshooting and diagnostics |
| Authentication logs | 180 days | Account security review |

**Implementation:**

```bash
# Configure log rotation in starforge
starforge --log-dir ./logs --log-format json

# Implement OS-level rotation
# /etc/logrotate.d/starforge
/path/to/logs/*.log {
    daily
    rotate 90
    compress
    delaycompress
    missingok
    notifempty
}
```

---

## Sensitive Operations Audit Log

### Complete Operation Reference

#### Wallet Operations

| Operation | What Gets Logged | Sensitivity | Example |
|-----------|-----------------|-------------|---------|
| **wallet create** | name, network, encryption enabled, duration, status | Public | `{operation: "wallet_create", wallet: "alice", network: "testnet", encrypted: true, duration_ms: 150, status: "success"}` |
| **wallet encrypt** | name, algorithm, KDF iterations (size not value), duration, status | Public | `{operation: "wallet_encrypt", wallet: "alice", algorithm: "aes-256-gcm", kdf_iterations: 100000, duration_ms: 245, status: "success"}` |
| **wallet fund** | name, network, amount, balance after, transaction hash, status | Public | `{operation: "wallet_fund", wallet: "alice", network: "testnet", amount: 1000, final_balance: 5000, tx_hash: "abc...", status: "success"}` |
| **wallet show** | name, reveal_requested, balance_queried, status | Public | `{operation: "wallet_show", wallet: "alice", reveal_requested: false, balance_status: "queried", status: "success"}` |
| **wallet remove** | name, network, confirmation, status | Public | `{operation: "wallet_remove", wallet: "alice", confirmed: true, status: "success"}` |
| **wallet rotate** | name, new_account_created, network, status | Public | `{operation: "wallet_rotate", wallet: "alice", new_public_key: "GDRX...4T", network: "testnet", status: "success"}` |

**Never log for wallet operations:**
- Secret keys or private key material
- Passphrases or encryption keys
- Wallet backup data
- Full account balances (OK to log final balance, not intermediate states)

#### Plugin Operations

| Operation | What Gets Logged | Sensitivity | Example |
|-----------|-----------------|-------------|---------|
| **plugin load** | name, version, source path, compatibility check, trust level, status | Public | `{operation: "plugin_load", plugin: "defi", version: "1.0.0", source: "/path/to/lib.so", compatible: true, trust_level: "verified", status: "success"}` |
| **plugin execute** | name, command (args sanitized), duration, exit code, status | Public | `{operation: "plugin_execute", plugin: "defi", command: "swap", duration_ms: 500, exit_code: 0, status: "success"}` |
| **plugin unload** | name, cleanup_status, status | Public | `{operation: "plugin_unload", plugin: "defi", cleanup: "successful", status: "success"}` |
| **plugin compatibility-check** | name, version, starforge_version, compatible, status | Public | `{operation: "plugin_check", plugin: "defi", plugin_version: "1.0.0", starforge_version: "0.1.0", compatible: true, status: "success"}` |

**Never log for plugin operations:**
- Plugin source code or binaries
- Plugin configuration secrets
- Execution results containing user data
- Sensitive plugin output

#### Contract Operations

| Operation | What Gets Logged | Sensitivity | Example |
|-----------|-----------------|-------------|---------|
| **contract validate** | file path, file size, warnings/errors, status | Public | `{operation: "contract_validate", file: "contract.wasm", file_size: 262144, warnings: 0, errors: 0, status: "success"}` |
| **contract deploy** | network, account (redacted), wasm hash, gas used, address, status | Public | `{operation: "contract_deploy", network: "testnet", account: "GDRX...4T", wasm_hash: "abc123...", gas_used: 12345, contract_address: "CCPYZ...FNCI", status: "success"}` |
| **contract inspect** | address (redacted), network, metadata retrieved, status | Public | `{operation: "contract_inspect", address: "CCPYZ...FNCI", network: "testnet", has_metadata: true, status: "success"}` |
| **contract invoke** | address (redacted), network, function name, param count, result status | Public | `{operation: "contract_invoke", address: "CCPYZ...FNCI", network: "testnet", function: "transfer", param_count: 3, result: "success", status: "success"}` |

**Never log for contract operations:**
- Private contract state
- Function arguments with secrets
- Signed XDR transactions
- Private key material
- Full transaction payloads

#### Network Operations

| Operation | What Gets Logged | Sensitivity | Example |
|-----------|-----------------|-------------|---------|
| **network switch** | from_network, to_network, status | Public | `{operation: "network_switch", from: "testnet", to: "mainnet", status: "success"}` |
| **network test** | network, horizon_status, soroban_status, latency_ms, status | Public | `{operation: "network_test", network: "testnet", horizon: "ok", soroban: "ok", latency_ms: 145, status: "success"}` |
| **network add** | network name, endpoint count, verification status, status | Public | `{operation: "network_add", name: "custom-net", horizons: 2, sorobans: 2, verified: true, status: "success"}` |

**Never log for network operations:**
- API keys or credentials
- Internal network addresses
- Custom network secrets

---

## Log Format and Examples

### Standard Log Entry Structure

Each audit log entry contains:

```json
{
  "timestamp": "2024-01-15T10:30:45.123Z",
  "level": "INFO",
  "target": "starforge::commands::wallet",
  "fields": {
    "operation": "wallet_create",
    "wallet": "alice",
    "network": "testnet",
    "encrypted": true,
    "duration_ms": 150,
    "status": "success"
  },
  "message": "Wallet created"
}
```

### Human-Readable Format

```
2024-01-15T10:30:45.123Z [INFO] starforge::commands::wallet: Wallet created
    operation: wallet_create
    wallet: alice
    network: testnet
    encrypted: true
    duration_ms: 150
    status: success
```

### Real-World Examples

#### Successful Deployment Sequence

```json
[
  {
    "timestamp": "2024-01-15T10:30:00Z",
    "operation": "contract_validate",
    "file": "contract.wasm",
    "file_size": 262144,
    "status": "success"
  },
  {
    "timestamp": "2024-01-15T10:30:05Z",
    "operation": "contract_deploy",
    "network": "testnet",
    "account": "GDRX...4T",
    "wasm_hash": "5d41402abc4b2a76b9719d911017c592",
    "gas_used": 12345,
    "contract_address": "CCPYZ...FNCI",
    "status": "success"
  }
]
```

#### Failed Wallet Operation

```json
{
  "timestamp": "2024-01-15T10:35:20Z",
  "operation": "wallet_fund",
  "wallet": "alice",
  "network": "testnet",
  "amount": 1000,
  "error_type": "InsufficientFunds",
  "error_message": "Account has insufficient balance",
  "status": "failed"
}
```

#### Plugin Execution with Duration

```json
{
  "timestamp": "2024-01-15T10:40:00Z",
  "operation": "plugin_execute",
  "plugin": "defi",
  "command": "swap",
  "duration_ms": 523,
  "exit_code": 0,
  "status": "success"
}
```

---

## Reviewing Audit Logs

### Daily Review Checklist

- [ ] No unexpected failed operations
- [ ] Plugin executions completed successfully
- [ ] Network connectivity stable
- [ ] Deployment operations recorded
- [ ] Error counts within expected range

### Weekly Security Review

```bash
# Find all wallet operations
jq 'select(.fields.operation | startswith("wallet_"))' logs/*.log

# Find all failed operations
jq 'select(.fields.status == "failed")' logs/*.log

# Find operations taking longer than expected
jq 'select(.fields.duration_ms > 5000)' logs/*.log
```

### Monthly Compliance Report

```bash
# Count operations by type
jq -r '.fields.operation' logs/*.log | sort | uniq -c

# Find all deployments
jq 'select(.fields.operation == "contract_deploy")' logs/*.log

# Identify configuration changes
jq 'select(.fields.operation | startswith("network_"))' logs/*.log
```

### Debugging Common Issues

#### Wallet Encryption Failures

```bash
jq 'select(.fields.operation == "wallet_encrypt" and .fields.status == "failed")' logs/*.log | jq '.fields | {wallet, error_type, duration_ms}'
```

#### Deployment Issues

```bash
jq 'select(.fields.operation == "contract_deploy")' logs/*.log | jq '.fields | {network, account, status, gas_used, error_type}'
```

#### Plugin Compatibility Problems

```bash
jq 'select(.fields.operation | contains("plugin")) and select(.fields.status == "failed")' logs/*.log
```

---

## Log Security and Retention

### File Permissions

Logs contain sensitive operational details. Protect them:

```bash
# Set restrictive permissions
chmod 600 /path/to/logs/*.log

# Verify permissions
ls -l /path/to/logs/

# Allow only owner to read
# Disable world/group access
```

### Sensitive Data Redaction

Logs automatically redact sensitive information:

```
Public key redaction at INFO level:
GDRXMZDQW34QHX6F5U6FFWJZZZDQ4KYWJO65HS4CUT62X7Y7RXYWXE4T
→ GDRX...4T

Full key at DEBUG level:
GDRXMZDQW34QHX6F5U6FFWJZZZDQ4KYWJO65HS4CUT62X7Y7RXYWXE4T (unchanged)
```

### Log Retention Policy

```
Security operations log:  Keep for 90 days (quarterly compliance)
Deployment records:       Keep for 1 year (contract audit trail)
Error logs:              Keep for 180 days (incident investigation)
Archive older logs:      Compress and store offline after retention period
```

### Secure Log Transmission

If sending logs to external systems:

```bash
# Encrypt logs before transmission
gpg --encrypt --recipient key-id logs/*.log

# Use TLS for network transmission
# Verify server certificate
curl --cacert ca.crt https://log-collector.example.com

# Authenticate with credentials
curl -u user:pass https://log-collector.example.com
```

---

## Integration with Monitoring

### Real-Time Alerting

Set up alerts for:

```yaml
# Example alerting rules
alerts:
  - name: failed_wallet_operation
    query: 'operation=wallet_* AND status=failed'
    severity: warning
    action: notify_admin
    
  - name: deployment_failure
    query: 'operation=contract_deploy AND status=failed'
    severity: critical
    action: page_oncall
    
  - name: plugin_compatibility_error
    query: 'operation=plugin_* AND error_type=CompatibilityError'
    severity: warning
    action: notify_admin
```

### Log Aggregation

Tools for collecting and analyzing logs:

| Tool | Use Case |
|------|----------|
| **ELK Stack** | Elasticsearch + Logstash + Kibana for log analysis |
| **Splunk** | Enterprise log monitoring and analysis |
| **CloudWatch** | AWS native log aggregation |
| **Datadog** | Cloud monitoring with log aggregation |
| **grep + awk** | Simple local log analysis |

### Example ELK Integration

```bash
# Configure filebeat to ship logs
filebeat.inputs:
  - type: log
    enabled: true
    paths:
      - /var/log/starforge/*.log
    fields:
      app: starforge
      env: production

output.elasticsearch:
  hosts: ["localhost:9200"]
```

### Visualization Examples

Create dashboards showing:
- **Operations timeline** - All security operations chronologically
- **Error rate** - Percentage of failed operations by type
- **Performance metrics** - Average duration by operation type
- **Network status** - Connectivity and latency trends
- **Deployment history** - All contract deployments with outcomes

---

## Incident Investigation

### Step-by-Step Incident Investigation

1. **Identify the timeframe**
   ```bash
   # Find operations around incident time
   jq 'select(.timestamp >= "2024-01-15T10:00:00Z" and .timestamp <= "2024-01-15T11:00:00Z")' logs/*.log
   ```

2. **Find the affected resource**
   ```bash
   # Find operations for a specific wallet
   jq 'select(.fields.wallet == "alice")' logs/*.log
   ```

3. **Trace the operation sequence**
   ```bash
   # Show all operations for a contract deployment
   jq 'select(.fields.operation | contains("deploy")) and select(.fields.contract_address == "CCPYZ...FNCI")' logs/*.log | jq '.[] | {timestamp, operation, status, duration_ms}'
   ```

4. **Identify the failure point**
   ```bash
   # Find first failure
   jq 'select(.fields.status == "failed")' logs/*.log | head -1
   ```

5. **Gather error context**
   ```bash
   # Get full error details
   jq 'select(.fields.error_type != null)' logs/*.log | jq '.fields | {operation, error_type, error_message}'
   ```

6. **Review recovery actions**
   ```bash
   # Find retry attempts
   jq 'select(.fields.attempt > 1)' logs/*.log
   ```

### Common Incident Scenarios

#### Scenario: Wallet Creation Failed

```bash
# Find the failure
jq 'select(.fields.operation == "wallet_create" and .fields.status == "failed")' logs/*.log

# Check disk space
df -h

# Verify permissions
ls -l ~/.starforge/

# Check system resources
top -b -n 1 | head -20
```

#### Scenario: Deployment Error

```bash
# Find deployment failures
jq 'select(.fields.operation == "contract_deploy" and .fields.status == "failed")' logs/*.log

# Check network connectivity
jq 'select(.fields.operation == "network_test")' logs/*.log | tail -5

# Verify account balance
jq 'select(.fields.operation == "wallet_show" and .fields.wallet == "deployer")' logs/*.log

# Review gas estimates
jq 'select(.fields.operation == "contract_validate")' logs/*.log
```

#### Scenario: Plugin Crash

```bash
# Find plugin errors
jq 'select(.fields.operation | contains("plugin")) and select(.fields.status == "failed")' logs/*.log

# Check compatibility
jq 'select(.fields.operation == "plugin_check")' logs/*.log

# Review execution logs
jq 'select(.fields.operation == "plugin_execute")' logs/*.log | tail -10
```

---

## Further Reading

- [SECURITY_LOGGING_GUIDE.md](SECURITY_LOGGING_GUIDE.md) - How to log security operations
- [SECURITY_LOGGING_BEST_PRACTICES.md](SECURITY_LOGGING_BEST_PRACTICES.md) - Implementation patterns
- [src/utils/logging.rs](src/utils/logging.rs) - Logging infrastructure

---

*Last updated: 2026-06-01*  
*Issue #223: Improve security logging and auditability*

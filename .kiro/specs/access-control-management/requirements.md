# Requirements Document

## Introduction

The Access Control Management System provides a secure, auditable, and testable access
control ecosystem for Soroban smart contracts deployed on the Stellar network. It
extends the standard OpenZeppelin-inspired Role-Based Access Control (RBAC) model with
granular permission tracking, reusable enforcement guards, structured audit event
emission, a comprehensive Foundry-based test suite, and a React/TypeScript admin
dashboard that visualizes the live access matrix across contract addresses.

This feature integrates into the StarForge workspace as a Solidity-compatible smart
contract module (targeting Soroban/Stellar), a Foundry test suite, and a React frontend
component surfaced through the StarForge UI layer.

---

## Glossary

- **AccessController**: The primary smart contract that manages role assignments and
  enforces access policies.
- **Role**: A named, bytes32-encoded identifier that groups a set of permissions
  together (e.g., `ADMIN_ROLE`, `MINTER_ROLE`).
- **Permission**: A granular, addressable capability that can be attached to or detached
  from a Role.
- **RoleAdmin**: The Role whose holders are authorized to grant and revoke a given Role.
- **Principal**: An Ethereum/Soroban account address that may hold one or more Roles.
- **AccessMatrix**: A read-only, UI-level data structure mapping contract addresses to
  their active Roles and associated Permissions.
- **AccessGuard**: A reusable Solidity modifier or equivalent Soroban guard function
  that enforces a required Role at a contract entry point.
- **AuditLog**: The ordered stream of on-chain events emitted by the AccessController
  for all authorization actions.
- **TestSuite**: The Foundry test file(s) that exercise the AccessController and
  AccessGuard logic.
- **Dashboard**: The React/TypeScript UI component that fetches and renders the
  AccessMatrix for a connected wallet or admin user.
- **Foundry**: The Solidity smart-contract testing framework (forge) used for the
  test suite.
- **NatSpec**: Ethereum Natural Language Specification format for Solidity inline
  documentation.
- **RBAC**: Role-Based Access Control — the authorization model used throughout this
  feature.

---

## Requirements

### Requirement 1: Role Lifecycle Management

**User Story:** As a contract administrator, I want to create, assign, and revoke roles
dynamically, so that I can manage which accounts are authorized to perform privileged
actions on deployed contracts.

#### Acceptance Criteria

1. THE AccessController SHALL define a `DEFAULT_ADMIN_ROLE` (bytes32 value of `0x00`)
   that acts as the root administrative role.

2. IF a Principal holds the RoleAdmin for a given Role, THEN THE AccessController SHALL
   allow that Principal to grant that Role to any non-zero account address.

3. IF a Principal holds the RoleAdmin for a given Role, THEN THE AccessController SHALL
   allow that Principal to revoke that Role from any account address.

4. WHEN a Role is granted to an account that already holds it, THE AccessController
   SHALL treat the operation as a no-op and SHALL NOT emit a duplicate `RoleGranted`
   event.

5. WHEN a Role is revoked from an account that does not hold it, THE AccessController
   SHALL treat the operation as a no-op and SHALL NOT emit a spurious `RoleRevoked`
   event.

6. THE AccessController SHALL expose a `hasRole(bytes32 role, address account)` query
   that returns a boolean indicating whether the account currently holds the role.

7. THE AccessController SHALL expose a `getRoleAdmin(bytes32 role)` query that returns
   the bytes32 identifier of the role whose holders may grant/revoke the queried role.

8. IF a caller attempts to grant or revoke a Role without holding the corresponding
   RoleAdmin, THEN THE AccessController SHALL revert the transaction with error code
   `AccessControl__UnauthorizedAdminAction`.

9. IF a caller holds the `DEFAULT_ADMIN_ROLE`, THEN THE AccessController SHALL allow
   that caller to change the RoleAdmin of any Role via `setRoleAdmin(bytes32 role,
   bytes32 adminRole)`.

10. IF a caller does not hold the `DEFAULT_ADMIN_ROLE` and attempts to call
    `setRoleAdmin`, THEN THE AccessController SHALL revert with
    `AccessControl__UnauthorizedAdminAction`.

11. THE AccessController SHALL support a `renounceRole(bytes32 role, address account)`
    function that allows a Principal to remove a Role from their own address, provided
    `account == msg.sender`.

12. IF `renounceRole` is called with `account != msg.sender`, THEN THE AccessController
    SHALL revert with `AccessControl__UnauthorizedAdminAction`.

13. IF `grantRole` is called with `account == address(0)`, THEN THE AccessController
    SHALL revert with `AccessControl__InvalidAccount`.

---

### Requirement 2: Granular Permission Tracking

**User Story:** As a security auditor, I want each role to carry a defined set of
granular permissions, so that I can verify that accounts only have the minimum required
capabilities.

#### Acceptance Criteria

1. THE AccessController SHALL store a mapping from each Role to a set of Permission
   identifiers (bytes32).

2. WHEN a Permission is added to a Role, THE AccessController SHALL emit a
   `PermissionAdded(bytes32 indexed role, bytes32 indexed permission)` event.

3. WHEN a Permission is removed from a Role, THE AccessController SHALL emit a
   `PermissionRemoved(bytes32 indexed role, bytes32 indexed permission)` event.

4. THE AccessController SHALL expose a `roleHasPermission(bytes32 role, bytes32 permission)`
   query that returns `true` if the permission is currently associated with the role,
   and `false` otherwise — including for roles that have no permissions assigned.

5. THE AccessController SHALL expose a `getPermissionsForRole(bytes32 role)` query that
   returns the complete list of Permission identifiers currently associated with that
   role; for a role with no permissions, it SHALL return an empty `bytes32[]` array.

6. IF a caller attempts to add or remove a Permission from a Role without holding the
   `DEFAULT_ADMIN_ROLE`, THEN THE AccessController SHALL revert with error code
   `AccessControl__InsufficientPrivilege`.

7. WHEN the same Permission is added to a Role that already contains it, THE
   AccessController SHALL treat the operation as a no-op and SHALL NOT emit a duplicate
   `PermissionAdded` event.

8. WHEN a Permission that is not associated with a Role is removed from that Role, THE
   AccessController SHALL treat the operation as a no-op and SHALL NOT emit a spurious
   `PermissionRemoved` event.

---

### Requirement 3: Access Policy Enforcement

**User Story:** As a smart contract developer, I want reusable access guards that I can
attach to contract entry points, so that unauthorized callers are blocked before any
state mutation occurs.

#### Acceptance Criteria

1. THE AccessController SHALL provide an `onlyRole(bytes32 role)` modifier that reverts
   with error code `AccessControl__MissingRole(bytes32 role, address account)` when the
   caller does not hold the specified role.

2. THE AccessController SHALL provide an `onlyPermission(bytes32 permission)` modifier
   that reverts with error code
   `AccessControl__MissingPermission(bytes32 permission, address account)` when none of
   the caller's roles carry the specified permission.

3. WHILE a Role is revoked from an account, THE AccessController SHALL immediately
   reject any subsequent call gated by `onlyRole` for that role from that account,
   within the same block.

4. WHEN a contract entry point is guarded by `onlyRole`, THE AccessController SHALL
   NOT proceed to execute the function body before the role check passes.

5. THE AccessController SHALL provide a `checkRole(bytes32 role, address account)`
   internal view function that child contracts can call without incurring an external
   call gas overhead.

6. WHERE a function requires multiple roles simultaneously, THE AccessController SHALL
   allow composition of guards so that all specified roles must be held by the caller;
   failure of any single role check SHALL revert with the identifier of the first
   failing role.

---

### Requirement 4: Access Audit Logging

**User Story:** As a compliance officer, I want every authorization action to be
recorded as an on-chain event, so that I can reconstruct a complete audit trail for
regulatory and security review.

#### Acceptance Criteria

1. WHEN a Role is granted to an account, THE AccessController SHALL emit a
   `RoleGranted(bytes32 indexed role, address indexed account, address indexed sender)`
   event.

2. WHEN a Role is revoked from an account, THE AccessController SHALL emit a
   `RoleRevoked(bytes32 indexed role, address indexed account, address indexed sender)`
   event.

3. WHEN a caller is rejected by `onlyRole` or `onlyPermission`, THE AccessController
   SHALL emit an
   `UnauthorizedAccessAttempted(bytes32 indexed role, address indexed account, address indexed target, bytes4 indexed selector)`
   event before reverting the transaction.

4. WHEN a RoleAdmin is changed via `setRoleAdmin`, THE AccessController SHALL emit a
   `RoleAdminChanged(bytes32 indexed role, bytes32 indexed previousAdminRole, bytes32 indexed newAdminRole)`
   event.

5. WHEN a Principal renounces a Role, THE AccessController SHALL emit a
   `RoleRevoked` event with `sender == account`.

6. THE AccessController SHALL index all event parameters marked `indexed` to enable
   efficient off-chain log filtering without requiring full event log scans.

7. THE AccessController SHALL NOT suppress, batch, or delay event emission; each
   authorization action SHALL produce exactly one corresponding event in the same
   transaction.

---

### Requirement 5: Access Control Test Suite

**User Story:** As a smart contract engineer, I want a comprehensive Foundry test suite
that validates all access control behaviors, so that I can detect regressions before
deployment.

#### Acceptance Criteria

1. THE TestSuite SHALL include a test file `AccessControlTest.t.sol` organized into
   named test contracts covering: role lifecycle, permission tracking, policy
   enforcement, and audit logging.

2. WHEN an authorized account calls a role-gated function, THE TestSuite SHALL assert
   that the call succeeds and that the expected state change occurs.

3. WHEN an unauthorized account calls a role-gated function, THE TestSuite SHALL assert
   that the call reverts with the exact error code defined in the AccessController.

4. WHEN a Role is revoked mid-test, THE TestSuite SHALL assert that the previously
   authorized account is blocked on all subsequent role-gated calls within the same test
   scenario.

5. THE TestSuite SHALL include fuzz tests (using Foundry's `vm.assume`) that generate
   arbitrary addresses and role bytes32 values to verify that only holders of the correct
   role pass the `hasRole` check.

6. THE TestSuite SHALL include invariant tests asserting that a Principal with no roles
   assigned can never satisfy `hasRole` for any non-zero role value.

7. THE TestSuite SHALL include tests for all defined error codes to ensure each revert
   path produces the expected error identifier.

8. THE TestSuite SHALL achieve 100% statement coverage of the AccessController contract
   as verified by `forge coverage`.

9. FOR ALL valid role-grant-then-revoke sequences on any address and role pair, THE
   TestSuite SHALL verify that `hasRole` returns `false` after revocation (round-trip
   property: grant → revoke → !hasRole).

10. THE TestSuite SHALL include an event-emission test that calls `vm.expectEmit` to
    assert that each audit event is emitted with the correct indexed parameters.

---

### Requirement 6: Access Control Visualization Dashboard

**User Story:** As a dApp administrator, I want a React/TypeScript UI component that
displays the current access matrix across all monitored contract addresses, so that I
can quickly understand and audit role assignments without reading raw contract state.

#### Acceptance Criteria

1. THE Dashboard SHALL render a table (AccessMatrix) with rows representing contract
   addresses and columns representing Role names, with cells indicating whether each
   Role is active on that contract.

2. WHEN a contract address is selected in the AccessMatrix, THE Dashboard SHALL display
   a detail panel listing all Principals assigned to each Role for that contract along
   with their associated Permissions.

3. THE Dashboard SHALL fetch role and permission data via a typed React hook
   `useAccessMatrix(contractAddresses: string[])` that returns a structured
   `AccessMatrixData` type.

4. WHEN role or permission data is loading, THE Dashboard SHALL display a loading
   skeleton in place of the AccessMatrix to prevent layout shift.

5. IF the data fetch fails, THE Dashboard SHALL display an inline error banner with the
   error message and a retry button, without crashing the component tree; no additional
   fallback behavior beyond the error banner is required.

6. THE Dashboard SHALL accept a `onRoleSelect` callback prop that is invoked with the
   selected role bytes32 identifier and contract address whenever a matrix cell is
   clicked.

7. THE Dashboard component and all sub-components SHALL include JSDoc documentation on
   all exported props interfaces and hook return types.

8. THE Dashboard SHALL be keyboard-navigable: matrix cells SHALL be focusable via Tab
   key, and selection SHALL be triggerable via Enter or Space.

9. WHERE the list of contract addresses is empty, THE Dashboard SHALL render an empty
   state message prompting the administrator to add contract addresses.

10. THE Dashboard SHALL be implemented in TypeScript with strict mode enabled and SHALL
    export all public types from a single barrel file (`index.ts`).

---

### Requirement 7: Security Invariants and Bypass Prevention

**User Story:** As a security engineer, I want the access control system to enforce
hard security invariants that prevent privilege escalation and bypass attacks, so that
compromised or malicious accounts cannot gain unauthorized access.

#### Acceptance Criteria

1. THE AccessController SHALL prevent any account from granting itself a Role it does
   not already hold through the RoleAdmin mechanism; self-escalation SHALL revert with
   `AccessControl__UnauthorizedAdminAction`.

2. WHEN the `DEFAULT_ADMIN_ROLE` has no current holders and a caller attempts to grant
   or revoke the `DEFAULT_ADMIN_ROLE`, THE AccessController SHALL revert with
   `AccessControl__UnauthorizedAdminAction`; checks for other roles SHALL proceed
   under their own RoleAdmin rules and are not affected by the absence of
   `DEFAULT_ADMIN_ROLE` holders.

3. THE AccessController SHALL use custom Solidity errors (not `require` string messages)
   for all revert paths to minimize gas consumption and enable precise off-chain error
   identification.

4. THE AccessController SHALL NOT rely on `tx.origin` for any authorization check;
   all role checks SHALL use `msg.sender` exclusively.

5. IF a reentrancy attack is attempted against a role-gated function, THE AccessController
   SHALL revert the reentrant call via a `ReentrancyGuard`-equivalent mechanism before
   any state mutation is applied in the outer call.

6. THE AccessController SHALL emit `UnauthorizedAccessAttempted` before reverting on
   every failed authorization check, ensuring failed attacks are visible in the event
   log even though the transaction reverts.

7. FOR ALL possible sequences of grant and revoke operations, THE AccessController SHALL
   maintain the invariant that `hasRole(role, account)` reflects the most recent
   finalized state of that role assignment (no stale caches, no race conditions in
   single-threaded EVM execution).

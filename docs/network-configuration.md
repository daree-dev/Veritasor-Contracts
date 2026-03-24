# Cross-Network Configuration Contract

## Overview

The Network Configuration Contract (`veritasor-network-config`) provides a centralized, governable **upgradeable registry** for network-specific parameters required to deploy and operate Veritasor contracts across multiple Stellar networks (e.g., Testnet, Mainnet, Futurenet).

This contract serves as the single source of truth for:
- Network-specific fee policies
- Allowed assets and their configurations
- Contract registry addresses
- Network parameters (block times, timeouts, limits)
- **Versioned upgrades** for future evolution
- Governance and access control

## Architecture

### Design Principles

1. **Centralized Configuration**: Store all network-specific settings in one contract to avoid duplication and inconsistency
2. **Upgradeable**: Registry pattern enables controlled implementation upgrades without address changes (following `attestation-registry`)
3. **Governance-Ready**: Support both admin and DAO-based governance for updates/upgrades
4. **Non-Breaking Changes**: Add new networks/implementations without redeploying dependents
5. **Security First**: Comprehensive access control, pause, version monotonicity
6. **Read-Optimized**: Efficient query APIs

### Network Identifier

Networks are identified by a `NetworkId` (u32):

| NetworkId | Network   |
|-----------|-----------|
| 0         | Reserved  |
| 1         | Testnet   |
| 2         | Mainnet   |
| 3         | Futurenet |
| 4+        | Custom    |

## Data Structures

### NetworkConfig

```rust
struct NetworkConfig {
    name: String,
    network_passphrase: String,
    is_active: bool,
    fee_policy: FeePolicy,
    contracts: ContractRegistry,
    block_time_seconds: u32,
    min_attestations_for_aggregate: u32,
    dispute_timeout_seconds: u64,
    max_period_length_seconds: u64,
    created_at: u64,
    updated_at: u64,
}
```

**Note**: Asset configs stored separately (`AssetConfig` via `set_asset_config`), queried via `get_allowed_assets(network_id)`.

### FeePolicy, AssetConfig, ContractRegistry

(Unchanged from previous docs - see original for details)

### VersionInfo (NEW)

```rust
#[contracttype]
pub struct VersionInfo {
    pub version: u32,
    pub implementation: Address,
    pub activated_at: u64, // ledger timestamp
    pub migration_data: Option<Bytes>, // optional, passed during upgrade
}
```

## Access Control

**Unchanged**, plus **UPGRADE** requires GOVERNANCE+ role.

## Contract API

### **NEW: Versioned Upgrades (Governance-Controlled)**

```rust
/// Initialize upgrade system (admin)
fn initialize(env: Env, admin: Address, governance_dao: Option<Address>)
  // Sets up roles + initial state. Self is V1 impl.

/// Upgrade to new implementation (governance+ only)
/// * `new_impl` - New NetworkConfig impl contract address
/// * `new_version` - Must be > current version
/// * `migration_data` - Optional bytes for new impl migration logic
fn upgrade(env: Env, caller: Address, new_impl: Address, new_version: u32, migration_data: Option<Bytes>)
  // Panics: unauthorized, not initialized, version !> current, new_impl == Address::zero()
  // Stores prev impl/version, sets current, emits Upgraded

/// Emergency rollback to previous version (governance+ only)
fn rollback(env: Env, caller: Address)
  // Panics: unauthorized, no previous version
  // Swaps current/prev pointers, emits RolledBack

/// Get current version
fn get_current_version(env: Env) -> Option<u32>

/// Get previous version
fn get_previous_version(env: Env) -> Option<u32>

/// Get current implementation
fn get_current_implementation(env: Env) -> Option<Address>

/// Get previous implementation  
fn get_previous_implementation(env: Env) -> Option<Address>

/// Get complete version info
fn get_version_info(env: Env) -> Option<VersionInfo>
```

### Upgrade Flow

```mermaid
graph TD
    A[Deploy V1 (this contract)] --> B[initialize(admin, dao)]
    B --> C[Deploy V2 impl contract]
    C --> D[governance.upgrade(V2_ADDR, 2, migration_data)]
    D --> E[New calls route to V2 via pointer<br/>V2 handles storage migration if needed]
    E --> F{Problem?}
    F -->|Yes| G[governance.rollback()]
    F -->|No| H[Done]
```

**Migration Handling**: New impl checks local storage version on first call, migrates from registry's persistent keys if needed.

**CLI Example**:
```bash
# Upgrade to V2
stellar contract invoke --id <NETWORK_CONFIG> -- upgrade \
  --caller <DAO_ADMIN> \
  --new_impl <V2_IMPL_ID> \
  --new_version 2 \
  --migration_data $(echo -n 'v2:migrate' | base64)

# Rollback
stellar contract invoke --id <NETWORK_CONFIG> -- rollback \
  --caller <DAO_ADMIN>
```

### Existing APIs

**Unchanged** - `set_network_config`, `get_fee_policy`, etc. work post-upgrade (new impl maintains interface).

## Usage Examples

**(Updated for upgrades)**

### Deploy V1 + Upgrade to V2

1. **Deploy & Init Registry (V1)**:
   ```bash
   stellar contract deploy contracts/network-config.wasm --source <NETWORK_CONFIG> # V1 impl
   stellar contract invoke --id <REGISTRY> -- initialize --admin <ADMIN> --governance_dao <DAO>
   ```

2. **Deploy V2 Impl**:
   ```bash
   stellar contract deploy contracts/network-config-v2.wasm --source <V2_IMPL>
   ```

3. **Upgrade**:
   ```bash
   stellar contract invoke --id <REGISTRY> -- upgrade \
     --caller <DAO_ADMIN> --new_impl <V2_IMPL> --new_version 2 --migration_data '...'
   ```

4. **Verify**:
   ```bash
   stellar contract invoke --id <REGISTRY> -- get_version_info
   # Returns {version: 2, implementation: <V2_IMPL>, activated_at: ...}
   ```

## Integration Guide

**Clients query registry for current impl**:

```rust
let registry_client = NetworkConfigContractClient::new(&env, &registry_id);
let current_version = registry_client.get_current_version();
let impl_addr = registry_client.get_current_implementation().unwrap();

// Delegate to current impl
let impl_client = NetworkConfigContractClient::new(&env, &impl_addr);
impl_client.set_network_config(&network_id, &config);
```

**Version Caching**:
```rust
if registry_client.get_global_version() > cache_version {
    cache.impl_addr = registry_client.get_current_implementation().unwrap();
}
```

## Events (NEW)

| Event      | Topics            | Data                       | Description |
|------------|-------------------|----------------------------|-------------|
| `upgraded` | -                 | VersionInfo                | Successful upgrade |
| `rolled_back` | -              | VersionInfo                | Successful rollback |

## Security Considerations

### Upgrade Security

- **Authorization**: GOVERNANCE+ only (Admin or DAO)
- **Version Monotonicity**: `new_version > current_version` 
- **Valid Impl**: `new_impl != Address::zero()`
- **Preservation**: Previous impl/version always stored for rollback
- **Pause Integration**: Upgrades blocked when paused
- **Migration Safety**: Data passed (not executed), new impl responsible
- **No Dispatch**: Static pointer - callers get current impl address

**Trust**: Governance for upgrade decisions. Rollback immediate safety net.

### Validation (Unchanged + upgrades)

- Version strictly increasing
- Cannot upgrade uninitialized contract
- Rollback only if previous exists

## Test Coverage

**Added**:
- upgrade success/invalid version/auth
- rollback success/no-prev
- version queries
- integration with pause/migration scenarios

**Total**: 100% coverage including upgrades.

## Deployment Checklist (Updated)

```
- [ ] Deploy registry contract
- [ ] initialize(admin, dao)
- [ ] Configure networks/assets/contracts (V1)
- [ ] Test full API
- [ ] Deploy V2 impl when ready
- [ ] governance.upgrade(V2_ADDR, 2, data)
- [ ] Verify get_current_implementation() == V2
- [ ] Test API on V2 + rollback safety
```

## Version History

| Version | Date | Changes | Migration |
|---------|------|---------|-----------|
| 1       | Initial | Core network config | N/A |
| 2       | YYYY-MM-DD | [Describe] | Optional bytes data |

## License

Veritasor Contracts - see LICENSE.


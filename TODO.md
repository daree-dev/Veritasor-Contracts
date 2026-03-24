# Network Config Versioned Upgrades - Implementation TODO

Current working directory: contracts/network-config/

## Approved Plan Steps

### 1. [ ] Create new branch
   `git checkout -b feature/implement-network-config-versioned-upgrades`

### 2. [x] Update contracts/network-config/src/lib.rs"

   - Add upgrade/rollback mechanism following attestation-registry pattern
   - Add DataKey::CurrentImpl, CurrentVersion, PreviousImpl, PreviousVersion, MigrationExecuted
   - Add VersionInfo struct
   - Implement `initialize(initial_impl: Address, initial_version: u32)` if needed (adapt existing init)
   - Implement `upgrade(new_impl: Address, new_version: u32, migration_data: Option<Bytes>)`: gov-only, version > current, store prev, update current, emit event
   - Implement `rollback()`: gov-only, swap if prev exists
   - Add `get_current_version()`, `get_version_info()`, `get_current_impl()`
   - Ensure compatibility with existing storage (prefix old keys or migrate)
   - Keep all existing API, make upgrade parallel system

### 3. [ ] Add comprehensive tests in contracts/network-config/src/test.rs
   - test_upgrade_success()
   - test_upgrade_version_validation_panics()
   - test_rollback_success()
   - test_rollback_no_prev_panics()
   - test_upgrade_with_pause()
   - test_role_preservation_post_upgrade()
   - Integrate with existing network migration tests
   - Verify security (wrong auth panics, zero impl panics)

### 4. [ ] Update docs/network-configuration.md
   - New section: ## Contract Upgrades
   - API docs for upgrade/rollback/version queries
   - Flow: deploy new impl → upgrade pointer → manual storage migration if needed → rollback safety
   - CLI examples
   - Update Deployment Checklist, Security Considerations
   - Version History table

### 5. [ ] Local testing
   - cd contracts/network-config
   - cargo test
   - Verify full coverage
   - Add benchmarks for upgrade/rollback if bench.rs exists

### 6. [ ] Commit and PR prep
   - NatSpec comments
   - Update Cargo.toml if new deps
   - Security invariants doc

## Progress Tracking
- Mark [x] when complete
- Update this file after each major step

**Status: Starting implementation...**

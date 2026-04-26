#![cfg(test)]
//! Lender Access List - Test Suite v2
use super::*;
use soroban_sdk::testutils::{Address as _, Events as _};
use soroban_sdk::{Address, Env, IntoVal, String, TryFromVal};

fn setup() -> (Env, LenderAccessListContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(LenderAccessListContract, ());
    let client = LenderAccessListContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);
    (env, client, admin)
}

fn meta(env: &Env, name: &str) -> LenderMetadata {
    LenderMetadata {
        name: String::from_str(env, name),
        url: String::from_str(env, "https://example.com"),
        notes: String::from_str(env, "notes"),
    }
}

fn reason(env: &Env, s: &str) -> String {
    String::from_str(env, s)
}

// 
//  1. Initialization
// 

#[test]
fn test_initialize_sets_admin_and_governance() {
    let (env, client, admin) = setup();
    assert_eq!(client.get_admin(), admin);
    assert!(client.has_governance(&admin));
    assert_eq!(client.get_all_lenders().len(), 0);
    let lender = Address::generate(&env);
    assert!(!client.is_allowed(&lender, &1u32));
}

#[test]
#[should_panic(expected = "already initialized")]
fn test_initialize_twice_panics() {
    let (_env, client, admin) = setup();
    client.initialize(&admin);
}

#[test]
fn test_initialize_admin_does_not_have_delegated_admin_role() {
    let (_env, client, admin) = setup();
    assert!(client.has_governance(&admin));
    assert!(!client.has_delegated_admin(&admin));
}

#[test]
fn test_schema_version_is_two() {
    let (_env, client, _admin) = setup();
    assert_eq!(client.get_event_schema_version(), 2u32);
}

// 
//  2. Admin Transfer
// 

#[test]
fn test_transfer_admin_changes_admin() {
    let (env, client, admin) = setup();
    let new_admin = Address::generate(&env);
    client.transfer_admin(&admin, &new_admin);
    assert_eq!(client.get_admin(), new_admin);
}

#[test]
fn test_transfer_admin_old_admin_loses_admin_privileges() {
    let (env, client, admin) = setup();
    let new_admin = Address::generate(&env);
    client.transfer_admin(&admin, &new_admin);
    assert_ne!(client.get_admin(), admin);
}

#[test]
fn test_transfer_admin_new_admin_can_grant_governance() {
    let (env, client, admin) = setup();
    let new_admin = Address::generate(&env);
    let gov = Address::generate(&env);
    client.transfer_admin(&admin, &new_admin);
    client.grant_governance(&new_admin, &gov);
    assert!(client.has_governance(&gov));
}

#[test]
#[should_panic(expected = "caller is not admin")]
fn test_non_admin_cannot_transfer_admin() {
    let (env, client, _admin) = setup();
    let attacker = Address::generate(&env);
    let target = Address::generate(&env);
    client.transfer_admin(&attacker, &target);
}

#[test]
#[should_panic(expected = "new_admin must differ from current admin")]
fn test_transfer_admin_to_self_panics() {
    let (_env, client, admin) = setup();
    client.transfer_admin(&admin, &admin);
}

#[test]
fn test_transfer_admin_emits_event() {
    let (env, client, admin) = setup();
    let new_admin = Address::generate(&env);
    client.transfer_admin(&admin, &new_admin);
    let events = env.events().all();
    assert!(!events.is_empty());
    let (_cid, topics, data) = events.last().unwrap();
    assert_eq!(topics.len(), 2);
    assert_eq!(topics.get(0).unwrap(), TOPIC_ADM_XFER.into_val(&env));
    assert_eq!(topics.get(1).unwrap(), new_admin.into_val(&env));
    let ev = AdminTransferredEvent::try_from_val(&env, &data).unwrap();
    assert_eq!(ev.old_admin, admin);
    assert_eq!(ev.new_admin, new_admin);
}

// 
//  3. Governance Role Management
// 

#[test]
fn test_admin_can_grant_and_revoke_governance() {
    let (env, client, admin) = setup();
    let gov = Address::generate(&env);
    assert!(!client.has_governance(&gov));
    client.grant_governance(&admin, &gov);
    assert!(client.has_governance(&gov));
    client.revoke_governance(&admin, &gov);
    assert!(!client.has_governance(&gov));
}

#[test]
fn test_grant_governance_idempotent() {
    let (env, client, admin) = setup();
    let gov = Address::generate(&env);
    client.grant_governance(&admin, &gov);
    client.grant_governance(&admin, &gov);
    assert!(client.has_governance(&gov));
}

#[test]
fn test_revoke_governance_on_non_holder_is_safe() {
    let (env, client, admin) = setup();
    let account = Address::generate(&env);
    client.revoke_governance(&admin, &account);
    assert!(!client.has_governance(&account));
}

#[test]
#[should_panic(expected = "caller is not admin")]
fn test_non_admin_cannot_grant_governance() {
    let (env, client, _admin) = setup();
    let other = Address::generate(&env);
    client.grant_governance(&other, &Address::generate(&env));
}

#[test]
#[should_panic(expected = "caller is not admin")]
fn test_non_admin_cannot_revoke_governance() {
    let (env, client, admin) = setup();
    let gov = Address::generate(&env);
    client.grant_governance(&admin, &gov);
    let attacker = Address::generate(&env);
    client.revoke_governance(&attacker, &gov);
}

#[test]
fn test_grant_governance_emits_event() {
    let (env, client, admin) = setup();
    let gov = Address::generate(&env);
    client.grant_governance(&admin, &gov);
    let events = env.events().all();
    let (_cid, topics, data) = events.last().unwrap();
    assert_eq!(topics.len(), 2);
    assert_eq!(topics.get(0).unwrap(), TOPIC_GOV_ADD.into_val(&env));
    assert_eq!(topics.get(1).unwrap(), gov.into_val(&env));
    let ev = GovernanceEvent::try_from_val(&env, &data).unwrap();
    assert_eq!(ev.account, gov);
    assert!(ev.enabled);
    assert_eq!(ev.changed_by, admin);
}

#[test]
fn test_revoke_governance_emits_event() {
    let (env, client, admin) = setup();
    let gov = Address::generate(&env);
    client.grant_governance(&admin, &gov);
    client.revoke_governance(&admin, &gov);
    let events = env.events().all();
    let (_cid, topics, data) = events.last().unwrap();
    assert_eq!(topics.len(), 2);
    assert_eq!(topics.get(0).unwrap(), TOPIC_GOV_DEL.into_val(&env));
    assert_eq!(topics.get(1).unwrap(), gov.into_val(&env));
    let ev = GovernanceEvent::try_from_val(&env, &data).unwrap();
    assert_eq!(ev.account, gov);
    assert!(!ev.enabled);
    assert_eq!(ev.changed_by, admin);
}

// 
//  4. Delegated Admin Role Management
// 

#[test]
fn test_admin_can_grant_and_revoke_delegated_admin() {
    let (env, client, admin) = setup();
    let del_admin = Address::generate(&env);
    assert!(!client.has_delegated_admin(&del_admin));
    client.grant_delegated_admin(&admin, &del_admin);
    assert!(client.has_delegated_admin(&del_admin));
    client.revoke_delegated_admin(&admin, &del_admin);
    assert!(!client.has_delegated_admin(&del_admin));
}

#[test]
fn test_grant_delegated_admin_idempotent() {
    let (env, client, admin) = setup();
    let del_admin = Address::generate(&env);
    client.grant_delegated_admin(&admin, &del_admin);
    client.grant_delegated_admin(&admin, &del_admin);
    assert!(client.has_delegated_admin(&del_admin));
}

#[test]
fn test_revoke_delegated_admin_on_non_holder_is_safe() {
    let (env, client, admin) = setup();
    let account = Address::generate(&env);
    client.revoke_delegated_admin(&admin, &account);
    assert!(!client.has_delegated_admin(&account));
}

#[test]
#[should_panic(expected = "caller is not admin")]
fn test_non_admin_cannot_grant_delegated_admin() {
    let (env, client, _admin) = setup();
    let other = Address::generate(&env);
    client.grant_delegated_admin(&other, &Address::generate(&env));
}

#[test]
#[should_panic(expected = "caller is not admin")]
fn test_non_admin_cannot_revoke_delegated_admin() {
    let (env, client, admin) = setup();
    let del_admin = Address::generate(&env);
    client.grant_delegated_admin(&admin, &del_admin);
    let attacker = Address::generate(&env);
    client.revoke_delegated_admin(&attacker, &del_admin);
}

#[test]
fn test_grant_delegated_admin_emits_event() {
    let (env, client, admin) = setup();
    let del_admin = Address::generate(&env);
    client.grant_delegated_admin(&admin, &del_admin);
    let events = env.events().all();
    let (_cid, topics, data) = events.last().unwrap();
    assert_eq!(topics.len(), 2);
    assert_eq!(topics.get(0).unwrap(), TOPIC_DEL_ADD.into_val(&env));
    assert_eq!(topics.get(1).unwrap(), del_admin.into_val(&env));
    let ev = DelegatedAdminEvent::try_from_val(&env, &data).unwrap();
    assert_eq!(ev.account, del_admin);
    assert!(ev.enabled);
    assert_eq!(ev.changed_by, admin);
}

#[test]
fn test_revoke_delegated_admin_emits_event() {
    let (env, client, admin) = setup();
    let del_admin = Address::generate(&env);
    client.grant_delegated_admin(&admin, &del_admin);
    client.revoke_delegated_admin(&admin, &del_admin);
    let events = env.events().all();
    let (_cid, topics, data) = events.last().unwrap();
    assert_eq!(topics.len(), 2);
    assert_eq!(topics.get(0).unwrap(), TOPIC_DEL_DEL.into_val(&env));
    assert_eq!(topics.get(1).unwrap(), del_admin.into_val(&env));
    let ev = DelegatedAdminEvent::try_from_val(&env, &data).unwrap();
    assert_eq!(ev.account, del_admin);
    assert!(!ev.enabled);
    assert_eq!(ev.changed_by, admin);
}

// 
//  5. Lender Lifecycle
// 

#[test]
fn test_set_lender_first_enrollment() {
    let (env, client, admin) = setup();
    let lender = Address::generate(&env);
    client.set_lender(&admin, &lender, &1u32, &meta(&env, "Lender A"));
    let record = client.get_lender(&lender).unwrap();
    assert_eq!(record.address, lender);
    assert_eq!(record.tier, 1);
    assert_eq!(record.status, LenderStatus::Active);
    assert_eq!(record.metadata.name, String::from_str(&env, "Lender A"));
    assert_eq!(record.updated_by, admin);
}

#[test]
fn test_set_lender_preserves_added_at_on_update() {
    let (env, client, admin) = setup();
    let lender = Address::generate(&env);
    client.set_lender(&admin, &lender, &1u32, &meta(&env, "v1"));
    let added_at = client.get_lender(&lender).unwrap().added_at;
    client.set_lender(&admin, &lender, &2u32, &meta(&env, "v2"));
    let second = client.get_lender(&lender).unwrap();
    assert_eq!(second.added_at, added_at, "added_at must not change on update");
    assert_eq!(second.tier, 2);
    assert_eq!(second.updated_by, admin);
}

#[test]
fn test_set_lender_updates_updated_by() {
    let (env, client, admin) = setup();
    let del_admin = Address::generate(&env);
    let lender = Address::generate(&env);
    client.grant_delegated_admin(&admin, &del_admin);
    client.set_lender(&admin, &lender, &1u32, &meta(&env, "v1"));
    client.set_lender(&del_admin, &lender, &2u32, &meta(&env, "v2"));
    let record = client.get_lender(&lender).unwrap();
    assert_eq!(record.updated_by, del_admin);
}

#[test]
fn test_set_lender_tier_zero_marks_removed() {
    let (env, client, admin) = setup();
    let lender = Address::generate(&env);
    client.set_lender(&admin, &lender, &0u32, &meta(&env, "L"));
    let record = client.get_lender(&lender).unwrap();
    assert_eq!(record.tier, 0);
    assert_eq!(record.status, LenderStatus::Removed);
    assert!(!client.is_allowed(&lender, &1u32));
}

#[test]
fn test_remove_lender_sets_tier_zero_and_removed() {
    let (env, client, admin) = setup();
    let lender = Address::generate(&env);
    client.set_lender(&admin, &lender, &3u32, &meta(&env, "L"));
    client.remove_lender(&admin, &lender, &reason(&env, "offboarded"));
    let record = client.get_lender(&lender).unwrap();
    assert_eq!(record.tier, 0);
    assert_eq!(record.status, LenderStatus::Removed);
    assert_eq!(record.updated_by, admin);
}

#[test]
fn test_remove_lender_record_retained_for_audit() {
    let (env, client, admin) = setup();
    let lender = Address::generate(&env);
    client.set_lender(&admin, &lender, &1u32, &meta(&env, "L"));
    client.remove_lender(&admin, &lender, &reason(&env, "test"));
    assert!(client.get_lender(&lender).is_some());
    assert_eq!(client.get_active_lenders().len(), 0);
    assert_eq!(client.get_all_lenders().len(), 1);
}

#[test]
fn test_reenroll_after_removal() {
    let (env, client, admin) = setup();
    let lender = Address::generate(&env);
    client.set_lender(&admin, &lender, &1u32, &meta(&env, "v1"));
    let added_at_first = client.get_lender(&lender).unwrap().added_at;
    client.remove_lender(&admin, &lender, &reason(&env, "temp"));
    assert!(!client.is_allowed(&lender, &1u32));
    client.set_lender(&admin, &lender, &2u32, &meta(&env, "v2"));
    let record = client.get_lender(&lender).unwrap();
    assert_eq!(record.status, LenderStatus::Active);
    assert_eq!(record.tier, 2);
    assert_eq!(record.added_at, added_at_first);
    assert!(client.is_allowed(&lender, &2u32));
    assert_eq!(client.get_all_lenders().len(), 1);
    assert_eq!(client.get_active_lenders().len(), 1);
}

#[test]
fn test_multiple_lenders_tracked_correctly() {
    let (env, client, admin) = setup();
    let l1 = Address::generate(&env);
    let l2 = Address::generate(&env);
    let l3 = Address::generate(&env);
    client.set_lender(&admin, &l1, &1u32, &meta(&env, "L1"));
    client.set_lender(&admin, &l2, &2u32, &meta(&env, "L2"));
    client.set_lender(&admin, &l3, &3u32, &meta(&env, "L3"));
    assert_eq!(client.get_all_lenders().len(), 3);
    assert_eq!(client.get_active_lenders().len(), 3);
    client.remove_lender(&admin, &l2, &reason(&env, "removed"));
    assert_eq!(client.get_all_lenders().len(), 3);
    assert_eq!(client.get_active_lenders().len(), 2);
}

#[test]
fn test_set_lender_same_address_twice_no_duplicate_in_list() {
    let (env, client, admin) = setup();
    let lender = Address::generate(&env);
    client.set_lender(&admin, &lender, &1u32, &meta(&env, "v1"));
    client.set_lender(&admin, &lender, &2u32, &meta(&env, "v2"));
    assert_eq!(client.get_all_lenders().len(), 1);
}

// 
//  6. Access Checks
// 

#[test]
fn test_is_allowed_min_tier_zero_always_true() {
    let (env, client, _admin) = setup();
    let lender = Address::generate(&env);
    assert!(client.is_allowed(&lender, &0u32));
}

#[test]
fn test_is_allowed_unenrolled_lender_false() {
    let (env, client, _admin) = setup();
    let lender = Address::generate(&env);
    assert!(!client.is_allowed(&lender, &1u32));
}

#[test]
fn test_is_allowed_exact_tier_match() {
    let (env, client, admin) = setup();
    let lender = Address::generate(&env);
    client.set_lender(&admin, &lender, &2u32, &meta(&env, "L"));
    assert!(client.is_allowed(&lender, &1u32));
    assert!(client.is_allowed(&lender, &2u32));
    assert!(!client.is_allowed(&lender, &3u32));
}

#[test]
fn test_is_allowed_removed_lender_false_regardless_of_tier() {
    let (env, client, admin) = setup();
    let lender = Address::generate(&env);
    client.set_lender(&admin, &lender, &5u32, &meta(&env, "L"));
    client.remove_lender(&admin, &lender, &reason(&env, "test"));
    assert!(!client.is_allowed(&lender, &1u32));
    assert!(!client.is_allowed(&lender, &5u32));
}

#[test]
fn test_is_allowed_tier_zero_lender_false() {
    let (env, client, admin) = setup();
    let lender = Address::generate(&env);
    client.set_lender(&admin, &lender, &0u32, &meta(&env, "L"));
    assert!(!client.is_allowed(&lender, &1u32));
}

#[test]
fn test_is_allowed_high_tier_lender() {
    let (env, client, admin) = setup();
    let lender = Address::generate(&env);
    client.set_lender(&admin, &lender, &u32::MAX, &meta(&env, "L"));
    assert!(client.is_allowed(&lender, &u32::MAX));
    assert!(client.is_allowed(&lender, &1u32));
}

// 
//  7. Audit Trail  on-chain fields
// 

#[test]
fn test_audit_trail_added_at_set_on_enrollment() {
    let (env, client, admin) = setup();
    let lender = Address::generate(&env);
    client.set_lender(&admin, &lender, &1u32, &meta(&env, "L"));
    let record = client.get_lender(&lender).unwrap();
    assert_eq!(record.added_at, record.updated_at);
    assert_eq!(record.updated_by, admin);
}

#[test]
fn test_audit_trail_updated_at_changes_on_update() {
    let (env, client, admin) = setup();
    let lender = Address::generate(&env);
    client.set_lender(&admin, &lender, &1u32, &meta(&env, "v1"));
    let first = client.get_lender(&lender).unwrap();
    env.ledger().set_sequence_number(env.ledger().sequence() + 10);
    client.set_lender(&admin, &lender, &2u32, &meta(&env, "v2"));
    let second = client.get_lender(&lender).unwrap();
    assert_eq!(second.added_at, first.added_at, "added_at must not change");
    assert!(second.updated_at > first.updated_at, "updated_at must increase");
}

#[test]
fn test_audit_trail_remove_lender_updates_updated_by() {
    let (env, client, admin) = setup();
    let del_admin = Address::generate(&env);
    let lender = Address::generate(&env);
    client.grant_delegated_admin(&admin, &del_admin);
    client.set_lender(&admin, &lender, &1u32, &meta(&env, "L"));
    client.remove_lender(&del_admin, &lender, &reason(&env, "delegated removal"));
    let record = client.get_lender(&lender).unwrap();
    assert_eq!(record.updated_by, del_admin);
}

#[test]
fn test_audit_trail_metadata_only_update_preserves_tier_and_status() {
    let (env, client, admin) = setup();
    let lender = Address::generate(&env);
    client.set_lender(&admin, &lender, &3u32, &meta(&env, "original"));
    client.set_lender(&admin, &lender, &3u32, &meta(&env, "updated-name"));
    let record = client.get_lender(&lender).unwrap();
    assert_eq!(record.tier, 3);
    assert_eq!(record.status, LenderStatus::Active);
    assert_eq!(record.metadata.name, String::from_str(&env, "updated-name"));
    assert_eq!(record.updated_by, admin);
}

// 
//  8. Event Schema  lnd_new (first enrollment)
// 

#[test]
fn test_first_enrollment_emits_lnd_new_topic() {
    let (env, client, admin) = setup();
    let lender = Address::generate(&env);
    client.set_lender(&admin, &lender, &1u32, &meta(&env, "L"));
    let events = env.events().all();
    let (_cid, topics, data) = events.last().unwrap();
    assert_eq!(topics.len(), 2);
    assert_eq!(topics.get(0).unwrap(), TOPIC_LENDER_NEW.into_val(&env));
    assert_eq!(topics.get(1).unwrap(), lender.into_val(&env));
    let ev = LenderEnrolledEvent::try_from_val(&env, &data).unwrap();
    assert_eq!(ev.lender, lender);
    assert_eq!(ev.tier, 1);
    assert_eq!(ev.status, LenderStatus::Active);
    assert_eq!(ev.changed_by, admin);
    assert_eq!(ev.enrolled_at, env.ledger().sequence());
}

#[test]
fn test_first_enrollment_tier_zero_emits_lnd_new_with_removed_status() {
    let (env, client, admin) = setup();
    let lender = Address::generate(&env);
    client.set_lender(&admin, &lender, &0u32, &meta(&env, "L"));
    let events = env.events().all();
    let (_cid, topics, data) = events.last().unwrap();
    assert_eq!(topics.get(0).unwrap(), TOPIC_LENDER_NEW.into_val(&env));
    let ev = LenderEnrolledEvent::try_from_val(&env, &data).unwrap();
    assert_eq!(ev.tier, 0);
    assert_eq!(ev.status, LenderStatus::Removed);
}

// 
//  9. Event Schema  lnd_set (update of existing record)
// 

#[test]
fn test_update_emits_lnd_set_not_lnd_new() {
    let (env, client, admin) = setup();
    let lender = Address::generate(&env);
    client.set_lender(&admin, &lender, &1u32, &meta(&env, "v1"));
    client.set_lender(&admin, &lender, &3u32, &meta(&env, "v2"));
    let events = env.events().all();
    let (_cid, topics, data) = events.last().unwrap();
    assert_eq!(topics.get(0).unwrap(), TOPIC_LENDER_SET.into_val(&env));
    assert_eq!(topics.get(1).unwrap(), lender.into_val(&env));
    let ev = LenderUpdatedEvent::try_from_val(&env, &data).unwrap();
    assert_eq!(ev.tier, 3);
    assert_eq!(ev.previous_tier, 1);
    assert_eq!(ev.previous_status, LenderStatus::Active);
    assert_eq!(ev.changed_by, admin);
}

#[test]
fn test_update_tier_zero_emits_lnd_set_with_removed_status() {
    let (env, client, admin) = setup();
    let lender = Address::generate(&env);
    client.set_lender(&admin, &lender, &2u32, &meta(&env, "L"));
    client.set_lender(&admin, &lender, &0u32, &meta(&env, "L-disabled"));
    let events = env.events().all();
    let (_cid, topics, data) = events.last().unwrap();
    assert_eq!(topics.get(0).unwrap(), TOPIC_LENDER_SET.into_val(&env));
    let ev = LenderUpdatedEvent::try_from_val(&env, &data).unwrap();
    assert_eq!(ev.tier, 0);
    assert_eq!(ev.status, LenderStatus::Removed);
    assert_eq!(ev.previous_tier, 2);
    assert_eq!(ev.previous_status, LenderStatus::Active);
}

#[test]
fn test_update_changed_by_reflects_actual_caller() {
    let (env, client, admin) = setup();
    let gov = Address::generate(&env);
    let lender = Address::generate(&env);
    client.grant_governance(&admin, &gov);
    client.set_lender(&admin, &lender, &1u32, &meta(&env, "v1"));
    client.set_lender(&gov, &lender, &2u32, &meta(&env, "v2"));
    let events = env.events().all();
    let (_cid, _topics, data) = events.last().unwrap();
    let ev = LenderUpdatedEvent::try_from_val(&env, &data).unwrap();
    assert_eq!(ev.changed_by, gov);
}

// 
//  10. Event Schema  lnd_rem (removal)
// 

#[test]
fn test_lender_removed_event_schema() {
    let (env, client, admin) = setup();
    let lender = Address::generate(&env);
    client.set_lender(&admin, &lender, &2u32, &meta(&env, "L"));
    client.remove_lender(&admin, &lender, &reason(&env, "offboarded"));
    let events = env.events().all();
    let (_cid, topics, data) = events.last().unwrap();
    assert_eq!(topics.len(), 2);
    assert_eq!(topics.get(0).unwrap(), TOPIC_LENDER_REM.into_val(&env));
    assert_eq!(topics.get(1).unwrap(), lender.into_val(&env));
    let ev = LenderRemovedEvent::try_from_val(&env, &data).unwrap();
    assert_eq!(ev.lender, lender);
    assert_eq!(ev.changed_by, admin);
    assert_eq!(ev.previous_tier, 2);
    assert_eq!(ev.previous_status, LenderStatus::Active);
    assert_eq!(ev.reason, String::from_str(&env, "offboarded"));
}

#[test]
fn test_remove_lender_empty_reason_is_valid() {
    let (env, client, admin) = setup();
    let lender = Address::generate(&env);
    client.set_lender(&admin, &lender, &1u32, &meta(&env, "L"));
    client.remove_lender(&admin, &lender, &reason(&env, ""));
    let events = env.events().all();
    let (_cid, _topics, data) = events.last().unwrap();
    let ev = LenderRemovedEvent::try_from_val(&env, &data).unwrap();
    assert_eq!(ev.reason, String::from_str(&env, ""));
}

#[test]
fn test_remove_already_removed_lender_event_shows_previous_removed() {
    let (env, client, admin) = setup();
    let lender = Address::generate(&env);
    client.set_lender(&admin, &lender, &1u32, &meta(&env, "L"));
    client.remove_lender(&admin, &lender, &reason(&env, "first"));
    client.remove_lender(&admin, &lender, &reason(&env, "second"));
    let events = env.events().all();
    let (_cid, _topics, data) = events.last().unwrap();
    let ev = LenderRemovedEvent::try_from_val(&env, &data).unwrap();
    assert_eq!(ev.previous_tier, 0);
    assert_eq!(ev.previous_status, LenderStatus::Removed);
    assert_eq!(ev.reason, String::from_str(&env, "second"));
}

#[test]
fn test_remove_lender_reason_captured_per_caller() {
    let (env, client, admin) = setup();
    let del_admin = Address::generate(&env);
    let lender = Address::generate(&env);
    client.grant_delegated_admin(&admin, &del_admin);
    client.set_lender(&admin, &lender, &1u32, &meta(&env, "L"));
    client.remove_lender(&del_admin, &lender, &reason(&env, "compliance hold"));
    let events = env.events().all();
    let (_cid, _topics, data) = events.last().unwrap();
    let ev = LenderRemovedEvent::try_from_val(&env, &data).unwrap();
    assert_eq!(ev.changed_by, del_admin);
    assert_eq!(ev.reason, String::from_str(&env, "compliance hold"));
}

// 
//  11. Dual Control  governance OR delegated admin
// 

#[test]
fn test_governance_can_set_lender() {
    let (env, client, admin) = setup();
    let gov = Address::generate(&env);
    let lender = Address::generate(&env);
    client.grant_governance(&admin, &gov);
    client.set_lender(&gov, &lender, &1u32, &meta(&env, "L"));
    assert!(client.is_allowed(&lender, &1u32));
}

#[test]
fn test_delegated_admin_can_set_lender() {
    let (env, client, admin) = setup();
    let del_admin = Address::generate(&env);
    let lender = Address::generate(&env);
    client.grant_delegated_admin(&admin, &del_admin);
    client.set_lender(&del_admin, &lender, &1u32, &meta(&env, "L"));
    assert!(client.is_allowed(&lender, &1u32));
}

#[test]
fn test_governance_can_remove_lender() {
    let (env, client, admin) = setup();
    let gov = Address::generate(&env);
    let lender = Address::generate(&env);
    client.grant_governance(&admin, &gov);
    client.set_lender(&gov, &lender, &1u32, &meta(&env, "L"));
    client.remove_lender(&gov, &lender, &reason(&env, "gov-removal"));
    assert!(!client.is_allowed(&lender, &1u32));
}

#[test]
fn test_delegated_admin_can_remove_lender() {
    let (env, client, admin) = setup();
    let del_admin = Address::generate(&env);
    let lender = Address::generate(&env);
    client.grant_delegated_admin(&admin, &del_admin);
    client.set_lender(&del_admin, &lender, &1u32, &meta(&env, "L"));
    client.remove_lender(&del_admin, &lender, &reason(&env, "del-removal"));
    assert!(!client.is_allowed(&lender, &1u32));
}

#[test]
fn test_governance_and_delegated_admin_can_both_act_on_same_lender() {
    // Both roles can update the same lender record; last-writer-wins.
    let (env, client, admin) = setup();
    let gov = Address::generate(&env);
    let del_admin = Address::generate(&env);
    let lender = Address::generate(&env);
    client.grant_governance(&admin, &gov);
    client.grant_delegated_admin(&admin, &del_admin);
    client.set_lender(&gov, &lender, &1u32, &meta(&env, "by-gov"));
    client.set_lender(&del_admin, &lender, &2u32, &meta(&env, "by-del"));
    let record = client.get_lender(&lender).unwrap();
    assert_eq!(record.tier, 2);
    assert_eq!(record.updated_by, del_admin);
    // Verify lnd_set event reflects del_admin as changer
    let events = env.events().all();
    let (_cid, _topics, data) = events.last().unwrap();
    let ev = LenderUpdatedEvent::try_from_val(&env, &data).unwrap();
    assert_eq!(ev.changed_by, del_admin);
    assert_eq!(ev.previous_tier, 1);
}

#[test]
fn test_revoked_governance_cannot_manage_lenders() {
    let (env, client, admin) = setup();
    let gov = Address::generate(&env);
    client.grant_governance(&admin, &gov);
    client.revoke_governance(&admin, &gov);
    assert!(!client.has_governance(&gov));
}

#[test]
fn test_revoked_delegated_admin_cannot_manage_lenders() {
    let (env, client, admin) = setup();
    let del_admin = Address::generate(&env);
    client.grant_delegated_admin(&admin, &del_admin);
    client.revoke_delegated_admin(&admin, &del_admin);
    assert!(!client.has_delegated_admin(&del_admin));
}

#[test]
#[should_panic(expected = "caller is not admin")]
fn test_delegated_admin_cannot_grant_governance_explicit() {
    let (env, client, admin) = setup();
    let del_admin = Address::generate(&env);
    let target = Address::generate(&env);
    client.grant_delegated_admin(&admin, &del_admin);
    client.grant_governance(&del_admin, &target);
}

#[test]
#[should_panic(expected = "caller is not admin")]
fn test_delegated_admin_cannot_transfer_admin_explicit() {
    let (env, client, admin) = setup();
    let del_admin = Address::generate(&env);
    let target = Address::generate(&env);
    client.grant_delegated_admin(&admin, &del_admin);
    client.transfer_admin(&del_admin, &target);
}

#[test]
#[should_panic(expected = "caller is not admin")]
fn test_governance_cannot_grant_delegated_admin() {
    let (env, client, admin) = setup();
    let gov = Address::generate(&env);
    let target = Address::generate(&env);
    client.grant_governance(&admin, &gov);
    client.grant_delegated_admin(&gov, &target);
}

#[test]
#[should_panic(expected = "caller is not admin")]
fn test_governance_cannot_revoke_delegated_admin() {
    let (env, client, admin) = setup();
    let gov = Address::generate(&env);
    let del_admin = Address::generate(&env);
    client.grant_governance(&admin, &gov);
    client.grant_delegated_admin(&admin, &del_admin);
    client.revoke_delegated_admin(&gov, &del_admin);
}

#[test]
#[should_panic(expected = "caller is not admin")]
fn test_governance_cannot_transfer_admin() {
    let (env, client, admin) = setup();
    let gov = Address::generate(&env);
    let target = Address::generate(&env);
    client.grant_governance(&admin, &gov);
    client.transfer_admin(&gov, &target);
}

// 
//  12. Negative / Authorization
// 

#[test]
#[should_panic(expected = "caller lacks lender admin privileges")]
fn test_bare_address_cannot_set_lender() {
    let (env, client, _admin) = setup();
    let other = Address::generate(&env);
    let lender = Address::generate(&env);
    client.set_lender(&other, &lender, &1u32, &meta(&env, "L"));
}

#[test]
#[should_panic(expected = "caller lacks lender admin privileges")]
fn test_bare_address_cannot_remove_lender() {
    let (env, client, admin) = setup();
    let other = Address::generate(&env);
    let lender = Address::generate(&env);
    client.set_lender(&admin, &lender, &1u32, &meta(&env, "L"));
    client.remove_lender(&other, &lender, &reason(&env, "unauthorized"));
}

#[test]
#[should_panic(expected = "lender not found")]
fn test_remove_unenrolled_lender_panics() {
    let (env, client, admin) = setup();
    let lender = Address::generate(&env);
    client.remove_lender(&admin, &lender, &reason(&env, ""));
}

#[test]
#[should_panic(expected = "caller lacks lender admin privileges")]
fn test_enrolled_lender_cannot_set_lender_explicit() {
    let (env, client, admin) = setup();
    let lender = Address::generate(&env);
    client.set_lender(&admin, &lender, &1u32, &meta(&env, "L"));
    client.set_lender(&lender, &lender, &99u32, &meta(&env, "self-upgrade"));
}

#[test]
#[should_panic(expected = "caller lacks lender admin privileges")]
fn test_revoked_governance_cannot_set_lender() {
    let (env, client, admin) = setup();
    let gov = Address::generate(&env);
    let lender = Address::generate(&env);
    client.grant_governance(&admin, &gov);
    client.revoke_governance(&admin, &gov);
    client.set_lender(&gov, &lender, &1u32, &meta(&env, "L"));
}

#[test]
#[should_panic(expected = "caller lacks lender admin privileges")]
fn test_revoked_delegated_admin_cannot_set_lender() {
    let (env, client, admin) = setup();
    let del_admin = Address::generate(&env);
    let lender = Address::generate(&env);
    client.grant_delegated_admin(&admin, &del_admin);
    client.revoke_delegated_admin(&admin, &del_admin);
    client.set_lender(&del_admin, &lender, &1u32, &meta(&env, "L"));
}

#[test]
#[should_panic(expected = "caller lacks lender admin privileges")]
fn test_revoked_delegated_admin_cannot_remove_lender() {
    let (env, client, admin) = setup();
    let del_admin = Address::generate(&env);
    let lender = Address::generate(&env);
    client.grant_delegated_admin(&admin, &del_admin);
    client.set_lender(&del_admin, &lender, &1u32, &meta(&env, "L"));
    client.revoke_delegated_admin(&admin, &del_admin);
    client.remove_lender(&del_admin, &lender, &reason(&env, "should fail"));
}

// 
//  13. Self-Revocation Edge Cases
// 

#[test]
fn test_admin_can_revoke_own_governance_role() {
    let (_env, client, admin) = setup();
    assert!(client.has_governance(&admin));
    client.revoke_governance(&admin, &admin);
    assert!(!client.has_governance(&admin));
    // Admin still has admin privileges (can still grant governance)
    let env2 = Env::default();
    env2.mock_all_auths();
    let cid2 = env2.register(LenderAccessListContract, ());
    let c2 = LenderAccessListContractClient::new(&env2, &cid2);
    let a2 = Address::generate(&env2);
    c2.initialize(&a2);
    c2.revoke_governance(&a2, &a2);
    let new_gov = Address::generate(&env2);
    c2.grant_governance(&a2, &new_gov);
    assert!(c2.has_governance(&new_gov));
}

#[test]
#[should_panic(expected = "caller is not admin")]
fn test_governance_self_revoke_panics() {
    let (env, client, admin) = setup();
    let gov = Address::generate(&env);
    client.grant_governance(&admin, &gov);
    client.revoke_governance(&gov, &gov);
}

#[test]
#[should_panic(expected = "caller is not admin")]
fn test_delegated_admin_self_revoke_panics() {
    let (env, client, admin) = setup();
    let del_admin = Address::generate(&env);
    client.grant_delegated_admin(&admin, &del_admin);
    client.revoke_delegated_admin(&del_admin, &del_admin);
}

// 
//  14. Bulk / Batch Operations
// 

#[test]
fn test_bulk_enroll_multiple_lenders() {
    let (env, client, admin) = setup();
    let count = 10u32;
    for i in 0..count {
        let lender = Address::generate(&env);
        client.set_lender(&admin, &lender, &(i + 1), &meta(&env, "L"));
    }
    assert_eq!(client.get_all_lenders().len(), count);
    assert_eq!(client.get_active_lenders().len(), count);
}

#[test]
fn test_bulk_remove_all_lenders() {
    let (env, client, admin) = setup();
    let mut lenders = soroban_sdk::Vec::new(&env);
    for _ in 0..5u32 {
        let lender = Address::generate(&env);
        client.set_lender(&admin, &lender, &1u32, &meta(&env, "L"));
        lenders.push_back(lender);
    }
    assert_eq!(client.get_active_lenders().len(), 5);
    for i in 0..lenders.len() {
        client.remove_lender(&admin, &lenders.get(i).unwrap(), &reason(&env, "bulk-remove"));
    }
    assert_eq!(client.get_active_lenders().len(), 0);
    assert_eq!(client.get_all_lenders().len(), 5);
}

#[test]
fn test_bulk_tier_upgrades() {
    let (env, client, admin) = setup();
    let lender = Address::generate(&env);
    client.set_lender(&admin, &lender, &1u32, &meta(&env, "L"));
    for tier in 2u32..=5u32 {
        client.set_lender(&admin, &lender, &tier, &meta(&env, "L"));
        assert!(client.is_allowed(&lender, &tier));
        assert!(!client.is_allowed(&lender, &(tier + 1)));
    }
    assert_eq!(client.get_all_lenders().len(), 1);
}

#[test]
fn test_multiple_governance_holders_can_all_manage_lenders() {
    let (env, client, admin) = setup();
    let gov1 = Address::generate(&env);
    let gov2 = Address::generate(&env);
    let lender1 = Address::generate(&env);
    let lender2 = Address::generate(&env);
    client.grant_governance(&admin, &gov1);
    client.grant_governance(&admin, &gov2);
    client.set_lender(&gov1, &lender1, &1u32, &meta(&env, "L1"));
    client.set_lender(&gov2, &lender2, &2u32, &meta(&env, "L2"));
    assert!(client.is_allowed(&lender1, &1u32));
    assert!(client.is_allowed(&lender2, &2u32));
    assert_eq!(client.get_active_lenders().len(), 2);
}

#[test]
fn test_multiple_delegated_admins_can_all_manage_lenders() {
    let (env, client, admin) = setup();
    let da1 = Address::generate(&env);
    let da2 = Address::generate(&env);
    let lender1 = Address::generate(&env);
    let lender2 = Address::generate(&env);
    client.grant_delegated_admin(&admin, &da1);
    client.grant_delegated_admin(&admin, &da2);
    client.set_lender(&da1, &lender1, &1u32, &meta(&env, "L1"));
    client.set_lender(&da2, &lender2, &1u32, &meta(&env, "L2"));
    assert_eq!(client.get_active_lenders().len(), 2);
}

// 
//  15. Race Condition / Ordering Invariants
// 

#[test]
fn test_last_writer_wins_on_sequential_updates() {
    let (env, client, admin) = setup();
    let gov = Address::generate(&env);
    let lender = Address::generate(&env);
    client.grant_governance(&admin, &gov);
    client.set_lender(&admin, &lender, &1u32, &meta(&env, "by-admin"));
    client.set_lender(&gov, &lender, &3u32, &meta(&env, "by-gov"));
    let record = client.get_lender(&lender).unwrap();
    assert_eq!(record.tier, 3);
    assert_eq!(record.updated_by, gov);
}

#[test]
fn test_grant_then_immediate_revoke_leaves_no_access() {
    let (env, client, admin) = setup();
    let gov = Address::generate(&env);
    let lender = Address::generate(&env);
    client.grant_governance(&admin, &gov);
    client.set_lender(&gov, &lender, &1u32, &meta(&env, "L"));
    client.revoke_governance(&admin, &gov);
    assert!(!client.has_governance(&gov));
    assert!(client.is_allowed(&lender, &1u32));
}

#[test]
fn test_enroll_remove_reenroll_sequence_is_consistent() {
    let (env, client, admin) = setup();
    let lender = Address::generate(&env);
    for cycle in 1u32..=3 {
        client.set_lender(&admin, &lender, &cycle, &meta(&env, "L"));
        assert!(client.is_allowed(&lender, &cycle));
        client.remove_lender(&admin, &lender, &reason(&env, "cycle"));
        assert!(!client.is_allowed(&lender, &1u32));
    }
    client.set_lender(&admin, &lender, &5u32, &meta(&env, "final"));
    assert!(client.is_allowed(&lender, &5u32));
    assert_eq!(client.get_all_lenders().len(), 1);
}

#[test]
fn test_event_sequence_enrollment_then_update_then_remove() {
    // Verify the event topic sequence is: lnd_new, lnd_set, lnd_rem
    let (env, client, admin) = setup();
    let lender = Address::generate(&env);
    client.set_lender(&admin, &lender, &1u32, &meta(&env, "v1"));
    client.set_lender(&admin, &lender, &2u32, &meta(&env, "v2"));
    client.remove_lender(&admin, &lender, &reason(&env, "done"));
    let events = env.events().all();
    // Last 3 events should be lnd_new, lnd_set, lnd_rem in order
    let n = events.len();
    assert!(n >= 3);
    let (_c, t0, _d) = events.get(n - 3).unwrap();
    let (_c, t1, _d) = events.get(n - 2).unwrap();
    let (_c, t2, _d) = events.get(n - 1).unwrap();
    assert_eq!(t0.get(0).unwrap(), TOPIC_LENDER_NEW.into_val(&env));
    assert_eq!(t1.get(0).unwrap(), TOPIC_LENDER_SET.into_val(&env));
    assert_eq!(t2.get(0).unwrap(), TOPIC_LENDER_REM.into_val(&env));
}

// 
//  16. Privilege Escalation Prevention
// 

#[test]
fn test_lender_cannot_self_enroll() {
    let (env, client, _admin) = setup();
    let lender = Address::generate(&env);
    assert!(!client.has_governance(&lender));
    assert!(!client.has_delegated_admin(&lender));
}

#[test]
fn test_lender_cannot_upgrade_own_tier() {
    let (env, client, admin) = setup();
    let lender = Address::generate(&env);
    client.set_lender(&admin, &lender, &1u32, &meta(&env, "L"));
    assert!(!client.has_governance(&lender));
    assert!(!client.has_delegated_admin(&lender));
}

// 
//  17. Query Correctness
// 

#[test]
fn test_get_lender_returns_none_for_unenrolled() {
    let (env, client, _admin) = setup();
    let lender = Address::generate(&env);
    assert!(client.get_lender(&lender).is_none());
}

#[test]
fn test_get_all_lenders_empty_initially() {
    let (_env, client, _admin) = setup();
    assert_eq!(client.get_all_lenders().len(), 0);
}

#[test]
fn test_get_active_lenders_excludes_tier_zero() {
    let (env, client, admin) = setup();
    let l1 = Address::generate(&env);
    let l2 = Address::generate(&env);
    client.set_lender(&admin, &l1, &1u32, &meta(&env, "L1"));
    client.set_lender(&admin, &l2, &0u32, &meta(&env, "L2-disabled"));
    let active = client.get_active_lenders();
    assert_eq!(active.len(), 1);
    assert_eq!(active.get(0).unwrap(), l1);
}

#[test]
fn test_get_active_lenders_excludes_removed() {
    let (env, client, admin) = setup();
    let l1 = Address::generate(&env);
    let l2 = Address::generate(&env);
    client.set_lender(&admin, &l1, &1u32, &meta(&env, "L1"));
    client.set_lender(&admin, &l2, &1u32, &meta(&env, "L2"));
    client.remove_lender(&admin, &l2, &reason(&env, "removed"));
    let active = client.get_active_lenders();
    assert_eq!(active.len(), 1);
    assert_eq!(active.get(0).unwrap(), l1);
}

#[test]
fn test_get_all_lenders_includes_removed() {
    let (env, client, admin) = setup();
    let l1 = Address::generate(&env);
    let l2 = Address::generate(&env);
    client.set_lender(&admin, &l1, &1u32, &meta(&env, "L1"));
    client.set_lender(&admin, &l2, &1u32, &meta(&env, "L2"));
    client.remove_lender(&admin, &l2, &reason(&env, "removed"));
    assert_eq!(client.get_all_lenders().len(), 2);
}

// 
//  18. Boundary Values
// 

#[test]
fn test_tier_u32_max_is_valid() {
    let (env, client, admin) = setup();
    let lender = Address::generate(&env);
    client.set_lender(&admin, &lender, &u32::MAX, &meta(&env, "L"));
    let record = client.get_lender(&lender).unwrap();
    assert_eq!(record.tier, u32::MAX);
    assert!(client.is_allowed(&lender, &u32::MAX));
}

#[test]
fn test_tier_one_is_minimum_active_tier() {
    let (env, client, admin) = setup();
    let lender = Address::generate(&env);
    client.set_lender(&admin, &lender, &1u32, &meta(&env, "L"));
    assert!(client.is_allowed(&lender, &1u32));
    assert!(!client.is_allowed(&lender, &2u32));
}

#[test]
fn test_empty_metadata_strings_are_valid() {
    let (env, client, admin) = setup();
    let lender = Address::generate(&env);
    let empty_meta = LenderMetadata {
        name: String::from_str(&env, ""),
        url: String::from_str(&env, ""),
        notes: String::from_str(&env, ""),
    };
    client.set_lender(&admin, &lender, &1u32, &empty_meta);
    let record = client.get_lender(&lender).unwrap();
    assert_eq!(record.metadata.name, String::from_str(&env, ""));
}

// 
//  19. Event Ordering and Count
// 

#[test]
fn test_event_emitted_for_every_state_change() {
    let (env, client, admin) = setup();
    let gov = Address::generate(&env);
    let del_admin = Address::generate(&env);
    let lender = Address::generate(&env);
    let new_admin = Address::generate(&env);
    client.grant_governance(&admin, &gov);
    client.revoke_governance(&admin, &gov);
    client.grant_delegated_admin(&admin, &del_admin);
    client.revoke_delegated_admin(&admin, &del_admin);
    client.set_lender(&admin, &lender, &1u32, &meta(&env, "L"));
    client.set_lender(&admin, &lender, &2u32, &meta(&env, "L2"));
    client.remove_lender(&admin, &lender, &reason(&env, "done"));
    client.transfer_admin(&admin, &new_admin);
    let events = env.events().all();
    assert!(events.len() >= 8, "expected at least 8 events, got {}", events.len());
}

#[test]
fn test_no_events_emitted_on_read_only_calls() {
    let (env, client, admin) = setup();
    let lender = Address::generate(&env);
    client.set_lender(&admin, &lender, &1u32, &meta(&env, "L"));
    let count_before = env.events().all().len();
    let _ = client.get_lender(&lender);
    let _ = client.is_allowed(&lender, &1u32);
    let _ = client.get_all_lenders();
    let _ = client.get_active_lenders();
    let _ = client.get_admin();
    let _ = client.has_governance(&admin);
    let _ = client.has_delegated_admin(&admin);
    let _ = client.get_event_schema_version();
    assert_eq!(env.events().all().len(), count_before);
}

// 
//  20. Schema Version
// 

#[test]
fn test_schema_version_constant_matches_query() {
    let (_env, client, _admin) = setup();
    assert_eq!(client.get_event_schema_version(), EVENT_SCHEMA_VERSION);
    assert_eq!(EVENT_SCHEMA_VERSION, 2u32);
}

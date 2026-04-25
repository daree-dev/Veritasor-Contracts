//! Tests for period parsing and maturity window enforcement.
//!
//! These tests exercise the `parse_period` and `is_period_within_maturity`
//! helpers directly (unit level) as well as the `redeem` entry-point
//! (integration level) to confirm the maturity gate is enforced end-to-end.

#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, token, Address, Env, String};

// ── helpers ──────────────────────────────────────────────────────────────────

fn create_token_contract<'a>(env: &Env, admin: &Address) -> token::StellarAssetClient<'a> {
    token::StellarAssetClient::new(
        env,
        &env.register_stellar_asset_contract_v2(admin.clone())
            .address(),
    )
}

fn setup_test() -> (Env, Address, Address, Address, Address, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    env.mock_all_auths_allowing_non_root_auth();

    let admin = Address::generate(&env);
    let issuer = Address::generate(&env);
    let owner = Address::generate(&env);
    let token_admin = Address::generate(&env);

    let token_client = create_token_contract(&env, &token_admin);
    let token = token_client.address.clone();
    token_client.mint(&issuer, &100_000_000);

    let attestation_contract = Address::generate(&env);

    (env, admin, issuer, owner, token, attestation_contract, token_admin)
}

// ── unit: parse_period ────────────────────────────────────────────────────────

#[test]
fn test_parse_period_valid() {
    let env = Env::default();
    let p = String::from_str(&env, "2026-02");
    let months = parse_period(&env, p);
    // 2026 * 12 + (02 - 1) = 24312 + 1 = 24313
    assert_eq!(months, 2026u64 * 12 + 1);
}

#[test]
fn test_parse_period_january() {
    let env = Env::default();
    let p = String::from_str(&env, "2026-01");
    let months = parse_period(&env, p);
    assert_eq!(months, 2026u64 * 12);
}

#[test]
fn test_parse_period_december() {
    let env = Env::default();
    let p = String::from_str(&env, "2026-12");
    let months = parse_period(&env, p);
    assert_eq!(months, 2026u64 * 12 + 11);
}

#[test]
#[should_panic(expected = "invalid period length")]
fn test_parse_period_invalid_length() {
    let env = Env::default();
    let p = String::from_str(&env, "2026-2");
    parse_period(&env, p);
}

#[test]
#[should_panic(expected = "invalid year digit")]
fn test_parse_period_invalid_digit() {
    let env = Env::default();
    let p = String::from_str(&env, "202a-02");
    parse_period(&env, p);
}

#[test]
#[should_panic(expected = "invalid month")]
fn test_parse_period_month_zero() {
    let env = Env::default();
    let p = String::from_str(&env, "2026-00");
    parse_period(&env, p);
}

#[test]
#[should_panic(expected = "invalid month")]
fn test_parse_period_month_thirteen() {
    let env = Env::default();
    let p = String::from_str(&env, "2026-13");
    parse_period(&env, p);
}

// ── unit: is_period_within_maturity ──────────────────────────────────────────

fn make_bond(env: &Env, issue_period: &str, maturity_periods: u32) -> Bond {
    let issuer = Address::generate(env);
    let attestation_contract = Address::generate(env);
    let token = Address::generate(env);
    Bond {
        id: 0,
        issuer,
        face_value: 1_000_000,
        structure: BondStructure::Fixed,
        revenue_share_bps: 0,
        min_payment_per_period: 100_000,
        max_payment_per_period: 100_000,
        maturity_periods,
        attestation_contract,
        token,
        status: BondStatus::Active,
        issued_at: 0,
        issue_period: String::from_str(env, issue_period),
    }
}

#[test]
fn test_is_within_maturity_first_period() {
    let env = Env::default();
    let bond = make_bond(&env, "2026-01", 12);
    // issue period itself is within maturity
    assert!(is_period_within_maturity(&env, &bond, String::from_str(&env, "2026-01")));
}

#[test]
fn test_is_within_maturity_last_valid_period() {
    let env = Env::default();
    let bond = make_bond(&env, "2026-01", 12);
    // 12 periods: 2026-01 through 2026-12 (indices 0..11)
    assert!(is_period_within_maturity(&env, &bond, String::from_str(&env, "2026-12")));
}

#[test]
fn test_is_within_maturity_expired() {
    let env = Env::default();
    let bond = make_bond(&env, "2026-01", 12);
    // 2027-01 is period index 12 — one past the window
    assert!(!is_period_within_maturity(&env, &bond, String::from_str(&env, "2027-01")));
}

#[test]
fn test_is_within_maturity_before_issue() {
    let env = Env::default();
    let bond = make_bond(&env, "2026-03", 12);
    // period before issue_period is outside the window
    assert!(!is_period_within_maturity(&env, &bond, String::from_str(&env, "2026-01")));
}

#[test]
fn test_is_within_maturity_single_period_bond() {
    let env = Env::default();
    let bond = make_bond(&env, "2026-06", 1);
    assert!(is_period_within_maturity(&env, &bond, String::from_str(&env, "2026-06")));
    assert!(!is_period_within_maturity(&env, &bond, String::from_str(&env, "2026-07")));
}

// ── integration: redeem maturity gate ────────────────────────────────────────

#[test]
fn test_redeem_within_maturity() {
    let (env, admin, issuer, owner, token, attestation_contract, _) = setup_test();
    let contract_id = env.register(RevenueBondContract, ());
    let client = RevenueBondContractClient::new(&env, &contract_id);
    client.initialize(&admin);

    let issue_period = String::from_str(&env, "2026-01");
    let bond_id = client.issue_bond(
        &issuer,
        &owner,
        &1_000_000,
        &BondStructure::Fixed,
        &0,
        &100_000,
        &100_000,
        &12,
        &issue_period,
        &attestation_contract,
        &token,
    );

    // 2026-02 is within the 12-period window starting 2026-01
    let period = String::from_str(&env, "2026-02");
    client.redeem(&bond_id, &period, &500_000);

    let rec = client.get_redemption(&bond_id, &period).unwrap();
    assert_eq!(rec.redemption_amount, 100_000);
}

#[test]
#[should_panic(expected = "period exceeds maturity")]
fn test_redeem_post_maturity_panics() {
    let (env, admin, issuer, owner, token, attestation_contract, _) = setup_test();
    let contract_id = env.register(RevenueBondContract, ());
    let client = RevenueBondContractClient::new(&env, &contract_id);
    client.initialize(&admin);

    let issue_period = String::from_str(&env, "2026-01");
    let bond_id = client.issue_bond(
        &issuer,
        &owner,
        &1_000_000,
        &BondStructure::Fixed,
        &0,
        &100_000,
        &100_000,
        &1, // only 2026-01 is valid
        &issue_period,
        &attestation_contract,
        &token,
    );

    // 2026-02 is one period past the single-period window
    let expired_period = String::from_str(&env, "2026-02");
    client.redeem(&bond_id, &expired_period, &500_000);
}

#[test]
#[should_panic(expected = "bond not active")]
fn test_redeem_matured_bond_panics() {
    let (env, admin, issuer, owner, token, attestation_contract, _) = setup_test();
    let contract_id = env.register(RevenueBondContract, ());
    let client = RevenueBondContractClient::new(&env, &contract_id);
    client.initialize(&admin);

    let issue_period = String::from_str(&env, "2026-01");
    let bond_id = client.issue_bond(
        &issuer,
        &owner,
        &1_000_000,
        &BondStructure::Fixed,
        &0,
        &100_000,
        &100_000,
        &12,
        &issue_period,
        &attestation_contract,
        &token,
    );

    client.mark_matured(&admin, &bond_id);

    let period = String::from_str(&env, "2026-02");
    client.redeem(&bond_id, &period, &500_000);
}

#[test]
fn test_remaining_value_matured_is_zero() {
    let (env, admin, issuer, owner, token, attestation_contract, _) = setup_test();
    let contract_id = env.register(RevenueBondContract, ());
    let client = RevenueBondContractClient::new(&env, &contract_id);
    client.initialize(&admin);

    let issue_period = String::from_str(&env, "2026-01");
    let bond_id = client.issue_bond(
        &issuer,
        &owner,
        &1_000_000,
        &BondStructure::Fixed,
        &0,
        &100_000,
        &100_000,
        &12,
        &issue_period,
        &attestation_contract,
        &token,
    );

    client.mark_matured(&admin, &bond_id);
    assert_eq!(client.get_remaining_value(&bond_id), 0);
}

#[test]
fn test_redeem_last_valid_period_in_window() {
    let (env, admin, issuer, owner, token, attestation_contract, _) = setup_test();
    let contract_id = env.register(RevenueBondContract, ());
    let client = RevenueBondContractClient::new(&env, &contract_id);
    client.initialize(&admin);

    let issue_period = String::from_str(&env, "2026-01");
    let bond_id = client.issue_bond(
        &issuer,
        &owner,
        &1_000_000,
        &BondStructure::Fixed,
        &0,
        &100_000,
        &100_000,
        &12,
        &issue_period,
        &attestation_contract,
        &token,
    );

    // 2026-12 is the last period in a 12-period window starting 2026-01
    let last_period = String::from_str(&env, "2026-12");
    client.redeem(&bond_id, &last_period, &500_000);

    let rec = client.get_redemption(&bond_id, &last_period).unwrap();
    assert_eq!(rec.redemption_amount, 100_000);
}

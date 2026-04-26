#![cfg(test)]
use super::*;
use soroban_sdk::{testutils::Address as _, token, Address, Env, String};

fn create_token<'a>(env: &Env, admin: &Address) -> token::StellarAssetClient<'a> {
    token::StellarAssetClient::new(
        env,
        &env.register_stellar_asset_contract_v2(admin.clone()).address(),
    )
}

fn setup() -> (Env, Address, Address, Address, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    env.mock_all_auths_allowing_non_root_auth();
    let admin = Address::generate(&env);
    let issuer = Address::generate(&env);
    let owner = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let tc = create_token(&env, &token_admin);
    tc.mint(&issuer, &500_000_000);
    let attestation = Address::generate(&env);
    (env, admin, issuer, owner, tc.address.clone(), attestation)
}

fn issue_hybrid(
    client: &RevenueBondContractClient,
    env: &Env,
    issuer: &Address,
    owner: &Address,
    face_value: i128,
    revenue_share_bps: u32,
    min_payment: i128,
    max_payment: i128,
    maturity: u32,
    attestation: &Address,
    token: &Address,
) -> u64 {
    let ip = String::from_str(env, "2026-01");
    client.issue_bond(
        issuer, owner, &face_value,
        &BondStructure::Hybrid,
        &revenue_share_bps,
        &min_payment, &max_payment,
        &maturity, &ip, attestation, token,
    )
}

//  Hybrid: revenue spike 

/// Revenue spike: hybrid bond caps at max_payment even when revenue component
/// alone would exceed it. Invariant: min_fixed + revenue_share <= max_payment.
#[test]
fn test_hybrid_revenue_spike_capped_at_max() {
    let (env, admin, issuer, owner, token, attestation) = setup();
    let c = env.register(RevenueBondContract, ());
    let client = RevenueBondContractClient::new(&env, &c);
    client.initialize(&admin);
    // min=200_000, share=10% (1000 bps), max=800_000
    // revenue=10_000_000 => component=1_000_000, total=1_200_000 => capped at 800_000
    let id = issue_hybrid(&client, &env, &issuer, &owner,
        10_000_000, 1000, 200_000, 800_000, 24, &attestation, &token);
    let p = String::from_str(&env, "2026-02");
    client.redeem(&id, &p, &10_000_000);
    let rec = client.get_redemption(&id, &p).unwrap();
    assert_eq!(rec.redemption_amount, 800_000);
}

/// Revenue spike across multiple periods: each period is independently capped.
/// Total redeemed must never exceed face_value.
#[test]
fn test_hybrid_spike_multi_period_face_value_cap() {
    let (env, admin, issuer, owner, token, attestation) = setup();
    let c = env.register(RevenueBondContract, ());
    let client = RevenueBondContractClient::new(&env, &c);
    client.initialize(&admin);
    // face=1_500_000, max=800_000 per period
    let id = issue_hybrid(&client, &env, &issuer, &owner,
        1_500_000, 1000, 200_000, 800_000, 24, &attestation, &token);
    let p1 = String::from_str(&env, "2026-02");
    let p2 = String::from_str(&env, "2026-03");
    client.redeem(&id, &p1, &10_000_000); // capped at 800_000
    client.redeem(&id, &p2, &10_000_000); // remaining=700_000, capped there
    let bond = client.get_bond(&id).unwrap();
    assert_eq!(bond.status, BondStatus::FullyRedeemed);
    assert_eq!(client.get_total_redeemed(&id), 1_500_000);
    let r2 = client.get_redemption(&id, &p2).unwrap();
    assert_eq!(r2.redemption_amount, 700_000);
}

/// Extreme spike: revenue = i128::MAX / 10000 to exercise saturating arithmetic.
/// Must not panic; must cap at max_payment.
#[test]
fn test_hybrid_extreme_revenue_saturating_arithmetic() {
    let (env, admin, issuer, owner, token, attestation) = setup();
    let c = env.register(RevenueBondContract, ());
    let client = RevenueBondContractClient::new(&env, &c);
    client.initialize(&admin);
    let id = issue_hybrid(&client, &env, &issuer, &owner,
        5_000_000, 500, 100_000, 1_000_000, 24, &attestation, &token);
    // i128::MAX as revenue  saturating_mul must not overflow
    let p = String::from_str(&env, "2026-02");
    client.redeem(&id, &p, &i128::MAX);
    let rec = client.get_redemption(&id, &p).unwrap();
    assert_eq!(rec.redemption_amount, 1_000_000);
}

//  Hybrid: revenue collapse 

/// Revenue collapse to zero: hybrid bond must still pay min_payment (the fixed
/// floor). This is the core double-payment invariant  min_fixed is paid once,
/// not once as floor AND once as revenue share.
#[test]
fn test_hybrid_zero_revenue_pays_min_only() {
    let (env, admin, issuer, owner, token, attestation) = setup();
    let c = env.register(RevenueBondContract, ());
    let client = RevenueBondContractClient::new(&env, &c);
    client.initialize(&admin);
    // min=200_000, share=10%, max=800_000
    // revenue=0 => component=0, total=200_000 (min floor)
    let id = issue_hybrid(&client, &env, &issuer, &owner,
        5_000_000, 1000, 200_000, 800_000, 24, &attestation, &token);
    let p = String::from_str(&env, "2026-02");
    client.redeem(&id, &p, &0);
    let rec = client.get_redemption(&id, &p).unwrap();
    // min_payment_per_period + 0 revenue_component = 200_000
    assert_eq!(rec.redemption_amount, 200_000);
    assert_eq!(client.get_total_redeemed(&id), 200_000);
}

/// Revenue collapse: min_payment is NOT double-counted. The formula is
/// min_payment + revenue_component, not max(min, share) + min.
/// With revenue=0 across many periods, total = periods * min_payment.
#[test]
fn test_hybrid_min_fixed_not_double_paid_across_periods() {
    let (env, admin, issuer, owner, token, attestation) = setup();
    let c = env.register(RevenueBondContract, ());
    let client = RevenueBondContractClient::new(&env, &c);
    client.initialize(&admin);
    // face=600_000, min=200_000, share=10%, max=800_000
    // 3 zero-revenue periods => 3 * 200_000 = 600_000 = face_value
    let id = issue_hybrid(&client, &env, &issuer, &owner,
        600_000, 1000, 200_000, 800_000, 24, &attestation, &token);
    let p1 = String::from_str(&env, "2026-02");
    let p2 = String::from_str(&env, "2026-03");
    let p3 = String::from_str(&env, "2026-04");
    client.redeem(&id, &p1, &0);
    client.redeem(&id, &p2, &0);
    client.redeem(&id, &p3, &0);
    assert_eq!(client.get_total_redeemed(&id), 600_000);
    let bond = client.get_bond(&id).unwrap();
    assert_eq!(bond.status, BondStatus::FullyRedeemed);
}

/// Revenue collapse mid-stream: some high-revenue periods followed by collapse.
/// Verifies face_value cap still applies and no double-payment occurs.
#[test]
fn test_hybrid_collapse_after_spike_caps_correctly() {
    let (env, admin, issuer, owner, token, attestation) = setup();
    let c = env.register(RevenueBondContract, ());
    let client = RevenueBondContractClient::new(&env, &c);
    client.initialize(&admin);
    // face=1_000_000, min=100_000, share=5% (500 bps), max=600_000
    let id = issue_hybrid(&client, &env, &issuer, &owner,
        1_000_000, 500, 100_000, 600_000, 24, &attestation, &token);
    // Period 1: revenue=8_000_000 => 100_000 + 400_000 = 500_000
    let p1 = String::from_str(&env, "2026-02");
    client.redeem(&id, &p1, &8_000_000);
    assert_eq!(client.get_total_redeemed(&id), 500_000);
    // Period 2: revenue=0 => 100_000 + 0 = 100_000
    let p2 = String::from_str(&env, "2026-03");
    client.redeem(&id, &p2, &0);
    assert_eq!(client.get_total_redeemed(&id), 600_000);
    // Period 3: revenue=0 => remaining=400_000, pays 100_000
    let p3 = String::from_str(&env, "2026-04");
    client.redeem(&id, &p3, &0);
    assert_eq!(client.get_total_redeemed(&id), 700_000);
    // Bond still active  not yet fully redeemed
    let bond = client.get_bond(&id).unwrap();
    assert_eq!(bond.status, BondStatus::Active);
    assert_eq!(client.get_remaining_value(&id), 300_000);
}

/// Minimum payment boundary: revenue exactly at the threshold where
/// revenue_component == min_payment. Total must be min + component, not 2*min.
#[test]
fn test_hybrid_revenue_component_equals_min_no_double_count() {
    let (env, admin, issuer, owner, token, attestation) = setup();
    let c = env.register(RevenueBondContract, ());
    let client = RevenueBondContractClient::new(&env, &c);
    client.initialize(&admin);
    // min=200_000, share=10% (1000 bps), max=800_000
    // revenue=2_000_000 => component=200_000, total=400_000 (not 600_000)
    let id = issue_hybrid(&client, &env, &issuer, &owner,
        5_000_000, 1000, 200_000, 800_000, 24, &attestation, &token);
    let p = String::from_str(&env, "2026-02");
    client.redeem(&id, &p, &2_000_000);
    let rec = client.get_redemption(&id, &p).unwrap();
    assert_eq!(rec.redemption_amount, 400_000); // 200_000 + 200_000
}

//  Hybrid: double-spend prevention 

/// Double-spend: same period cannot be redeemed twice on a hybrid bond,
/// regardless of revenue amount.
#[test]
#[should_panic(expected = "already redeemed for period")]
fn test_hybrid_double_spend_same_period_panics() {
    let (env, admin, issuer, owner, token, attestation) = setup();
    let c = env.register(RevenueBondContract, ());
    let client = RevenueBondContractClient::new(&env, &c);
    client.initialize(&admin);
    let id = issue_hybrid(&client, &env, &issuer, &owner,
        5_000_000, 1000, 200_000, 800_000, 24, &attestation, &token);
    let p = String::from_str(&env, "2026-02");
    client.redeem(&id, &p, &3_000_000);
    // Second call with different revenue must still panic
    client.redeem(&id, &p, &0);
}

/// Double-spend attempt with zero revenue after a spike: the lock is on the
/// period key, not the revenue amount. Must panic.
#[test]
#[should_panic(expected = "already redeemed for period")]
fn test_hybrid_double_spend_zero_after_spike_panics() {
    let (env, admin, issuer, owner, token, attestation) = setup();
    let c = env.register(RevenueBondContract, ());
    let client = RevenueBondContractClient::new(&env, &c);
    client.initialize(&admin);
    let id = issue_hybrid(&client, &env, &issuer, &owner,
        5_000_000, 1000, 200_000, 800_000, 24, &attestation, &token);
    let p = String::from_str(&env, "2026-03");
    client.redeem(&id, &p, &10_000_000);
    client.redeem(&id, &p, &0);
}

//  Hybrid: attestation lag / missing attestation 

/// Attestation not found: redeem must panic. The mock returns Some for all
/// calls, so we verify the happy path here and document the guard location.
/// In production the attestation contract would return None for a missing entry.
#[test]
fn test_hybrid_attestation_present_succeeds() {
    let (env, admin, issuer, owner, token, attestation) = setup();
    let c = env.register(RevenueBondContract, ());
    let client = RevenueBondContractClient::new(&env, &c);
    client.initialize(&admin);
    let id = issue_hybrid(&client, &env, &issuer, &owner,
        5_000_000, 1000, 200_000, 800_000, 24, &attestation, &token);
    let p = String::from_str(&env, "2026-02");
    // Mock always returns Some + not revoked, so this must succeed
    client.redeem(&id, &p, &1_000_000);
    let rec = client.get_redemption(&id, &p).unwrap();
    // 200_000 + 100_000 = 300_000
    assert_eq!(rec.redemption_amount, 300_000);
}

//  Hybrid: ownership transfer during volatile redemption 

/// Ownership transfer between two volatile periods: payment for each period
/// goes to whoever owns the bond at redemption time.
#[test]
fn test_hybrid_payment_routes_to_current_owner_after_transfer() {
    let (env, admin, issuer, owner, token, attestation) = setup();
    let c = env.register(RevenueBondContract, ());
    let client = RevenueBondContractClient::new(&env, &c);
    client.initialize(&admin);
    let id = issue_hybrid(&client, &env, &issuer, &owner,
        5_000_000, 1000, 200_000, 800_000, 24, &attestation, &token);

    // Period 1: owner redeems during spike
    let p1 = String::from_str(&env, "2026-02");
    client.redeem(&id, &p1, &5_000_000);
    assert_eq!(client.get_owner(&id).unwrap(), owner);

    // Transfer ownership
    let new_owner = Address::generate(&env);
    client.transfer_ownership(&id, &owner, &new_owner);
    assert_eq!(client.get_owner(&id).unwrap(), new_owner);

    // Period 2: new_owner redeems during collapse
    let p2 = String::from_str(&env, "2026-03");
    client.redeem(&id, &p2, &0);
    let r2 = client.get_redemption(&id, &p2).unwrap();
    assert_eq!(r2.redemption_amount, 200_000);
}

//  Hybrid: face value boundary precision 

/// Last redemption exactly exhausts face value: bond transitions to FullyRedeemed
/// and remaining_value returns 0.
#[test]
fn test_hybrid_exact_face_value_exhaustion() {
    let (env, admin, issuer, owner, token, attestation) = setup();
    let c = env.register(RevenueBondContract, ());
    let client = RevenueBondContractClient::new(&env, &c);
    client.initialize(&admin);
    // face=500_000, min=200_000, share=10%, max=500_000
    // Period 1: revenue=3_000_000 => 200_000+300_000=500_000 = face_value
    let id = issue_hybrid(&client, &env, &issuer, &owner,
        500_000, 1000, 200_000, 500_000, 24, &attestation, &token);
    let p = String::from_str(&env, "2026-02");
    client.redeem(&id, &p, &3_000_000);
    let bond = client.get_bond(&id).unwrap();
    assert_eq!(bond.status, BondStatus::FullyRedeemed);
    assert_eq!(client.get_total_redeemed(&id), 500_000);
    assert_eq!(client.get_remaining_value(&id), 0);
}

/// Redemption on a fully-redeemed hybrid bond must panic.
#[test]
#[should_panic(expected = "bond not active")]
fn test_hybrid_redeem_after_fully_redeemed_panics() {
    let (env, admin, issuer, owner, token, attestation) = setup();
    let c = env.register(RevenueBondContract, ());
    let client = RevenueBondContractClient::new(&env, &c);
    client.initialize(&admin);
    let id = issue_hybrid(&client, &env, &issuer, &owner,
        300_000, 1000, 200_000, 500_000, 24, &attestation, &token);
    let p1 = String::from_str(&env, "2026-02");
    let p2 = String::from_str(&env, "2026-03");
    client.redeem(&id, &p1, &0); // 200_000
    client.redeem(&id, &p2, &0); // 100_000 remaining => FullyRedeemed
    // Third call must panic
    let p3 = String::from_str(&env, "2026-04");
    client.redeem(&id, &p3, &0);
}

//  Hybrid: issuance validation 

/// Hybrid bond with revenue_share_bps=0 behaves like a fixed bond:
/// every period pays exactly min_payment regardless of revenue.
#[test]
fn test_hybrid_zero_share_bps_equals_fixed_behavior() {
    let (env, admin, issuer, owner, token, attestation) = setup();
    let c = env.register(RevenueBondContract, ());
    let client = RevenueBondContractClient::new(&env, &c);
    client.initialize(&admin);
    let ip = String::from_str(&env, "2026-01");
    let id = client.issue_bond(
        &issuer, &owner, &2_000_000,
        &BondStructure::Hybrid, &0,
        &300_000, &300_000, &12, &ip, &attestation, &token,
    );
    let p = String::from_str(&env, "2026-02");
    client.redeem(&id, &p, &50_000_000);
    let rec = client.get_redemption(&id, &p).unwrap();
    assert_eq!(rec.redemption_amount, 300_000);
}

/// Hybrid bond with revenue_share_bps=10000 (100%): total = min + full_revenue,
/// capped at max. Verifies the formula handles the maximum share correctly.
#[test]
fn test_hybrid_max_share_bps_formula() {
    let (env, admin, issuer, owner, token, attestation) = setup();
    let c = env.register(RevenueBondContract, ());
    let client = RevenueBondContractClient::new(&env, &c);
    client.initialize(&admin);
    // min=100_000, share=100% (10000 bps), max=2_000_000
    // revenue=500_000 => component=500_000, total=600_000
    let ip = String::from_str(&env, "2026-01");
    let id = client.issue_bond(
        &issuer, &owner, &10_000_000,
        &BondStructure::Hybrid, &10000,
        &100_000, &2_000_000, &24, &ip, &attestation, &token,
    );
    let p = String::from_str(&env, "2026-02");
    client.redeem(&id, &p, &500_000);
    let rec = client.get_redemption(&id, &p).unwrap();
    assert_eq!(rec.redemption_amount, 600_000); // 100_000 + 500_000
}

/// Issuer and owner must differ  hybrid bond issuance with same address panics.
#[test]
#[should_panic(expected = "issuer and owner must differ")]
fn test_hybrid_issuer_equals_owner_panics() {
    let (env, admin, issuer, _, token, attestation) = setup();
    let c = env.register(RevenueBondContract, ());
    let client = RevenueBondContractClient::new(&env, &c);
    client.initialize(&admin);
    let ip = String::from_str(&env, "2026-01");
    // Pass issuer as both issuer and owner
    client.issue_bond(
        &issuer, &issuer, &1_000_000,
        &BondStructure::Hybrid, &500,
        &100_000, &500_000, &12, &ip, &attestation, &token,
    );
}

/// max_payment < min_payment is rejected at issuance for hybrid bonds.
#[test]
#[should_panic(expected = "max must be >= min")]
fn test_hybrid_invalid_payment_range_panics() {
    let (env, admin, issuer, owner, token, attestation) = setup();
    let c = env.register(RevenueBondContract, ());
    let client = RevenueBondContractClient::new(&env, &c);
    client.initialize(&admin);
    let ip = String::from_str(&env, "2026-01");
    client.issue_bond(
        &issuer, &owner, &1_000_000,
        &BondStructure::Hybrid, &500,
        &500_000, &100_000, &12, &ip, &attestation, &token,
    );
}

//  Hybrid: negative revenue guard 

/// Negative attested_revenue must be rejected  the contract asserts >= 0.
#[test]
#[should_panic(expected = "attested_revenue must be non-negative")]
fn test_hybrid_negative_revenue_panics() {
    let (env, admin, issuer, owner, token, attestation) = setup();
    let c = env.register(RevenueBondContract, ());
    let client = RevenueBondContractClient::new(&env, &c);
    client.initialize(&admin);
    let id = issue_hybrid(&client, &env, &issuer, &owner,
        5_000_000, 1000, 200_000, 800_000, 24, &attestation, &token);
    let p = String::from_str(&env, "2026-02");
    client.redeem(&id, &p, &-1);
}

//  Hybrid: defaulted bond 

/// Hybrid bond marked defaulted cannot be redeemed even during a revenue spike.
#[test]
#[should_panic(expected = "bond not active")]
fn test_hybrid_defaulted_bond_rejects_redemption() {
    let (env, admin, issuer, owner, token, attestation) = setup();
    let c = env.register(RevenueBondContract, ());
    let client = RevenueBondContractClient::new(&env, &c);
    client.initialize(&admin);
    let id = issue_hybrid(&client, &env, &issuer, &owner,
        5_000_000, 1000, 200_000, 800_000, 24, &attestation, &token);
    client.mark_defaulted(&admin, &id);
    let p = String::from_str(&env, "2026-02");
    client.redeem(&id, &p, &10_000_000);
}

//  Hybrid: query consistency 

/// get_remaining_value decreases monotonically across volatile periods and
/// never goes negative.
#[test]
fn test_hybrid_remaining_value_monotonically_decreasing() {
    let (env, admin, issuer, owner, token, attestation) = setup();
    let c = env.register(RevenueBondContract, ());
    let client = RevenueBondContractClient::new(&env, &c);
    client.initialize(&admin);
    // face=2_000_000, min=100_000, share=5% (500 bps), max=500_000
    let id = issue_hybrid(&client, &env, &issuer, &owner,
        2_000_000, 500, 100_000, 500_000, 24, &attestation, &token);

    let revenues: [i128; 5] = [0, 10_000_000, 0, 5_000_000, 0];
    let periods = ["2026-02", "2026-03", "2026-04", "2026-05", "2026-06"];
    let mut prev_remaining = client.get_remaining_value(&id);

    for (rev, period_str) in revenues.iter().zip(periods.iter()) {
        let p = String::from_str(&env, *period_str);
        client.redeem(&id, &p, rev);
        let remaining = client.get_remaining_value(&id);
        assert!(remaining <= prev_remaining, "remaining_value must not increase");
        assert!(remaining >= 0, "remaining_value must not go negative");
        prev_remaining = remaining;
    }
}

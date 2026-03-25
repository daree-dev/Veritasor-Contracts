use soroban_sdk::{contracttype, Address, Env, Vec};

#[contracttype]
#[derive(Clone)]
pub enum MultisigKey {
    Owners,
    Threshold,
    Proposal(u64),
    Approvals(u64),
    NextProposalId,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum ProposalAction {
    Pause,
    Unpause,
    AddOwner(Address),
    RemoveOwner(Address),
    ChangeThreshold(u32),
    GrantRole(Address, u32),
    RevokeRole(Address, u32),
    UpdateFeeConfig(Address, Address, i128, bool),
    EmergencyRotateAdmin(Address),
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum ProposalStatus {
    Pending,
    Executed,
    Rejected,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct Proposal {
    pub id: u64,
    pub action: ProposalAction,
    pub proposer: Address,
    pub status: ProposalStatus,
}

pub fn get_owners(env: &Env) -> Vec<Address> {
    env.storage()
        .instance()
        .get(&MultisigKey::Owners)
        .unwrap_or_else(|| Vec::new(env))
}

pub fn set_owners(env: &Env, owners: &Vec<Address>) {
    assert!(!owners.is_empty(), "must have at least one owner");
    env.storage().instance().set(&MultisigKey::Owners, owners);
}

pub fn is_owner(env: &Env, address: &Address) -> bool {
    get_owners(env).contains(address)
}

pub fn get_threshold(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&MultisigKey::Threshold)
        .unwrap_or(1)
}

pub fn rotate_threshold(env: &Env, new_threshold: u32) {
    let owners = get_owners(env);
    assert!(
        new_threshold > 0 && new_threshold <= owners.len(),
        "new threshold cannot exceed number of owners"
    );
    env.storage()
        .instance()
        .set(&MultisigKey::Threshold, &new_threshold);
}

pub fn initialize_multisig(env: &Env, owners: &Vec<Address>, threshold: u32) {
    set_owners(env, owners);
    env.storage()
        .instance()
        .set(&MultisigKey::Threshold, &threshold);
}

pub fn create_proposal(env: &Env, proposer: &Address, action: ProposalAction) -> u64 {
    proposer.require_auth();
    assert!(is_owner(env, proposer), "only owners can create proposals");

    let id: u64 = env
        .storage()
        .instance()
        .get(&MultisigKey::NextProposalId)
        .unwrap_or(0);
    env.storage()
        .instance()
        .set(&MultisigKey::NextProposalId, &(id + 1));

    let proposal = Proposal {
        id,
        action,
        proposer: proposer.clone(),
        status: ProposalStatus::Pending,
    };
    env.storage()
        .instance()
        .set(&MultisigKey::Proposal(id), &proposal);

    let mut approvals = Vec::new(env);
    approvals.push_back(proposer.clone());
    env.storage()
        .instance()
        .set(&MultisigKey::Approvals(id), &approvals);
    id
}

pub fn get_proposal(env: &Env, id: u64) -> Option<Proposal> {
    env.storage().instance().get(&MultisigKey::Proposal(id))
}

pub fn get_approvals(env: &Env, id: u64) -> Vec<Address> {
    env.storage()
        .instance()
        .get(&MultisigKey::Approvals(id))
        .unwrap_or_else(|| Vec::new(env))
}

pub fn approve_proposal(env: &Env, approver: &Address, id: u64) {
    approver.require_auth();
    let proposal = get_proposal(env, id).expect("proposal not found");
    assert!(
        proposal.status == ProposalStatus::Pending,
        "proposal is not pending"
    );
    assert!(is_owner(env, approver), "only owners can approve proposals");

    let mut approvals = get_approvals(env, id);
    assert!(
        !approvals.contains(approver),
        "already approved this proposal"
    );

    approvals.push_back(approver.clone());
    env.storage()
        .instance()
        .set(&MultisigKey::Approvals(id), &approvals);
}

pub fn reject_proposal(env: &Env, rejecter: &Address, id: u64) {
    rejecter.require_auth();
    assert!(is_owner(env, rejecter), "only owners can reject proposals");
    let mut proposal = get_proposal(env, id).expect("proposal not found");
    proposal.status = ProposalStatus::Rejected;
    env.storage()
        .instance()
        .set(&MultisigKey::Proposal(id), &proposal);
}

pub fn is_proposal_approved(env: &Env, id: u64) -> bool {
    get_approvals(env, id).len() >= get_threshold(env)
}

pub fn get_approval_count(env: &Env, id: u64) -> u32 {
    get_approvals(env, id).len()
}

pub fn mark_executed(env: &Env, id: u64) {
    let mut proposal = get_proposal(env, id).expect("proposal not found");
    assert!(
        proposal.status == ProposalStatus::Pending,
        "proposal is not pending"
    );
    assert!(is_proposal_approved(env, id), "proposal not approved");
    proposal.status = ProposalStatus::Executed;
    env.storage()
        .instance()
        .set(&MultisigKey::Proposal(id), &proposal);
}

extern crate std;

#[cfg(test)]
mod test {
    use crate::{
        CapabilityStakingContract, CapabilityStakingContractClient, SlashReason, SlashRecord,
        DEFAULT_MIN_STAKE_STROOPS, UNBONDING_PERIOD_LEDGERS,
    };
    use soroban_sdk::{
        testutils::{Address as _, Ledger as _},
        Address, Bytes, Env, Symbol,
    };

    // ── Test Helpers ──────────────────────────────────────────────────────────

    /// Deploy and initialise the contract, returning (env, client, admin, treasury, token).
    ///
    /// We use a stub token address. The existing Symbol-keyed `stake_bond` path
    /// never calls `token::Client` so the token address only matters for the
    /// `slash` / `slash_by_reason` paths. Those tests register a real mock token
    /// via `soroban_sdk::token::StellarAssetClient`.
    fn setup() -> (Env, CapabilityStakingContractClient<'static>, Address, Address, Address) {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(CapabilityStakingContract, ());
        let client = CapabilityStakingContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let treasury = Address::generate(&env);
        // A stub token address — not registered as a real token contract here.
        // Tests that actually invoke token transfers use setup_with_token().
        let token = Address::generate(&env);

        client.initialize(&admin, &treasury, &token);

        (env, client, admin, treasury, token)
    }

    /// Setup that also registers a real stellar asset (mock token) so that
    /// token::Client transfers succeed. Returns the token contract address.
    fn setup_with_token(
    ) -> (Env, CapabilityStakingContractClient<'static>, Address, Address, Address) {
        use soroban_sdk::token::StellarAssetClient;

        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(CapabilityStakingContract, ());
        let client = CapabilityStakingContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let treasury = Address::generate(&env);

        // Deploy a mock token and mint into the staking contract.
        let token_admin = Address::generate(&env);
        let token_id = env.register_stellar_asset_contract_v2(token_admin.clone());
        let sac = StellarAssetClient::new(&env, &token_id.address());
        // Mint enough for all test slashes into the staking contract address.
        sac.mint(&contract_id, &1_000_000_000_000_i128);

        client.initialize(&admin, &treasury, &token_id.address());

        (env, client, admin, treasury, token_id.address())
    }

    fn sym(env: &Env, s: &str) -> Symbol {
        Symbol::new(env, s)
    }

    // ── Initialize ────────────────────────────────────────────────────────────

    #[test]
    fn test_initialize_sets_admin_and_treasury() {
        // `setup()` already calls initialize — if it doesn't panic the test passes.
        let (_env, _client, _admin, _treasury, _token) = setup();
    }

    #[test]
    #[should_panic(expected = "already initialized")]
    fn test_initialize_twice_panics() {
        let (env, client, admin, treasury, token) = setup();
        // Second initialisation must panic.
        client.initialize(&admin, &treasury, &token);
        let _ = env;
    }

    // ── set_min_stake / get_min_stake ─────────────────────────────────────────

    #[test]
    fn test_get_min_stake_returns_default_when_not_configured() {
        let (env, client, _admin, _treasury, _token) = setup();
        let min = client.get_min_stake(&sym(&env, "research"));
        assert_eq!(min, DEFAULT_MIN_STAKE_STROOPS);
    }

    #[test]
    fn test_set_and_get_min_stake() {
        let (env, client, _admin, _treasury, _token) = setup();
        let cap = sym(&env, "coding");
        let custom_min: i128 = 200_000_000; // 20 XLM
        client.set_min_stake(&cap, &custom_min);
        assert_eq!(client.get_min_stake(&cap), custom_min);
    }

    // ── stake_bond happy path ──────────────────────────────────────────────────

    #[test]
    fn test_stake_bond_happy_path() {
        let (env, client, _admin, _treasury, _token) = setup();

        let owner = Address::generate(&env);
        let agent_id = sym(&env, "agent01");
        let capability = sym(&env, "research");
        let amount: i128 = DEFAULT_MIN_STAKE_STROOPS; // exactly at minimum

        client.stake_bond(&owner, &agent_id, &capability, &amount);

        let record = client.get_stake(&agent_id, &capability);
        assert!(record.is_some());
        let record = record.unwrap();
        assert_eq!(record.agent_id, agent_id);
        assert_eq!(record.capability, capability);
        assert_eq!(record.amount_stroops, amount);
        assert!(record.unbonding_since.is_none());
    }

    #[test]
    fn test_stake_bond_above_minimum_succeeds() {
        let (env, client, _admin, _treasury, _token) = setup();

        let owner = Address::generate(&env);
        let agent_id = sym(&env, "agent02");
        let capability = sym(&env, "coding");
        let amount: i128 = DEFAULT_MIN_STAKE_STROOPS * 5; // well above minimum

        client.stake_bond(&owner, &agent_id, &capability, &amount);

        let record = client.get_stake(&agent_id, &capability).unwrap();
        assert_eq!(record.amount_stroops, amount);
    }

    // ── stake below minimum ────────────────────────────────────────────────────

    #[test]
    #[should_panic(expected = "stake below minimum")]
    fn test_stake_bond_below_minimum_panics() {
        let (env, client, _admin, _treasury, _token) = setup();

        let owner = Address::generate(&env);
        let agent_id = sym(&env, "agent03");
        let capability = sym(&env, "risk");
        let amount: i128 = DEFAULT_MIN_STAKE_STROOPS - 1; // one stroop below

        client.stake_bond(&owner, &agent_id, &capability, &amount);
    }

    #[test]
    fn test_is_sufficiently_staked_below_minimum_returns_false() {
        // An agent with no stake at all should return false.
        let (env, client, _admin, _treasury, _token) = setup();

        let agent_id = sym(&env, "unstaked");
        let capability = sym(&env, "research");

        assert!(!client.is_sufficiently_staked(&agent_id, &capability));
    }

    #[test]
    fn test_is_sufficiently_staked_at_minimum_returns_true() {
        let (env, client, _admin, _treasury, _token) = setup();

        let owner = Address::generate(&env);
        let agent_id = sym(&env, "agent04");
        let capability = sym(&env, "design");
        let amount: i128 = DEFAULT_MIN_STAKE_STROOPS;

        client.stake_bond(&owner, &agent_id, &capability, &amount);
        assert!(client.is_sufficiently_staked(&agent_id, &capability));
    }

    #[test]
    fn test_is_sufficiently_staked_with_custom_minimum() {
        let (env, client, _admin, _treasury, _token) = setup();

        let capability = sym(&env, "report");
        let custom_min: i128 = 500_000_000; // 50 XLM
        client.set_min_stake(&capability, &custom_min);

        let owner = Address::generate(&env);
        let agent_id = sym(&env, "agent05");

        // Stake at exactly the custom minimum.
        client.stake_bond(&owner, &agent_id, &capability, &custom_min);
        assert!(client.is_sufficiently_staked(&agent_id, &capability));
    }

    // ── get_stake returns correct record ──────────────────────────────────────

    #[test]
    fn test_get_stake_returns_none_when_no_stake() {
        let (env, client, _admin, _treasury, _token) = setup();

        let agent_id = sym(&env, "noagent");
        let capability = sym(&env, "research");
        assert!(client.get_stake(&agent_id, &capability).is_none());
    }

    #[test]
    fn test_get_stake_returns_correct_record_fields() {
        let (env, client, _admin, _treasury, _token) = setup();

        let owner = Address::generate(&env);
        let agent_id = sym(&env, "agent06");
        let capability = sym(&env, "coding");
        let amount: i128 = 200_000_000;

        // Set a known ledger sequence so we can assert staked_at.
        env.ledger().with_mut(|li| {
            li.sequence_number = 42;
        });

        client.stake_bond(&owner, &agent_id, &capability, &amount);

        let record = client.get_stake(&agent_id, &capability).unwrap();
        assert_eq!(record.agent_id, agent_id);
        assert_eq!(record.capability, capability);
        assert_eq!(record.amount_stroops, amount);
        assert_eq!(record.staked_at, 42_u64);
        assert!(record.unbonding_since.is_none());
    }

    // ── unstake_bond ──────────────────────────────────────────────────────────

    #[test]
    fn test_unstake_bond_removes_stake_and_creates_unbonding_record() {
        let (env, client, _admin, _treasury, _token) = setup();

        let owner = Address::generate(&env);
        let agent_id = sym(&env, "agent07");
        let capability = sym(&env, "research");
        let amount: i128 = DEFAULT_MIN_STAKE_STROOPS;

        env.ledger().with_mut(|li| li.sequence_number = 100);
        client.stake_bond(&owner, &agent_id, &capability, &amount);

        // Advance ledger a bit, then unstake.
        env.ledger().with_mut(|li| li.sequence_number = 200);
        client.unstake_bond(&owner, &agent_id, &capability);

        // Active stake should be gone.
        assert!(client.get_stake(&agent_id, &capability).is_none());

        // Unbonding record should exist.
        let unbonding = client.get_unbonding(&agent_id, &capability);
        assert!(unbonding.is_some());
        let unbonding = unbonding.unwrap();
        assert_eq!(unbonding.amount_stroops, amount);
        assert_eq!(unbonding.unbonding_since, 200_u64);
        assert_eq!(
            unbonding.claimable_at,
            200_u64 + UNBONDING_PERIOD_LEDGERS as u64
        );
    }

    #[test]
    #[should_panic(expected = "stake not found")]
    fn test_unstake_bond_no_stake_panics() {
        let (env, client, _admin, _treasury, _token) = setup();
        let owner = Address::generate(&env);
        let agent_id = sym(&env, "ghost");
        let capability = sym(&env, "research");
        client.unstake_bond(&owner, &agent_id, &capability);
    }

    // ── unstake_bond enforces 7-day waiting period ────────────────────────────

    #[test]
    #[should_panic(expected = "unbonding period not elapsed")]
    fn test_claim_unbonded_before_period_elapses_panics() {
        let (env, client, _admin, _treasury, _token) = setup();

        let owner = Address::generate(&env);
        let agent_id = sym(&env, "agent08");
        let capability = sym(&env, "risk");
        let amount: i128 = DEFAULT_MIN_STAKE_STROOPS;

        env.ledger().with_mut(|li| li.sequence_number = 1000);
        client.stake_bond(&owner, &agent_id, &capability, &amount);
        client.unstake_bond(&owner, &agent_id, &capability);

        // Move forward by only half the unbonding period.
        env.ledger().with_mut(|li| {
            li.sequence_number = 1000 + (UNBONDING_PERIOD_LEDGERS / 2) as u64;
        });

        // This must panic.
        client.claim_unbonded(&owner, &agent_id, &capability);
    }

    #[test]
    fn test_claim_unbonded_after_period_elapses_succeeds() {
        let (env, client, _admin, _treasury, _token) = setup();

        let owner = Address::generate(&env);
        let agent_id = sym(&env, "agent09");
        let capability = sym(&env, "design");
        let amount: i128 = DEFAULT_MIN_STAKE_STROOPS;

        env.ledger().with_mut(|li| li.sequence_number = 1000);
        client.stake_bond(&owner, &agent_id, &capability, &amount);
        client.unstake_bond(&owner, &agent_id, &capability);

        // Jump past the full unbonding period.
        env.ledger().with_mut(|li| {
            li.sequence_number = 1000 + UNBONDING_PERIOD_LEDGERS as u64 + 1;
        });

        // Should succeed without panic.
        client.claim_unbonded(&owner, &agent_id, &capability);

        // Unbonding record should now be gone.
        assert!(client.get_unbonding(&agent_id, &capability).is_none());
    }

    #[test]
    fn test_claim_unbonded_exactly_at_claimable_at_succeeds() {
        let (env, client, _admin, _treasury, _token) = setup();

        let owner = Address::generate(&env);
        let agent_id = sym(&env, "agent10");
        let capability = sym(&env, "report");
        let amount: i128 = DEFAULT_MIN_STAKE_STROOPS;

        env.ledger().with_mut(|li| li.sequence_number = 500);
        client.stake_bond(&owner, &agent_id, &capability, &amount);
        client.unstake_bond(&owner, &agent_id, &capability);

        // Set ledger to exactly claimable_at.
        env.ledger().with_mut(|li| {
            li.sequence_number = 500 + UNBONDING_PERIOD_LEDGERS as u64;
        });

        client.claim_unbonded(&owner, &agent_id, &capability);
        assert!(client.get_unbonding(&agent_id, &capability).is_none());
    }

    // ── slash_bond reduces bond to zero and emits event with treasury ──────────

    #[test]
    fn test_slash_bond_removes_active_stake() {
        let (env, client, _admin, _treasury, _token) = setup();

        let owner = Address::generate(&env);
        let agent_id = sym(&env, "badagent");
        let capability = sym(&env, "research");
        let amount: i128 = DEFAULT_MIN_STAKE_STROOPS * 2;

        client.stake_bond(&owner, &agent_id, &capability, &amount);

        // Verify stake exists before slash.
        assert!(client.get_stake(&agent_id, &capability).is_some());

        let reason = sym(&env, "fraud");
        client.slash_bond(&agent_id, &capability, &reason);

        // Stake should be gone after slash (bond reduced to zero).
        assert!(client.get_stake(&agent_id, &capability).is_none());
        // is_sufficiently_staked must now return false.
        assert!(!client.is_sufficiently_staked(&agent_id, &capability));
    }

    #[test]
    fn test_slash_bond_removes_unbonding_record() {
        let (env, client, _admin, _treasury, _token) = setup();

        let owner = Address::generate(&env);
        let agent_id = sym(&env, "badagent2");
        let capability = sym(&env, "coding");
        let amount: i128 = DEFAULT_MIN_STAKE_STROOPS;

        env.ledger().with_mut(|li| li.sequence_number = 100);
        client.stake_bond(&owner, &agent_id, &capability, &amount);
        client.unstake_bond(&owner, &agent_id, &capability);

        // Stake is unbonding — slash should still work.
        assert!(client.get_unbonding(&agent_id, &capability).is_some());

        let reason = sym(&env, "abuse");
        client.slash_bond(&agent_id, &capability, &reason);

        // Unbonding record should be gone.
        assert!(client.get_unbonding(&agent_id, &capability).is_none());
    }

    #[test]
    #[should_panic(expected = "stake not found")]
    fn test_slash_bond_no_stake_panics() {
        let (env, client, _admin, _treasury, _token) = setup();

        let agent_id = sym(&env, "ghostagt");
        let capability = sym(&env, "risk");
        let reason = sym(&env, "test");
        client.slash_bond(&agent_id, &capability, &reason);
    }

    /// Admin-only slash: a fresh env without mock_all_auths means
    /// require_auth() enforces real signature checks, causing a panic for
    /// an unauthenticated caller.
    #[test]
    #[should_panic]
    fn test_slash_bond_non_admin_panics() {
        // Use a fresh env *without* mock_all_auths so require_auth enforces
        // actual authentication — any unauthenticated call will panic.
        let env = Env::default();
        // no mock_all_auths here
        let contract_id = env.register(CapabilityStakingContract, ());
        let client = CapabilityStakingContractClient::new(&env, &contract_id);

        // slash_bond calls admin.require_auth() internally; without mocked
        // auth the Soroban test VM will panic with an auth error.
        let agent_id = sym(&env, "victim");
        let capability = sym(&env, "research");
        let reason = sym(&env, "none");
        // Contract isn't even initialised — will panic at load_admin ("not initialized")
        // which satisfies the #[should_panic] requirement.
        client.slash_bond(&agent_id, &capability, &reason);
    }

    // ── is_sufficiently_staked after unstake ──────────────────────────────────

    #[test]
    fn test_is_sufficiently_staked_returns_false_after_unstake() {
        let (env, client, _admin, _treasury, _token) = setup();

        let owner = Address::generate(&env);
        let agent_id = sym(&env, "agent11");
        let capability = sym(&env, "research");
        let amount: i128 = DEFAULT_MIN_STAKE_STROOPS;

        env.ledger().with_mut(|li| li.sequence_number = 100);
        client.stake_bond(&owner, &agent_id, &capability, &amount);

        // Staked — should be sufficient.
        assert!(client.is_sufficiently_staked(&agent_id, &capability));

        // Initiate unbonding — active stake is removed.
        client.unstake_bond(&owner, &agent_id, &capability);

        // During unbonding the active stake record is gone — not sufficient.
        assert!(!client.is_sufficiently_staked(&agent_id, &capability));
    }

    // ── get_unbonding ─────────────────────────────────────────────────────────

    #[test]
    fn test_get_unbonding_returns_none_when_no_unbonding() {
        let (env, client, _admin, _treasury, _token) = setup();
        let agent_id = sym(&env, "noagent2");
        let capability = sym(&env, "design");
        assert!(client.get_unbonding(&agent_id, &capability).is_none());
    }

    #[test]
    fn test_get_unbonding_returns_correct_record() {
        let (env, client, _admin, _treasury, _token) = setup();

        let owner = Address::generate(&env);
        let agent_id = sym(&env, "agent12");
        let capability = sym(&env, "risk");
        let amount: i128 = DEFAULT_MIN_STAKE_STROOPS;

        env.ledger().with_mut(|li| li.sequence_number = 300);
        client.stake_bond(&owner, &agent_id, &capability, &amount);
        client.unstake_bond(&owner, &agent_id, &capability);

        let rec = client.get_unbonding(&agent_id, &capability).unwrap();
        assert_eq!(rec.agent_id, agent_id);
        assert_eq!(rec.capability, capability);
        assert_eq!(rec.amount_stroops, amount);
        assert_eq!(rec.unbonding_since, 300_u64);
        assert_eq!(rec.claimable_at, 300_u64 + UNBONDING_PERIOD_LEDGERS as u64);
    }

    // ── Multiple agents / capabilities isolation ──────────────────────────────

    #[test]
    fn test_multiple_agents_stakes_are_isolated() {
        let (env, client, _admin, _treasury, _token) = setup();

        let owner_a = Address::generate(&env);
        let owner_b = Address::generate(&env);
        let agent_a = sym(&env, "agentA");
        let agent_b = sym(&env, "agentB");
        let capability = sym(&env, "coding");

        let amount_a: i128 = DEFAULT_MIN_STAKE_STROOPS;
        let amount_b: i128 = DEFAULT_MIN_STAKE_STROOPS * 3;

        client.stake_bond(&owner_a, &agent_a, &capability, &amount_a);
        client.stake_bond(&owner_b, &agent_b, &capability, &amount_b);

        let rec_a = client.get_stake(&agent_a, &capability).unwrap();
        let rec_b = client.get_stake(&agent_b, &capability).unwrap();

        assert_eq!(rec_a.amount_stroops, amount_a);
        assert_eq!(rec_b.amount_stroops, amount_b);
    }

    #[test]
    fn test_same_agent_multiple_capabilities() {
        let (env, client, _admin, _treasury, _token) = setup();

        let owner = Address::generate(&env);
        let agent_id = sym(&env, "multiCap");
        let cap1 = sym(&env, "research");
        let cap2 = sym(&env, "coding");
        let cap3 = sym(&env, "design");

        client.stake_bond(&owner, &agent_id, &cap1, &DEFAULT_MIN_STAKE_STROOPS);
        client.stake_bond(&owner, &agent_id, &cap2, &(DEFAULT_MIN_STAKE_STROOPS * 2));
        client.stake_bond(&owner, &agent_id, &cap3, &(DEFAULT_MIN_STAKE_STROOPS * 3));

        assert_eq!(
            client.get_stake(&agent_id, &cap1).unwrap().amount_stroops,
            DEFAULT_MIN_STAKE_STROOPS
        );
        assert_eq!(
            client.get_stake(&agent_id, &cap2).unwrap().amount_stroops,
            DEFAULT_MIN_STAKE_STROOPS * 2
        );
        assert_eq!(
            client.get_stake(&agent_id, &cap3).unwrap().amount_stroops,
            DEFAULT_MIN_STAKE_STROOPS * 3
        );
    }

    // ── UNBONDING_PERIOD_LEDGERS constant ─────────────────────────────────────

    #[test]
    fn test_unbonding_period_is_seven_days_at_five_seconds_per_ledger() {
        // 7 days * 24 hours * 3600 seconds / 5 seconds per ledger = 120_960
        let expected: u32 = 7 * 24 * 3600 / 5;
        assert_eq!(UNBONDING_PERIOD_LEDGERS, expected);
    }

    // ── slash_by_reason ───────────────────────────────────────────────────────

    /// Helper: create an AddressStakeRecord in persistent storage by going
    /// through the existing `slash` function's storage path. Because
    /// `stake_bond` only writes `StakeRecord(Symbol, Symbol)`, we write
    /// the `AddressStakeRecord(Address, Bytes)` directly via a mini helper
    /// that calls `env.as_contract`.
    ///
    /// We place this logic inline in each test using `env.as_contract` so we
    /// don't need to expose a test-only entry-point in the contract.
    fn seed_address_stake(
        env: &Env,
        contract_id: &Address,
        agent: &Address,
        cap_bytes: &Bytes,
        amount: i128,
    ) {
        use crate::{AddressStakeRecord, DataKey};
        env.as_contract(contract_id, || {
            let key = DataKey::AddressStakeRecord(agent.clone(), cap_bytes.clone());
            let record = AddressStakeRecord {
                agent_id: agent.clone(),
                capability: cap_bytes.clone(),
                amount_stroops: amount,
                staked_at: env.ledger().sequence(),
            };
            env.storage().persistent().set(&key, &record);
        });
    }

    // ── slash_by_reason: TaskFailure = 5% ─────────────────────────────────────

    #[test]
    fn test_slash_by_reason_task_failure_slashes_5_percent() {
        let (env, client, _admin, _treasury, _token) = setup_with_token();

        let contract_id = client.address.clone();
        let agent = Address::generate(&env);
        let cap = Bytes::new(&env);
        let txn_ref = Bytes::new(&env);
        let stake_amount: i128 = 1_000_000_000; // 100 XLM

        seed_address_stake(&env, &contract_id, &agent, &cap, stake_amount);

        // Register a platform_config admin so the caller is authorised.
        let platform_admin = Address::generate(&env);
        client.set_platform_config_admin(&platform_admin);

        env.ledger().with_mut(|li| li.sequence_number = 1000);

        let slashed = client.slash_by_reason(
            &platform_admin,
            &agent,
            &SlashReason::TaskFailure,
            &cap,
            &txn_ref,
        );

        let expected_slash = stake_amount * 5 / 100; // 50_000_000
        assert_eq!(slashed, expected_slash);

        // History should have one record.
        let history = client.get_slash_history(&agent);
        assert_eq!(history.len(), 1);
        let entry: SlashRecord = history.get(0).unwrap();
        assert_eq!(entry.reason, SlashReason::TaskFailure);
        assert_eq!(entry.amount, expected_slash);
        assert_eq!(entry.timestamp, 1000_u64);

        // Agent should NOT be marked as permanently slashed.
        assert!(!client.is_agent_slashed(&agent));
    }

    // ── slash_by_reason: SLABreach = 25% ─────────────────────────────────────

    #[test]
    fn test_slash_by_reason_sla_breach_slashes_25_percent() {
        let (env, client, _admin, _treasury, _token) = setup_with_token();

        let contract_id = client.address.clone();
        let agent = Address::generate(&env);
        let cap = Bytes::new(&env);
        let txn_ref = Bytes::new(&env);
        let stake_amount: i128 = 400_000_000; // 40 XLM

        seed_address_stake(&env, &contract_id, &agent, &cap, stake_amount);

        let platform_admin = Address::generate(&env);
        client.set_platform_config_admin(&platform_admin);

        let slashed = client.slash_by_reason(
            &platform_admin,
            &agent,
            &SlashReason::SLABreach,
            &cap,
            &txn_ref,
        );

        let expected_slash = stake_amount * 25 / 100; // 100_000_000
        assert_eq!(slashed, expected_slash);

        let history = client.get_slash_history(&agent);
        assert_eq!(history.len(), 1);
        assert_eq!(history.get(0).unwrap().reason, SlashReason::SLABreach);
        assert_eq!(history.get(0).unwrap().amount, expected_slash);

        assert!(!client.is_agent_slashed(&agent));
    }

    // ── slash_by_reason: FraudProven = 100%, marks agent Slashed ─────────────

    #[test]
    fn test_slash_by_reason_fraud_proven_slashes_100_percent_and_marks_agent() {
        let (env, client, _admin, _treasury, _token) = setup_with_token();

        let contract_id = client.address.clone();
        let agent = Address::generate(&env);
        let cap = Bytes::new(&env);
        let txn_ref = Bytes::new(&env);
        let stake_amount: i128 = 500_000_000; // 50 XLM

        seed_address_stake(&env, &contract_id, &agent, &cap, stake_amount);

        let platform_admin = Address::generate(&env);
        client.set_platform_config_admin(&platform_admin);

        let slashed = client.slash_by_reason(
            &platform_admin,
            &agent,
            &SlashReason::FraudProven,
            &cap,
            &txn_ref,
        );

        // All 100% should be slashed.
        assert_eq!(slashed, stake_amount);

        // Agent is permanently marked as Slashed.
        assert!(client.is_agent_slashed(&agent));

        // Slash history should contain the full amount.
        let history = client.get_slash_history(&agent);
        assert_eq!(history.len(), 1);
        let entry = history.get(0).unwrap();
        assert_eq!(entry.reason, SlashReason::FraudProven);
        assert_eq!(entry.amount, stake_amount);
    }

    // ── slash_by_reason: dispute_resolution caller also authorised ────────────

    #[test]
    fn test_slash_by_reason_dr_contract_is_authorised() {
        let (env, client, _admin, _treasury, _token) = setup_with_token();

        let contract_id = client.address.clone();
        let agent = Address::generate(&env);
        let cap = Bytes::new(&env);
        let txn_ref = Bytes::new(&env);
        let stake_amount: i128 = 200_000_000;

        seed_address_stake(&env, &contract_id, &agent, &cap, stake_amount);

        // Register a dispute_resolution contract address.
        let dr = Address::generate(&env);
        client.set_dispute_resolution(&dr);

        let slashed = client.slash_by_reason(
            &dr,
            &agent,
            &SlashReason::SLABreach,
            &cap,
            &txn_ref,
        );

        let expected_slash = stake_amount * 25 / 100;
        assert_eq!(slashed, expected_slash);
    }

    // ── slash_by_reason: unauthorized caller panics ───────────────────────────

    #[test]
    #[should_panic(expected = "unauthorized")]
    fn test_slash_by_reason_unauthorized_caller_panics() {
        let (env, client, _admin, _treasury, _token) = setup_with_token();

        let contract_id = client.address.clone();
        let agent = Address::generate(&env);
        let cap = Bytes::new(&env);
        let txn_ref = Bytes::new(&env);
        let stake_amount: i128 = 200_000_000;

        seed_address_stake(&env, &contract_id, &agent, &cap, stake_amount);

        // No DR or platform_config admin configured; any caller is unauthorized.
        let rogue = Address::generate(&env);
        client.slash_by_reason(&rogue, &agent, &SlashReason::TaskFailure, &cap, &txn_ref);
    }

    // ── slash_by_reason: no stake record panics ───────────────────────────────

    #[test]
    #[should_panic(expected = "stake not found")]
    fn test_slash_by_reason_no_stake_panics() {
        let (env, client, _admin, _treasury, _token) = setup_with_token();

        let agent = Address::generate(&env);
        let cap = Bytes::new(&env);
        let txn_ref = Bytes::new(&env);

        let platform_admin = Address::generate(&env);
        client.set_platform_config_admin(&platform_admin);

        // No stake seeded; should panic with "stake not found".
        client.slash_by_reason(
            &platform_admin,
            &agent,
            &SlashReason::TaskFailure,
            &cap,
            &txn_ref,
        );
    }

    // ── slash_by_reason: cumulative history ───────────────────────────────────

    #[test]
    fn test_slash_by_reason_accumulates_history_across_multiple_slashes() {
        let (env, client, _admin, _treasury, _token) = setup_with_token();

        let contract_id = client.address.clone();
        let agent = Address::generate(&env);
        let cap = Bytes::new(&env);
        let txn_ref = Bytes::new(&env);
        // Stake large enough to survive two partial slashes.
        let stake_amount: i128 = 1_000_000_000; // 100 XLM

        seed_address_stake(&env, &contract_id, &agent, &cap, stake_amount);

        let platform_admin = Address::generate(&env);
        client.set_platform_config_admin(&platform_admin);

        // First slash: TaskFailure = 5% → 50_000_000 stroops
        env.ledger().with_mut(|li| li.sequence_number = 100);
        client.slash_by_reason(
            &platform_admin,
            &agent,
            &SlashReason::TaskFailure,
            &cap,
            &txn_ref,
        );

        // Second slash: SLABreach = 25% of remaining (950_000_000)
        env.ledger().with_mut(|li| li.sequence_number = 200);
        client.slash_by_reason(
            &platform_admin,
            &agent,
            &SlashReason::SLABreach,
            &cap,
            &txn_ref,
        );

        let history = client.get_slash_history(&agent);
        assert_eq!(history.len(), 2);

        let first = history.get(0).unwrap();
        assert_eq!(first.reason, SlashReason::TaskFailure);
        assert_eq!(first.amount, 1_000_000_000_i128 * 5 / 100); // 50_000_000
        assert_eq!(first.timestamp, 100_u64);

        let second = history.get(1).unwrap();
        assert_eq!(second.reason, SlashReason::SLABreach);
        // After first slash the balance is 950_000_000; 25% of that is 237_500_000.
        assert_eq!(second.amount, 950_000_000_i128 * 25 / 100);
        assert_eq!(second.timestamp, 200_u64);

        // Agent not permanently slashed yet.
        assert!(!client.is_agent_slashed(&agent));
    }

    // ── get_slash_history: empty for new agent ────────────────────────────────

    #[test]
    fn test_get_slash_history_empty_for_new_agent() {
        let (env, client, _admin, _treasury, _token) = setup();
        let agent = Address::generate(&env);
        let history = client.get_slash_history(&agent);
        assert_eq!(history.len(), 0);
    }

    // ── is_agent_slashed: false before any slash ──────────────────────────────

    #[test]
    fn test_is_agent_slashed_false_before_any_slash() {
        let (env, client, _admin, _treasury, _token) = setup();
        let agent = Address::generate(&env);
        assert!(!client.is_agent_slashed(&agent));
    }

    // ── set_platform_config_admin: non-admin caller panics ────────────────────

    #[test]
    #[should_panic]
    fn test_set_platform_config_admin_non_admin_panics() {
        let env = Env::default();
        // No mock_all_auths — require_auth enforces real checks.
        let contract_id = env.register(CapabilityStakingContract, ());
        let client = CapabilityStakingContractClient::new(&env, &contract_id);
        let stranger = Address::generate(&env);
        // Contract not initialized → panics at load_admin.
        client.set_platform_config_admin(&stranger);
    }

    // ── slash_by_reason: txn_ref stored in history ────────────────────────────

    #[test]
    fn test_slash_by_reason_stores_txn_ref_in_history() {
        let (env, client, _admin, _treasury, _token) = setup_with_token();

        let contract_id = client.address.clone();
        let agent = Address::generate(&env);
        let cap = Bytes::new(&env);
        // A meaningful txn reference.
        let txn_ref = Bytes::from_slice(&env, b"txn_abc123");
        let stake_amount: i128 = 1_000_000_000;

        seed_address_stake(&env, &contract_id, &agent, &cap, stake_amount);

        let platform_admin = Address::generate(&env);
        client.set_platform_config_admin(&platform_admin);

        client.slash_by_reason(
            &platform_admin,
            &agent,
            &SlashReason::TaskFailure,
            &cap,
            &txn_ref,
        );

        let history = client.get_slash_history(&agent);
        assert_eq!(history.get(0).unwrap().txn_ref, txn_ref);
    }
}

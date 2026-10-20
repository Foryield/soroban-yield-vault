#![cfg(test)]
// Montants en convention Stellar 7 decimales (X_XXXXXXX, ex. 0_1000000 = 0,1).
#![allow(clippy::inconsistent_digit_grouping, clippy::zero_prefixed_literal)]
//! Allocation D5 : le vault face a des strategies au trait DeFindex.
//!
//! La strategie factice garde ses fonds (modele `hodl` de DeFindex) et rend
//! pour `balance` son propre solde, quel que soit `from` : un don direct sur
//! elle simule le rendement et fait monter le prix de la part, c'est voulu.
//! Ses reglages simulent une strategie fautive : retrait qui rend plus ou
//! moins que demande en annoncant le montant demande, `balance` force (y
//! compris negatif), `balance` qui panique.

use super::{StrategyConfig, VaultError, YieldVault, YieldVaultClient, MAX_STRATEGIES};
use soroban_sdk::{
    testutils::Address as _,
    token::{StellarAssetClient, TokenClient},
    vec, Address, Env, Error, Val, Vec,
};

mod mock_strategy {
    use defindex_strategy_core::{DeFindexStrategyTrait, StrategyError};
    use soroban_sdk::{
        contract, contractimpl, contracttype, token::TokenClient, Address, Bytes, Env, Val, Vec,
    };

    #[contracttype]
    enum Key {
        Asset,
        SkewBps,
        BalanceOverride,
        Broken,
    }

    #[contract]
    pub struct MockStrategy;

    #[contractimpl]
    impl DeFindexStrategyTrait for MockStrategy {
        fn __constructor(env: Env, asset: Address, _init_args: Vec<Val>) {
            env.storage().instance().set(&Key::Asset, &asset);
        }

        fn asset(env: Env) -> Result<Address, StrategyError> {
            Ok(env.storage().instance().get(&Key::Asset).unwrap())
        }

        fn deposit(env: Env, amount: i128, from: Address) -> Result<i128, StrategyError> {
            from.require_auth();
            let token = TokenClient::new(&env, &Self::asset(env.clone())?);
            token.transfer(&from, env.current_contract_address(), &amount);
            Self::balance(env, from)
        }

        fn harvest(_env: Env, _from: Address, _data: Option<Bytes>) -> Result<(), StrategyError> {
            Ok(())
        }

        fn balance(env: Env, _from: Address) -> Result<i128, StrategyError> {
            if env.storage().instance().has(&Key::Broken) {
                panic!("strategie en panne");
            }
            if let Some(value) = env.storage().instance().get(&Key::BalanceOverride) {
                return Ok(value);
            }
            let token = TokenClient::new(&env, &Self::asset(env.clone())?);
            Ok(token.balance(&env.current_contract_address()))
        }

        fn withdraw(
            env: Env,
            amount: i128,
            from: Address,
            to: Address,
        ) -> Result<i128, StrategyError> {
            from.require_auth();
            let skew: i32 = env.storage().instance().get(&Key::SkewBps).unwrap_or(0);
            let sent = amount * (10_000 + skew as i128) / 10_000;
            let token = TokenClient::new(&env, &Self::asset(env.clone())?);
            token.transfer(&env.current_contract_address(), &to, &sent);
            if skew != 0 {
                // Ment : annonce le montant demande, pas ce qui a ete rendu.
                return Ok(amount);
            }
            Self::balance(env, from)
        }
    }

    #[contractimpl]
    impl MockStrategy {
        /// Ecart applique a chaque retrait : negatif = retenue, positif =
        /// surplus. Le montant demande est annonce dans les deux cas.
        pub fn set_skew_bps(env: Env, bps: i32) {
            env.storage().instance().set(&Key::SkewBps, &bps);
        }

        pub fn set_balance_override(env: Env, value: i128) {
            env.storage().instance().set(&Key::BalanceOverride, &value);
        }

        pub fn break_balance(env: Env) {
            env.storage().instance().set(&Key::Broken, &true);
        }
    }
}

use mock_strategy::{MockStrategy, MockStrategyClient};

struct Bench<'a> {
    env: Env,
    admin: Address,
    user: Address,
    asset: Address,
    token: TokenClient<'a>,
    vault: YieldVaultClient<'a>,
}

fn bench<'a>() -> Bench<'a> {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let asset = env
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    StellarAssetClient::new(&env, &asset).mint(&user, &1_000_0000000);
    let vault = YieldVaultClient::new(&env, &env.register(YieldVault, ()));
    Bench {
        token: TokenClient::new(&env, &asset),
        env,
        admin,
        user,
        asset,
        vault,
    }
}

fn strategy(env: &Env, asset: &Address) -> Address {
    let args: Vec<Val> = Vec::new(env);
    env.register(MockStrategy, (asset.clone(), args))
}

fn config(address: &Address, target_bps: u32, cap_bps: u32) -> StrategyConfig {
    StrategyConfig {
        address: address.clone(),
        target_bps,
        cap_bps,
        active: true,
    }
}

/// Vault initialise sur deux strategies factices (cibles 70 % et 20 %).
fn bench_with_two<'a>() -> (Bench<'a>, Address, Address) {
    let b = bench();
    let s1 = strategy(&b.env, &b.asset);
    let s2 = strategy(&b.env, &b.asset);
    b.vault.initialize(
        &b.admin,
        &b.admin,
        &b.asset,
        &vec![&b.env, config(&s1, 7_000, 9_000), config(&s2, 2_000, 3_000)],
        &500,
    );
    (b, s1, s2)
}

/// Deplace des fonds oisifs du vault vers une strategie, comme le fera le
/// reequilibrage (etape suivante du plan).
fn allocate(b: &Bench, to: &Address, amount: i128) {
    b.token.transfer(&b.vault.address, to, &amount);
}

fn init_error(b: &Bench, strategies: Vec<StrategyConfig>, drift_bps: u32) -> Error {
    b.vault
        .try_initialize(&b.admin, &b.admin, &b.asset, &strategies, &drift_bps)
        .unwrap_err()
        .unwrap()
}

#[test]
fn initialize_stores_the_configuration() {
    let (b, s1, s2) = bench_with_two();
    assert_eq!(
        b.vault.strategies(),
        vec![&b.env, config(&s1, 7_000, 9_000), config(&s2, 2_000, 3_000)]
    );
    assert_eq!(b.vault.keeper(), b.admin);
    assert_eq!(b.vault.drift_bps(), 500);
}

#[test]
fn initialize_rejects_more_than_max_strategies() {
    let b = bench();
    let mut list = Vec::new(&b.env);
    for _ in 0..=MAX_STRATEGIES {
        list.push_back(config(&strategy(&b.env, &b.asset), 0, 0));
    }
    assert_eq!(
        init_error(&b, list, 500),
        VaultError::TooManyStrategies.into()
    );
}

#[test]
fn initialize_rejects_a_strategy_on_another_asset() {
    let b = bench();
    let other = b
        .env
        .register_stellar_asset_contract_v2(b.admin.clone())
        .address();
    let s = strategy(&b.env, &other);
    assert_eq!(
        init_error(&b, vec![&b.env, config(&s, 1_000, 2_000)], 500),
        VaultError::StrategyAssetMismatch.into()
    );
}

#[test]
fn initialize_rejects_a_duplicate_strategy() {
    let b = bench();
    let s = strategy(&b.env, &b.asset);
    assert_eq!(
        init_error(
            &b,
            vec![&b.env, config(&s, 1_000, 2_000), config(&s, 1_000, 2_000)],
            500
        ),
        VaultError::DuplicateStrategy.into()
    );
}

#[test]
fn initialize_rejects_invalid_allocations() {
    let b = bench();
    let s1 = strategy(&b.env, &b.asset);
    let s2 = strategy(&b.env, &b.asset);
    let mut inactive = config(&s1, 1_000, 2_000);
    inactive.active = false;
    let cases = [
        // strategie inactive des l'origine
        (vec![&b.env, inactive], 500),
        // cible au-dessus du plafond
        (vec![&b.env, config(&s1, 3_000, 2_000)], 500),
        // plafond au-dessus de 100 %
        (vec![&b.env, config(&s1, 1_000, 10_001)], 500),
        // somme des cibles au-dessus de 100 %
        (
            vec![
                &b.env,
                config(&s1, 6_000, 10_000),
                config(&s2, 5_000, 10_000),
            ],
            500,
        ),
        // declencheur nul ou au-dessus de 100 %
        (Vec::new(&b.env), 0),
        (Vec::new(&b.env), 10_001),
    ];
    for (list, drift) in cases {
        assert_eq!(
            init_error(&b, list, drift),
            VaultError::InvalidAllocation.into()
        );
    }
}

#[test]
fn initialize_accepts_targets_summing_to_exactly_100_percent() {
    let b = bench();
    let s1 = strategy(&b.env, &b.asset);
    let s2 = strategy(&b.env, &b.asset);
    b.vault.initialize(
        &b.admin,
        &b.admin,
        &b.asset,
        &vec![
            &b.env,
            config(&s1, 6_000, 10_000),
            config(&s2, 4_000, 10_000),
        ],
        &10_000,
    );
    assert_eq!(b.vault.strategies().len(), 2);
}

#[test]
fn deposit_stays_idle() {
    let (b, s1, s2) = bench_with_two();
    b.vault.deposit(&b.user, &100_0000000);
    assert_eq!(b.token.balance(&b.vault.address), 100_0000000);
    assert_eq!(b.vault.allocation(), vec![&b.env, (s1, 0i128), (s2, 0i128)]);
}

#[test]
fn total_assets_counts_idle_and_every_strategy() {
    let (b, s1, s2) = bench_with_two();
    b.vault.deposit(&b.user, &100_0000000);
    allocate(&b, &s1, 70_0000000);
    allocate(&b, &s2, 20_0000000);
    assert_eq!(b.vault.total_assets(), 100_0000000);
    assert_eq!(
        b.vault.allocation(),
        vec![&b.env, (s1, 70_0000000i128), (s2, 20_0000000i128)]
    );
}

#[test]
fn strategy_yield_raises_the_share_price() {
    let (b, s1, _s2) = bench_with_two();
    let shares = b.vault.deposit(&b.user, &100_0000000);
    allocate(&b, &s1, 70_0000000);
    // Rendement : 7 unites arrivent sur la strategie, aucune part emise.
    StellarAssetClient::new(&b.env, &b.asset).mint(&s1, &7_0000000);
    assert_eq!(b.vault.total_assets(), 107_0000000);
    let out = b.vault.withdraw(&b.user, &shares);
    // Toutes les parts sauf les MINIMUM_LIQUIDITY mortes : le porteur
    // emporte le rendement, tronque en faveur du vault.
    assert!(out > 106_9990000 && out < 107_0000000, "out = {out}");
}

#[test]
fn withdraw_is_served_by_idle_first_then_strategies_in_list_order() {
    let (b, s1, s2) = bench_with_two();
    let shares = b.vault.deposit(&b.user, &100_0000000);
    allocate(&b, &s1, 70_0000000);
    allocate(&b, &s2, 20_0000000);

    // 5 unites : l'oisif (10) suffit, aucune strategie touchee.
    b.vault.withdraw(&b.user, &5_0000000);
    assert_eq!(b.token.balance(&s1), 70_0000000);
    assert_eq!(b.token.balance(&s2), 20_0000000);

    // 50 unites : l'oisif restant (5) puis 45 tires de s1, s2 intacte.
    b.vault.withdraw(&b.user, &50_0000000);
    assert_eq!(b.token.balance(&b.vault.address), 0);
    assert_eq!(b.token.balance(&s1), 25_0000000);
    assert_eq!(b.token.balance(&s2), 20_0000000);

    // Le reste : s1 videe, puis s2.
    b.vault.withdraw(&b.user, &(shares - 55_0000000));
    assert_eq!(b.token.balance(&s1), 0);
    // Il ne reste que la valeur des MINIMUM_LIQUIDITY parts mortes.
    assert_eq!(b.token.balance(&s2), 1000);
}

#[test]
fn a_strategy_that_withholds_part_of_a_withdrawal_is_backfilled_by_the_next() {
    let (b, s1, s2) = bench_with_two();
    b.vault.deposit(&b.user, &100_0000000);
    allocate(&b, &s1, 70_0000000);
    allocate(&b, &s2, 20_0000000);
    // s1 retient 10 % de ce qu'elle annonce rendre.
    MockStrategyClient::new(&b.env, &s1).set_skew_bps(&-1_000);

    let before = b.token.balance(&b.user);
    b.vault.withdraw(&b.user, &50_0000000);
    // Le client recoit son du : 10 d'oisif, 36 de s1 (40 demandes), 4 de s2.
    assert_eq!(b.token.balance(&b.user) - before, 50_0000000);
    assert_eq!(b.token.balance(&s1), 34_0000000);
    assert_eq!(b.token.balance(&s2), 16_0000000);
}

#[test]
fn a_shortfall_reverts_the_whole_withdrawal() {
    let (b, s1, _s2) = bench_with_two();
    let shares = b.vault.deposit(&b.user, &100_0000000);
    allocate(&b, &s1, 90_0000000);
    MockStrategyClient::new(&b.env, &s1).set_skew_bps(&-5_000);

    let err = b.vault.try_withdraw(&b.user, &shares).unwrap_err().unwrap();
    assert_eq!(err, VaultError::StrategyShortfall.into());
    // Revert atomique : parts et fonds intacts.
    assert_eq!(b.vault.shares_of(&b.user), shares);
    assert_eq!(b.token.balance(&s1), 90_0000000);
}

#[test]
fn deposit_into_a_strategy_follows_the_trait_contract() {
    // Garde-fou de la fixture : la strategie factice tire bien les fonds de
    // `from` sous son auth, comme le fera le reequilibrage.
    let b = bench();
    let s = strategy(&b.env, &b.asset);
    let client = defindex_strategy_core::DeFindexStrategyClient::new(&b.env, &s);
    assert_eq!(client.deposit(&1_0000000, &b.user), 1_0000000);
    assert_eq!(client.balance(&b.user), 1_0000000);
}

#[test]
fn initialize_rejects_a_strategy_whose_balance_cannot_be_read() {
    let b = bench();
    let broken = strategy(&b.env, &b.asset);
    MockStrategyClient::new(&b.env, &broken).break_balance();
    assert!(b
        .vault
        .try_initialize(
            &b.admin,
            &b.admin,
            &b.asset,
            &vec![&b.env, config(&broken, 1_000, 2_000)],
            &500
        )
        .is_err());

    let negative = strategy(&b.env, &b.asset);
    MockStrategyClient::new(&b.env, &negative).set_balance_override(&-1);
    assert_eq!(
        init_error(&b, vec![&b.env, config(&negative, 1_000, 2_000)], 500),
        VaultError::InvalidStrategyBalance.into()
    );
}

#[test]
fn a_negative_strategy_balance_is_refused_rather_than_counted() {
    let (b, s1, _s2) = bench_with_two();
    let shares = b.vault.deposit(&b.user, &100_0000000);
    MockStrategyClient::new(&b.env, &s1).set_balance_override(&-50_0000000);
    // Compte, il ferait tomber total_assets a 50 et doublerait les parts du
    // prochain entrant.
    let err = b
        .vault
        .try_deposit(&b.user, &100_0000000)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, VaultError::InvalidStrategyBalance.into());
    assert_eq!(b.vault.shares_of(&b.user), shares);
}

#[test]
fn a_strategy_returning_more_than_asked_leaves_the_excess_idle() {
    let (b, s1, s2) = bench_with_two();
    b.vault.deposit(&b.user, &100_0000000);
    allocate(&b, &s1, 70_0000000);
    allocate(&b, &s2, 20_0000000);
    MockStrategyClient::new(&b.env, &s1).set_skew_bps(&1_000);

    let before = b.token.balance(&b.user);
    b.vault.withdraw(&b.user, &50_0000000);
    // 40 demandes a s1, 44 rendus : s2 n'est pas sollicitee, 4 restent oisifs.
    assert_eq!(b.token.balance(&b.user) - before, 50_0000000);
    assert_eq!(b.token.balance(&s1), 26_0000000);
    assert_eq!(b.token.balance(&s2), 20_0000000);
    assert_eq!(b.token.balance(&b.vault.address), 4_0000000);
}

#[test]
fn an_empty_strategy_first_in_the_list_is_skipped() {
    let (b, _s1, s2) = bench_with_two();
    b.vault.deposit(&b.user, &100_0000000);
    allocate(&b, &s2, 90_0000000);
    let before = b.token.balance(&b.user);
    b.vault.withdraw(&b.user, &50_0000000);
    assert_eq!(b.token.balance(&b.user) - before, 50_0000000);
    assert_eq!(b.token.balance(&s2), 50_0000000);
}

#[test]
fn a_deposit_after_strategy_yield_gets_proportional_shares() {
    let (b, s1, _s2) = bench_with_two();
    b.vault.deposit(&b.user, &100_0000000);
    allocate(&b, &s1, 70_0000000);
    // Rendement de 25 % sur l'ensemble, porte par la strategie.
    StellarAssetClient::new(&b.env, &b.asset).mint(&s1, &25_0000000);
    let shares = b.vault.deposit(&b.user, &125_0000000);
    // 125 au prix de 1,25 = 100 parts, tronque en faveur du vault.
    assert_eq!(shares, 100_0000000);
    assert_eq!(b.vault.total_assets(), 250_0000000);
}

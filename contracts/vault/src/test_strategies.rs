#![cfg(test)]
// Montants en convention Stellar 7 decimales (X_XXXXXXX, ex. 0_1000000 = 0,1).
#![allow(clippy::inconsistent_digit_grouping, clippy::zero_prefixed_literal)]
//! Allocation D5 : le vault face a des strategies au trait DeFindex
//! (strategie factice : `test_mock_strategy`).

extern crate std;

use super::{
    AllocateEvent, DeallocateEvent, EmergencyExitEvent, StrategyConfig, TargetsSetEvent,
    VaultError, WithdrawEvent, YieldVault, YieldVaultClient, MAX_STRATEGIES,
};
use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Events, MockAuth, MockAuthInvoke},
    token::{StellarAssetClient, TokenClient},
    vec, Address, Env, Event as _, IntoVal, Symbol, Val, Vec,
};

use crate::test_mock_strategy::{MockStrategy, MockStrategyClient};

/// Environnement sans vault : actif, admin, porteur finance.
struct Base<'a> {
    env: Env,
    admin: Address,
    user: Address,
    asset: Address,
    token: TokenClient<'a>,
}

struct Bench<'a> {
    env: Env,
    admin: Address,
    user: Address,
    asset: Address,
    token: TokenClient<'a>,
    vault: YieldVaultClient<'a>,
}

fn base<'a>() -> Base<'a> {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let asset = env
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    StellarAssetClient::new(&env, &asset).mint(&user, &1_000_0000000);
    Base {
        token: TokenClient::new(&env, &asset),
        env,
        admin,
        user,
        asset,
    }
}

/// Deploie le vault : le constructeur pose et valide la configuration dans
/// la transaction meme du deploiement.
fn deploy<'a>(b: Base<'a>, strategies: Vec<StrategyConfig>, drift_bps: u32) -> Bench<'a> {
    let id = b.env.register(
        YieldVault,
        (
            b.admin.clone(),
            b.admin.clone(),
            b.asset.clone(),
            strategies,
            drift_bps,
        ),
    );
    Bench {
        vault: YieldVaultClient::new(&b.env, &id),
        env: b.env,
        admin: b.admin,
        user: b.user,
        asset: b.asset,
        token: b.token,
    }
}

/// Message du deploiement refuse. Le SDK n'offre pas de `try_register` : on
/// capture la panique du constructeur. L'hote la rapporte en
/// `Error(Context, InvalidAction)` et range l'erreur typee du vault dans son
/// journal de diagnostic, que le message de panique reproduit.
fn deploy_error(b: Base, strategies: Vec<StrategyConfig>, drift_bps: u32) -> std::string::String {
    let payload = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        deploy(b, strategies, drift_bps);
    }))
    .expect_err("le deploiement devait echouer");
    payload
        .downcast_ref::<std::string::String>()
        .cloned()
        .or_else(|| {
            payload
                .downcast_ref::<&str>()
                .map(|s| std::string::String::from(*s))
        })
        .unwrap_or_default()
}

fn assert_rejected(b: Base, strategies: Vec<StrategyConfig>, drift_bps: u32, err: VaultError) {
    let message = deploy_error(b, strategies, drift_bps);
    let expected = std::format!(
        "\"constructor invocation has failed with error\", Error(Contract, #{})",
        err as u32
    );
    assert!(
        message.contains(&expected),
        "attendu {expected}, obtenu {message}"
    );
}

fn strategy(env: &Env, asset: &Address) -> Address {
    let args: Vec<Val> = Vec::new(env);
    env.register(MockStrategy, (asset.clone(), args))
}

fn config(address: &Address, target_bps: u32, cap_bps: u32) -> StrategyConfig {
    StrategyConfig {
        address: address.clone(),
        protocol: symbol_short!("mock"),
        target_bps,
        cap_bps,
        active: true,
    }
}

/// Vault deploye sur deux strategies factices (cibles 70 % et 20 %).
fn bench_with_two<'a>() -> (Bench<'a>, Address, Address) {
    let b = base();
    let s1 = strategy(&b.env, &b.asset);
    let s2 = strategy(&b.env, &b.asset);
    let list = vec![&b.env, config(&s1, 7_000, 9_000), config(&s2, 2_000, 3_000)];
    (deploy(b, list, 500), s1, s2)
}

/// Deplace des fonds oisifs du vault vers une strategie, comme le fera le
/// reequilibrage (etape suivante du plan).
fn allocate(b: &Bench, to: &Address, amount: i128) {
    b.token.transfer(&b.vault.address, to, &amount);
}

#[test]
fn constructor_stores_the_configuration() {
    let (b, s1, s2) = bench_with_two();
    assert_eq!(
        b.vault.strategies(),
        vec![&b.env, config(&s1, 7_000, 9_000), config(&s2, 2_000, 3_000)]
    );
    assert_eq!(b.vault.keeper(), b.admin);
    assert_eq!(b.vault.drift_bps(), 500);
}

#[test]
fn constructor_rejects_more_than_max_strategies() {
    let b = base();
    let mut list = Vec::new(&b.env);
    for _ in 0..=MAX_STRATEGIES {
        list.push_back(config(&strategy(&b.env, &b.asset), 0, 0));
    }
    assert_rejected(b, list, 500, VaultError::TooManyStrategies);
}

#[test]
fn constructor_rejects_a_strategy_on_another_asset() {
    let b = base();
    let other = b
        .env
        .register_stellar_asset_contract_v2(b.admin.clone())
        .address();
    let list = vec![&b.env, config(&strategy(&b.env, &other), 1_000, 2_000)];
    assert_rejected(b, list, 500, VaultError::StrategyAssetMismatch);
}

#[test]
fn constructor_rejects_a_duplicate_strategy() {
    let b = base();
    let s = strategy(&b.env, &b.asset);
    let list = vec![&b.env, config(&s, 1_000, 2_000), config(&s, 1_000, 2_000)];
    assert_rejected(b, list, 500, VaultError::DuplicateStrategy);
}

#[test]
fn constructor_rejects_invalid_allocations() {
    // Un environnement neuf par cas : un deploiement refuse ne doit rien
    // laisser derriere lui qui fausse le cas suivant.
    for case in 0..7 {
        let b = base();
        let s1 = strategy(&b.env, &b.asset);
        let s2 = strategy(&b.env, &b.asset);
        let (list, drift) = match case {
            // strategie inactive des l'origine
            0 => {
                let mut inactive = config(&s1, 1_000, 2_000);
                inactive.active = false;
                (vec![&b.env, inactive], 500)
            }
            // cible au-dessus du plafond
            1 => (vec![&b.env, config(&s1, 3_000, 2_000)], 500),
            // plafond au-dessus de 100 %
            2 => (vec![&b.env, config(&s1, 1_000, 10_001)], 500),
            // somme des cibles au-dessus de 100 %
            3 => (
                vec![
                    &b.env,
                    config(&s1, 6_000, 10_000),
                    config(&s2, 5_000, 10_000),
                ],
                500,
            ),
            // declencheur nul ou au-dessus de 100 %
            4 => (Vec::new(&b.env), 0),
            // declencheur sous la perte toleree par lot : les pertes se
            // composeraient de lot en lot
            5 => (Vec::new(&b.env), 100),
            _ => (Vec::new(&b.env), 10_001),
        };
        assert_rejected(b, list, drift, VaultError::InvalidAllocation);
    }
}

#[test]
fn constructor_accepts_targets_summing_to_exactly_100_percent() {
    let b = base();
    let s1 = strategy(&b.env, &b.asset);
    let s2 = strategy(&b.env, &b.asset);
    let list = vec![
        &b.env,
        config(&s1, 6_000, 10_000),
        config(&s2, 4_000, 10_000),
    ];
    let b = deploy(b, list, 10_000);
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
    let b = base();
    let s = strategy(&b.env, &b.asset);
    let client = defindex_strategy_core::DeFindexStrategyClient::new(&b.env, &s);
    assert_eq!(client.deposit(&1_0000000, &b.user), 1_0000000);
    assert_eq!(client.balance(&b.user), 1_0000000);
}

#[test]
fn constructor_rejects_a_strategy_whose_balance_cannot_be_read() {
    let b = base();
    let broken = strategy(&b.env, &b.asset);
    MockStrategyClient::new(&b.env, &broken).break_balance();
    let list = vec![&b.env, config(&broken, 1_000, 2_000)];
    // Panique de la strategie elle-meme, pas une erreur du vault.
    assert!(!deploy_error(b, list, 500).is_empty());

    let b = base();
    let negative = strategy(&b.env, &b.asset);
    MockStrategyClient::new(&b.env, &negative).set_balance_override(&-1);
    let list = vec![&b.env, config(&negative, 1_000, 2_000)];
    assert_rejected(b, list, 500, VaultError::InvalidStrategyBalance);
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

// --- Reequilibrage (etape 4) ---

fn deploy_with_keeper<'a>(
    b: Base<'a>,
    keeper: &Address,
    strategies: Vec<StrategyConfig>,
) -> Bench<'a> {
    let id = b.env.register(
        YieldVault,
        (
            b.admin.clone(),
            keeper.clone(),
            b.asset.clone(),
            strategies,
            500u32,
        ),
    );
    Bench {
        vault: YieldVaultClient::new(&b.env, &id),
        env: b.env,
        admin: b.admin,
        user: b.user,
        asset: b.asset,
        token: b.token,
    }
}

#[test]
fn rebalance_brings_every_pocket_to_its_target_in_one_batch() {
    let (b, s1, s2) = bench_with_two();
    b.vault.deposit(&b.user, &100_0000000);

    let moves = b.vault.rebalance(&b.admin);

    assert_eq!(
        moves,
        vec![
            &b.env,
            (s1.clone(), 70_0000000i128),
            (s2.clone(), 20_0000000i128)
        ]
    );
    assert_eq!(b.token.balance(&s1), 70_0000000);
    assert_eq!(b.token.balance(&s2), 20_0000000);
    assert_eq!(b.token.balance(&b.vault.address), 10_0000000);
    assert_eq!(b.vault.total_assets(), 100_0000000);
}

#[test]
fn rebalance_waits_for_a_pocket_to_drift_beyond_the_trigger() {
    let (b, _s1, _s2) = bench_with_two();
    b.vault.deposit(&b.user, &100_0000000);
    b.vault.rebalance(&b.admin);

    // Oisif 12 pour une cible de 10,2 : ecart de 1,8 point, sous les 5.
    b.vault.deposit(&b.user, &2_0000000);
    let err = b.vault.try_rebalance(&b.admin).unwrap_err().unwrap();
    assert_eq!(err, VaultError::NothingToRebalance.into());

    // Oisif 22 pour une cible de 11,2 : ecart de 9,6 points, au-dessus.
    b.vault.deposit(&b.user, &10_0000000);
    b.vault.rebalance(&b.admin);
    assert_eq!(b.token.balance(&b.vault.address), 11_2000000);
}

#[test]
fn rebalance_on_an_empty_vault_has_nothing_to_do() {
    let (b, _s1, _s2) = bench_with_two();
    let err = b.vault.try_rebalance(&b.admin).unwrap_err().unwrap();
    assert_eq!(err, VaultError::NothingToRebalance.into());
}

#[test]
fn rebalance_withdraws_from_overweight_strategies_before_feeding_others() {
    let (b, s1, s2) = bench_with_two();
    b.vault.deposit(&b.user, &100_0000000);
    b.vault.rebalance(&b.admin);

    // Inversion des poids, sous les plafonds (90 % et 30 %) : s1 20, s2 30.
    b.vault.set_targets(&vec![&b.env, 2_000u32, 3_000u32]);
    let moves = b.vault.rebalance(&b.admin);

    assert_eq!(
        moves,
        vec![
            &b.env,
            (s1.clone(), -50_0000000i128),
            (s2.clone(), 10_0000000i128)
        ]
    );
    assert_eq!(b.token.balance(&s1), 20_0000000);
    assert_eq!(b.token.balance(&s2), 30_0000000);
    assert_eq!(b.token.balance(&b.vault.address), 50_0000000);
}

#[test]
fn rebalance_is_reserved_to_the_keeper_and_the_admin() {
    let b = base();
    let keeper = Address::generate(&b.env);
    let outsider = Address::generate(&b.env);
    let s = strategy(&b.env, &b.asset);
    let list = vec![&b.env, config(&s, 5_000, 6_000)];
    let b = deploy_with_keeper(b, &keeper, list);
    b.vault.deposit(&b.user, &100_0000000);

    let err = b.vault.try_rebalance(&outsider).unwrap_err().unwrap();
    assert_eq!(err, VaultError::NotAuthorized.into());

    b.vault.rebalance(&keeper);
    assert_eq!(b.token.balance(&s), 50_0000000);

    b.vault.set_targets(&vec![&b.env, 0u32]);
    b.vault.rebalance(&b.admin);
    assert_eq!(b.token.balance(&s), 0);
}

#[test]
fn rebalance_is_blocked_by_the_pause() {
    let (b, _s1, _s2) = bench_with_two();
    b.vault.deposit(&b.user, &100_0000000);
    b.vault.pause();
    let err = b.vault.try_rebalance(&b.admin).unwrap_err().unwrap();
    assert_eq!(err, VaultError::ContractPaused.into());
}

#[test]
fn rebalance_tolerates_an_entry_cost_within_the_margin() {
    let (b, s1, _s2) = bench_with_two();
    b.vault.deposit(&b.user, &100_0000000);

    // 1 % de frais sur 70 verses : 0,7 de perte pour 90 deplaces, sous 1 %.
    MockStrategyClient::new(&b.env, &s1).set_deposit_fee_bps(&100);
    b.vault.rebalance(&b.admin);
    assert_eq!(b.vault.total_assets(), 99_3000000);
}

#[test]
fn a_rebalance_losing_more_than_the_margin_reverts_entirely() {
    let (b, s1, s2) = bench_with_two();
    b.vault.deposit(&b.user, &100_0000000);
    // 2 % sur 70 : 1,4 de perte pour 90 deplaces, au-dessus de 1 %.
    MockStrategyClient::new(&b.env, &s1).set_deposit_fee_bps(&200);

    let err = b.vault.try_rebalance(&b.admin).unwrap_err().unwrap();
    assert_eq!(err, VaultError::RebalanceLoss.into());
    // Revert du lot entier : rien n'a bouge.
    assert_eq!(b.token.balance(&b.vault.address), 100_0000000);
    assert_eq!(b.token.balance(&s1), 0);
    assert_eq!(b.token.balance(&s2), 0);
}

#[test]
fn a_strategy_overstating_its_value_beyond_its_cap_reverts_the_batch() {
    let b = base();
    let s = strategy(&b.env, &b.asset);
    // Cible au plafond : la moindre surevaluation le crève.
    let list = vec![&b.env, config(&s, 7_000, 7_000)];
    let b = deploy(b, list, 500);
    b.vault.deposit(&b.user, &100_0000000);
    MockStrategyClient::new(&b.env, &s).set_inflate_bps(&10_000);

    let err = b.vault.try_rebalance(&b.admin).unwrap_err().unwrap();
    assert_eq!(err, VaultError::CapExceeded.into());
}

#[test]
fn set_targets_stays_under_the_caps_and_100_percent() {
    let (b, _s1, _s2) = bench_with_two();
    let cases = [
        vec![&b.env, 9_001u32, 0u32],     // au-dessus du plafond de s1
        vec![&b.env, 8_000u32, 3_000u32], // somme au-dessus de 100 %
        vec![&b.env, 1_000u32],           // longueur differente
    ];
    for targets in cases {
        let err = b.vault.try_set_targets(&targets).unwrap_err().unwrap();
        assert_eq!(err, VaultError::InvalidAllocation.into());
    }
    b.vault.set_targets(&vec![&b.env, 9_000u32, 1_000u32]);
    assert_eq!(b.vault.strategies().get(0).unwrap().target_bps, 9_000);
}

// --- Sortie d'urgence ---

#[test]
fn emergency_exit_recovers_everything_and_retires_the_strategy() {
    let (b, s1, s2) = bench_with_two();
    b.vault.deposit(&b.user, &100_0000000);
    b.vault.rebalance(&b.admin);
    b.vault.pause();

    // En pause : la sortie d'urgence passe quand tout le reste est gele.
    let recovered = b.vault.emergency_exit(&s1, &false);

    assert_eq!(recovered, 70_0000000);
    assert_eq!(b.token.balance(&s1), 0);
    let config = b.vault.strategies().get(0).unwrap();
    assert!(!config.active);
    assert_eq!(config.target_bps, 0);
    assert_eq!(b.vault.total_assets(), 100_0000000);
    assert_eq!(
        b.vault.allocation(),
        vec![&b.env, (s1.clone(), 0i128), (s2.clone(), 20_0000000i128)]
    );

    // Plus jamais alimentee : sa cible tombee a 0, l'oisif (80) est a sa
    // nouvelle cible et rien ne bouge. Un nouveau depot qui ecarte s2 de sa
    // cible relance le lot, sans rien verser a s1.
    b.vault.unpause();
    let err = b.vault.try_rebalance(&b.admin).unwrap_err().unwrap();
    assert_eq!(err, VaultError::NothingToRebalance.into());
    b.vault.deposit(&b.user, &100_0000000);
    b.vault.rebalance(&b.admin);
    assert_eq!(b.token.balance(&s1), 0);
    assert_eq!(b.token.balance(&s2), 40_0000000);
    // Et une cible non nulle lui est refusee.
    let err = b
        .vault
        .try_set_targets(&vec![&b.env, 1_000u32, 2_000u32])
        .unwrap_err()
        .unwrap();
    assert_eq!(err, VaultError::InvalidAllocation.into());
    // Et on ne sort pas deux fois.
    let err = b
        .vault
        .try_emergency_exit(&s1, &false)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, VaultError::StrategyNotFound.into());
}

#[test]
fn emergency_exit_without_write_off_changes_nothing_if_the_strategy_cannot_pay() {
    let (b, s1, _s2) = bench_with_two();
    b.vault.deposit(&b.user, &100_0000000);
    b.vault.rebalance(&b.admin);
    MockStrategyClient::new(&b.env, &s1).break_withdraw();

    // Pool gele : les fonds sont immobilises, pas perdus. On ne les passe pas
    // en perte sans le dire.
    assert!(b.vault.try_emergency_exit(&s1, &false).is_err());
    assert!(b.vault.strategies().get(0).unwrap().active);
    assert_eq!(b.vault.total_assets(), 100_0000000);
}

#[test]
fn a_write_off_retires_a_broken_strategy_and_unblocks_the_vault() {
    let (b, s1, _s2) = bench_with_two();
    b.vault.deposit(&b.user, &100_0000000);
    b.vault.rebalance(&b.admin);
    MockStrategyClient::new(&b.env, &s1).break_balance();

    // Strategie illisible : total_assets, donc depots et retraits, bloques.
    assert!(b.vault.try_withdraw(&b.user, &1_0000000).is_err());

    // Le passage en perte se decide vault gele.
    b.vault.pause();
    let recovered = b.vault.emergency_exit(&s1, &true);
    b.vault.unpause();

    assert_eq!(recovered, 0);
    // Les 70 sont passes en perte : le vault repart sur 30.
    assert_eq!(b.vault.total_assets(), 30_0000000);
    let before = b.token.balance(&b.user);
    b.vault.withdraw(&b.user, &10_0000000);
    assert_eq!(b.token.balance(&b.user) - before, 3_0000000);
}

#[test]
fn emergency_exit_rejects_an_unknown_strategy() {
    let (b, _s1, _s2) = bench_with_two();
    let unknown = strategy(&b.env, &b.asset);
    let err = b
        .vault
        .try_emergency_exit(&unknown, &false)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, VaultError::StrategyNotFound.into());
}

#[test]
fn the_loss_margin_holds_exactly_at_1_percent_of_the_volume_moved() {
    for (fee_bps, passes) in [(100_i128, true), (101, false)] {
        let b = base();
        let s = strategy(&b.env, &b.asset);
        let list = vec![&b.env, config(&s, 7_000, 9_000)];
        let b = deploy(b, list, 500);
        b.vault.deposit(&b.user, &100_0000000);
        MockStrategyClient::new(&b.env, &s).set_deposit_fee_bps(&fee_bps);
        // 70 deplaces : 1 % = 0,7 de perte admise, pas une unite de plus.
        let result = b.vault.try_rebalance(&b.admin);
        assert_eq!(result.is_ok(), passes, "frais de {fee_bps} pb");
    }
}

#[test]
fn a_rotation_counts_the_volume_moved_once_not_twice() {
    let b = base();
    let s1 = strategy(&b.env, &b.asset);
    let s2 = strategy(&b.env, &b.asset);
    let list = vec![&b.env, config(&s1, 7_000, 9_000), config(&s2, 0, 9_000)];
    let b = deploy(b, list, 500);
    b.vault.deposit(&b.user, &100_0000000);
    b.vault.rebalance(&b.admin);

    // Rotation complete de s1 vers s2 avec 1,5 % de frais d'entree : 1,05 de
    // perte pour 70 deplaces. Comptes deux fois (140), ils passeraient.
    b.vault.set_targets(&vec![&b.env, 0u32, 7_000u32]);
    MockStrategyClient::new(&b.env, &s2).set_deposit_fee_bps(&150);
    let err = b.vault.try_rebalance(&b.admin).unwrap_err().unwrap();
    assert_eq!(err, VaultError::RebalanceLoss.into());
}

#[test]
fn a_strategy_keeping_part_of_a_rebalance_withdrawal_loses_nothing() {
    let (b, s1, s2) = bench_with_two();
    b.vault.deposit(&b.user, &100_0000000);
    b.vault.rebalance(&b.admin);
    b.vault.set_targets(&vec![&b.env, 2_000u32, 3_000u32]);
    // s1 ne rend que 45 des 50 demandes, en annoncant 50 : les 5 restent
    // chez elle, toujours comptes. Le vault rapporte ce qui est rentre,
    // aucune valeur ne se perd.
    MockStrategyClient::new(&b.env, &s1).set_skew_bps(&-1_000);
    let moves = b.vault.rebalance(&b.admin);

    assert_eq!(
        moves,
        vec![
            &b.env,
            (s1.clone(), -45_0000000i128),
            (s2.clone(), 10_0000000i128)
        ]
    );
    assert_eq!(b.token.balance(&s1), 25_0000000);
    assert_eq!(b.vault.total_assets(), 100_0000000);
}

#[test]
fn a_tolerated_loss_does_not_trigger_the_next_batch() {
    let (b, s1, _s2) = bench_with_two();
    b.vault.deposit(&b.user, &100_0000000);
    MockStrategyClient::new(&b.env, &s1).set_deposit_fee_bps(&100);
    b.vault.rebalance(&b.admin);
    // L'ecart laisse par la perte (moins de 1 point) reste sous le
    // declencheur (5 points) : les pertes ne se composent pas.
    let err = b.vault.try_rebalance(&b.admin).unwrap_err().unwrap();
    assert_eq!(err, VaultError::NothingToRebalance.into());
}

#[test]
fn the_cap_holds_for_a_strategy_whose_target_sits_below_it() {
    let b = base();
    let s = strategy(&b.env, &b.asset);
    let list = vec![&b.env, config(&s, 5_000, 6_000)];
    let b = deploy(b, list, 500);
    b.vault.deposit(&b.user, &100_0000000);
    // 50 verses annonces 100 : 100 sur 150 au total, au-dessus des 60 %.
    MockStrategyClient::new(&b.env, &s).set_inflate_bps(&10_000);
    let err = b.vault.try_rebalance(&b.admin).unwrap_err().unwrap();
    assert_eq!(err, VaultError::CapExceeded.into());
}

#[test]
fn emergency_exit_without_write_off_refuses_a_partial_return() {
    let (b, s1, _s2) = bench_with_two();
    b.vault.deposit(&b.user, &100_0000000);
    b.vault.rebalance(&b.admin);
    // La strategie ne rend que 70 % de ce qu'elle annonce.
    MockStrategyClient::new(&b.env, &s1).set_skew_bps(&-3_000);

    let err = b
        .vault
        .try_emergency_exit(&s1, &false)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, VaultError::StrategyShortfall.into());
    assert!(b.vault.strategies().get(0).unwrap().active);
    assert_eq!(b.token.balance(&s1), 70_0000000);
}

#[test]
fn emergency_exit_without_write_off_holds_the_margin_at_one_percent() {
    // 1 % retenu : dans la marge, la sortie passe ; 1,01 % : refusee.
    for (skew, accepted) in [(-100, true), (-101, false)] {
        let (b, s1, _s2) = bench_with_two();
        b.vault.deposit(&b.user, &100_0000000);
        b.vault.rebalance(&b.admin);
        MockStrategyClient::new(&b.env, &s1).set_skew_bps(&skew);
        let result = b.vault.try_emergency_exit(&s1, &false);
        assert_eq!(result.is_ok(), accepted, "retenue de {skew} pb");
    }
}

#[test]
fn a_write_off_of_a_frozen_strategy_retires_it_with_nothing_recovered() {
    let (b, s1, _s2) = bench_with_two();
    b.vault.deposit(&b.user, &100_0000000);
    b.vault.rebalance(&b.admin);
    MockStrategyClient::new(&b.env, &s1).break_withdraw();
    b.vault.pause();

    assert_eq!(b.vault.emergency_exit(&s1, &true), 0);
    assert!(!b.vault.strategies().get(0).unwrap().active);
    assert_eq!(b.vault.total_assets(), 30_0000000);
}

#[test]
fn the_keeper_can_be_replaced_by_the_admin() {
    let (b, _s1, _s2) = bench_with_two();
    let keeper = Address::generate(&b.env);
    b.vault.set_keeper(&keeper);
    assert_eq!(b.vault.keeper(), keeper);
    b.vault.deposit(&b.user, &100_0000000);
    b.vault.rebalance(&keeper);
}

#[test]
fn admin_functions_reject_anyone_else() {
    let (b, s1, _s2) = bench_with_two();
    let outsider = Address::generate(&b.env);
    // Seule l'auth de l'inconnu est fournie : celle de l'admin manque.
    let targets = vec![&b.env, 1_000u32, 1_000u32];
    b.env.mock_auths(&[MockAuth {
        address: &outsider,
        invoke: &MockAuthInvoke {
            contract: &b.vault.address,
            fn_name: "set_targets",
            args: (targets.clone(),).into_val(&b.env),
            sub_invokes: &[],
        },
    }]);
    assert!(b.vault.try_set_targets(&targets).is_err());

    b.env.mock_auths(&[MockAuth {
        address: &outsider,
        invoke: &MockAuthInvoke {
            contract: &b.vault.address,
            fn_name: "set_keeper",
            args: (outsider.clone(),).into_val(&b.env),
            sub_invokes: &[],
        },
    }]);
    assert!(b.vault.try_set_keeper(&outsider).is_err());

    b.env.mock_auths(&[MockAuth {
        address: &outsider,
        invoke: &MockAuthInvoke {
            contract: &b.vault.address,
            fn_name: "emergency_exit",
            args: (s1.clone(), false).into_val(&b.env),
            sub_invokes: &[],
        },
    }]);
    assert!(b.vault.try_emergency_exit(&s1, &false).is_err());

    // Rien n'a change.
    assert_eq!(b.vault.keeper(), b.admin);
    assert!(b.vault.strategies().get(0).unwrap().active);
    assert_eq!(b.vault.strategies().get(0).unwrap().target_bps, 7_000);
}

#[test]
fn a_target_at_its_cap_survives_a_tolerated_loss_elsewhere() {
    let b = base();
    let s1 = strategy(&b.env, &b.asset);
    let s2 = strategy(&b.env, &b.asset);
    let list = vec![&b.env, config(&s1, 7_000, 7_000), config(&s2, 2_000, 3_000)];
    let b = deploy(b, list, 500);
    b.vault.deposit(&b.user, &100_0000000);
    // 1 % de frais a l'entree de s2 : total final 99,8. Rapportes a ce total,
    // les 70 de s1 creveraient son plafond de 70 % ; le lot est sain.
    MockStrategyClient::new(&b.env, &s2).set_deposit_fee_bps(&100);
    b.vault.rebalance(&b.admin);
    assert_eq!(b.token.balance(&s1), 70_0000000);
}

// --- Cout de sortie a la charge du client sortant (decision du 20/10) ---

/// Vault a une strategie (cible 90 %), 100 deposes et alloues : 10 oisifs,
/// 90 dans la strategie.
fn bench_with_one_allocated<'a>() -> (Bench<'a>, Address) {
    let b = base();
    let s = strategy(&b.env, &b.asset);
    let list = vec![&b.env, config(&s, 9_000, 9_000)];
    let b = deploy(b, list, 500);
    b.vault.deposit(&b.user, &100_0000000);
    b.vault.rebalance(&b.admin);
    (b, s)
}

#[test]
fn the_exiting_client_bears_an_exit_cost_within_the_margin() {
    let (b, s) = bench_with_one_allocated();
    // La strategie rend 0,5 % de moins que demande, comme un pool LP qui
    // preleve ses frais et son glissement a la sortie.
    MockStrategyClient::new(&b.env, &s).set_skew_bps(&-50);

    let before = b.token.balance(&b.user);
    let paid = b.vault.withdraw(&b.user, &50_0000000);
    // all() ne garde que la derniere invocation : capturer avant toute
    // lecture.
    let events = b.env.events().all().filter_by_contract(&b.vault.address);

    // 10 d'oisif, 40 demandes a la strategie, 39,8 rentres : le client
    // recoit 49,8 et c'est ce que withdraw annonce.
    assert_eq!(paid, 49_8000000);
    assert_eq!(b.token.balance(&b.user) - before, 49_8000000);
    assert_eq!(b.token.balance(&b.vault.address), 0);

    // Les events portent l'etat reel apres le cout de sortie, calcule sans
    // relire les strategies.
    let total_assets = b.vault.total_assets();
    let position_after = b.vault.allocation().get(0).unwrap().1;
    assert_eq!(
        events,
        [
            DeallocateEvent {
                strategy: s,
                protocol: symbol_short!("mock"),
                assets: 39_8000000,
                position_after,
                decided_by: b.user.clone(),
                algo: None,
                reason: symbol_short!("redeem"),
                seq: 3,
                total_assets,
            }
            .to_xdr(&b.env, &b.vault.address),
            WithdrawEvent {
                owner: b.user.clone(),
                shares: 50_0000000,
                assets: 49_8000000,
                seq: 4,
                total_assets,
                total_shares: b.vault.total_shares(),
            }
            .to_xdr(&b.env, &b.vault.address),
        ]
    );
}

#[test]
fn the_exit_cost_margin_holds_exactly_at_1_percent() {
    for (skew, passes) in [(-100_i32, true), (-101, false)] {
        let (b, s) = bench_with_one_allocated();
        MockStrategyClient::new(&b.env, &s).set_skew_bps(&skew);
        let shares = b.vault.shares_of(&b.user);
        let result = b.vault.try_withdraw(&b.user, &50_0000000);
        assert_eq!(result.is_ok(), passes, "ecart de {skew} pb");
        if !passes {
            assert_eq!(
                result.unwrap_err().unwrap(),
                VaultError::StrategyShortfall.into()
            );
            assert_eq!(b.vault.shares_of(&b.user), shares);
        }
    }
}

#[test]
fn emergency_exit_accepts_an_exit_cost_within_the_margin() {
    let (b, s) = bench_with_one_allocated();
    MockStrategyClient::new(&b.env, &s).set_skew_bps(&-50);
    // 90 dus, 89,55 rentres : dans la marge, la sortie passe sans write_off.
    assert_eq!(b.vault.emergency_exit(&s, &false), 89_5500000);
    assert!(!b.vault.strategies().get(0).unwrap().active);
}

// --- Events du schema D6a ---

#[test]
fn rebalance_records_each_move_as_an_algorithm_decision() {
    let (b, s1, s2) = bench_with_two();
    b.vault.deposit(&b.user, &100_0000000);

    b.vault.rebalance(&b.admin);

    let algo = Some(Symbol::new(&b.env, "fy_alloc_v1"));
    let allocate = |strategy: &Address, assets: i128, seq: u64| {
        AllocateEvent {
            strategy: strategy.clone(),
            protocol: symbol_short!("mock"),
            assets,
            position_after: assets,
            decided_by: b.admin.clone(),
            algo: algo.clone(),
            reason: symbol_short!("drift"),
            seq,
            total_assets: 100_0000000,
        }
        .to_xdr(&b.env, &b.vault.address)
    };
    assert_eq!(
        b.env.events().all().filter_by_contract(&b.vault.address),
        [allocate(&s1, 70_0000000, 2), allocate(&s2, 20_0000000, 3)]
    );
}

#[test]
fn a_withdrawal_served_by_a_strategy_records_a_client_redeem() {
    let (b, s1, _s2) = bench_with_two();
    b.vault.deposit(&b.user, &100_0000000);
    allocate(&b, &s1, 70_0000000);

    // 50 : l'oisif (30) puis 20 tires de s1.
    b.vault.withdraw(&b.user, &50_0000000);

    assert_eq!(
        b.env.events().all().filter_by_contract(&b.vault.address),
        [
            DeallocateEvent {
                strategy: s1.clone(),
                protocol: symbol_short!("mock"),
                assets: 20_0000000,
                position_after: 50_0000000,
                decided_by: b.user.clone(),
                algo: None,
                reason: symbol_short!("redeem"),
                seq: 2,
                total_assets: 50_0000000,
            }
            .to_xdr(&b.env, &b.vault.address),
            WithdrawEvent {
                owner: b.user.clone(),
                shares: 50_0000000,
                assets: 50_0000000,
                seq: 3,
                total_assets: 50_0000000,
                total_shares: 50_0000000,
            }
            .to_xdr(&b.env, &b.vault.address),
        ]
    );
}

#[test]
fn a_write_off_requires_the_pause_and_records_what_was_lost() {
    let (b, s1, s2) = bench_with_two();
    b.vault.deposit(&b.user, &100_0000000);
    b.vault.rebalance(&b.admin);
    MockStrategyClient::new(&b.env, &s1).break_withdraw();

    let err = b.vault.try_emergency_exit(&s1, &true).unwrap_err().unwrap();
    assert_eq!(err, VaultError::WriteOffRequiresPause.into());

    b.vault.pause();
    b.vault.emergency_exit(&s1, &true);

    // Pool gele : la strategie annoncait 70, rien n'est rentre.
    assert_eq!(
        b.env.events().all().filter_by_contract(&b.vault.address),
        [EmergencyExitEvent {
            strategy: s1,
            position_before: Some(70_0000000),
            assets_recovered: 0,
            written_off: Some(70_0000000),
            write_off: true,
            decided_by: b.admin.clone(),
            seq: 5,
            total_assets: 30_0000000,
        }
        .to_xdr(&b.env, &b.vault.address)]
    );

    // Strategie illisible : la valeur d'avant est inconnue, la perte aussi.
    MockStrategyClient::new(&b.env, &s2).break_balance();
    b.vault.emergency_exit(&s2, &true);
    assert_eq!(
        b.env.events().all().filter_by_contract(&b.vault.address),
        [EmergencyExitEvent {
            strategy: s2,
            position_before: None,
            assets_recovered: 0,
            written_off: None,
            write_off: true,
            decided_by: b.admin.clone(),
            seq: 6,
            total_assets: 10_0000000,
        }
        .to_xdr(&b.env, &b.vault.address)]
    );
}

#[test]
fn an_emergency_exit_without_write_off_records_its_exit_cost() {
    let (b, s1, _s2) = bench_with_two();
    b.vault.deposit(&b.user, &100_0000000);
    b.vault.rebalance(&b.admin);
    // 0,5 % retenu a la sortie, dans la marge.
    MockStrategyClient::new(&b.env, &s1).set_skew_bps(&-50);

    b.vault.emergency_exit(&s1, &false);

    assert_eq!(
        b.env.events().all().filter_by_contract(&b.vault.address),
        [EmergencyExitEvent {
            strategy: s1,
            position_before: Some(70_0000000),
            assets_recovered: 69_6500000,
            written_off: Some(3500000),
            write_off: false,
            decided_by: b.admin.clone(),
            seq: 4,
            total_assets: 99_6500000,
        }
        .to_xdr(&b.env, &b.vault.address)]
    );
}

#[test]
fn set_targets_records_the_previous_targets() {
    let (b, _s1, _s2) = bench_with_two();

    b.vault.set_targets(&vec![&b.env, 5_000u32, 3_000u32]);

    assert_eq!(
        b.env.events().all().filter_by_contract(&b.vault.address),
        [TargetsSetEvent {
            old: vec![&b.env, 7_000u32, 2_000u32],
            new: vec![&b.env, 5_000u32, 3_000u32],
            admin: b.admin.clone(),
            seq: 1,
        }
        .to_xdr(&b.env, &b.vault.address)]
    );
}

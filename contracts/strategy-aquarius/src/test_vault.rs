#![cfg(test)]
// Montants en convention Stellar 7 decimales (X_XXXXXXX, ex. 0_1000000 = 0,1).
#![allow(clippy::inconsistent_digit_grouping, clippy::zero_prefixed_literal)]
//! Le vault D5 sur la strategie Aquarius, pile Aquarius reelle. Seules les
//! signatures de l'appelant de tete (porteur, keeper, admin) sont fournies :
//! toute la chaine vault -> strategie -> router -> pool s'autorise elle-meme.

use super::test::{setup, Bench, PRICE};
use soroban_sdk::{
    testutils::{Address as _, Ledger, MockAuth, MockAuthInvoke},
    token::StellarAssetClient,
    vec, Address, IntoVal, Vec,
};
use yield_vault::{StrategyConfig, YieldVault, YieldVaultClient};

fn vault_on_aquarius<'a>(b: &Bench<'a>) -> YieldVaultClient<'a> {
    let list = vec![
        &b.env,
        StrategyConfig {
            address: b.strategy.address.clone(),
            target_bps: 2_000,
            cap_bps: 3_000,
            active: true,
        },
    ];
    let id = b.env.register(
        YieldVault,
        (
            b.admin.clone(),
            b.keeper.clone(),
            b.usdc.address.clone(),
            list,
            500u32,
        ),
    );
    YieldVaultClient::new(&b.env, &id)
}

/// Seule l'auth de `who` sur l'appel de tete, sans sous-appel : tout ce que
/// le vault et la strategie font ensuite doit etre pre-autorise par eux.
fn only(b: &Bench, who: &Address, contract: &Address, fn_name: &str, args: Vec<soroban_sdk::Val>) {
    b.env.mock_auths(&[MockAuth {
        address: who,
        invoke: &MockAuthInvoke {
            contract,
            fn_name,
            args,
            sub_invokes: &[],
        },
    }]);
}

/// Depot de 100 dans le vault (sous mock_all_auths : ce chemin est D1) puis
/// lot du keeper avec sa seule signature : 20 partent dans Aquarius.
fn funded<'a>(b: &Bench<'a>) -> YieldVaultClient<'a> {
    let vault = vault_on_aquarius(b);
    vault.deposit(&b.user, &100_0000000);
    only(
        b,
        &b.keeper,
        &vault.address,
        "rebalance",
        (b.keeper.clone(),).into_val(&b.env),
    );
    vault.rebalance(&b.keeper);
    b.env.mock_all_auths();
    vault
}

#[test]
fn a_keeper_rebalance_reaches_the_aquarius_pool() {
    let b = setup();
    let vault = funded(&b);

    let invested = b.strategy.balance(&vault.address);
    // 20 verses, moins les parts mortes et le cout d'entree (bien sous 1 %).
    assert!(invested > 19_8000000 && invested < 20_0000000, "{invested}");
    assert_eq!(b.usdc.balance(&vault.address), 80_0000000);
    assert!(vault.total_assets() > 99_8000000);
}

#[test]
fn a_withdrawal_through_aquarius_pays_the_leaver_within_the_margin() {
    let b = setup();
    let vault = funded(&b);
    let before = b.usdc.balance(&b.user);

    // 95 parts : 80 d'oisif, le reste sort du pool. Le client paie la sortie,
    // dans la marge de 1 % de ce qui est tire de la strategie.
    let shares = 95_0000000;
    let due = shares * vault.total_assets() / vault.total_shares();
    only(
        &b,
        &b.user,
        &vault.address,
        "withdraw",
        (b.user.clone(), shares).into_val(&b.env),
    );
    let paid = vault.withdraw(&b.user, &shares);
    b.env.mock_all_auths();

    assert_eq!(b.usdc.balance(&b.user) - before, paid);
    assert!(paid <= due, "{paid} pour {due}");
    // Le manque ne porte que sur les ~15 tires de la strategie.
    assert!(
        (due - paid) * 100 <= (due - 80_0000000),
        "{paid} pour {due}"
    );
}

#[test]
fn emergency_exit_brings_the_lp_position_home_within_the_margin() {
    let b = setup();
    let vault = funded(&b);
    let invested = b.strategy.balance(&vault.address);

    only(
        &b,
        &b.admin,
        &vault.address,
        "emergency_exit",
        (b.strategy.address.clone(), false).into_val(&b.env),
    );
    let recovered = vault.emergency_exit(&b.strategy.address, &false);
    b.env.mock_all_auths();

    assert!(
        recovered * 100 >= invested * 99,
        "{recovered} pour {invested}"
    );
    assert!(!vault.strategies().get(0).unwrap().active);
}

#[test]
fn a_silent_keeper_is_covered_by_the_admin_publishing_the_price() {
    let b = setup();
    let vault = funded(&b);
    // Plus de 26 heures sans publication : la strategie ne se valorise plus,
    // total_assets du vault non plus.
    b.env.ledger().with_mut(|li| li.timestamp += 27 * 3_600);
    assert!(vault.try_total_assets().is_err());

    // L'admin publie lui-meme : tout repart, sortie d'urgence comprise, sans
    // passage en perte force.
    b.strategy.set_reference_price(&b.admin, &PRICE, &0);
    assert!(vault.total_assets() > 99_8000000);
    assert!(vault.emergency_exit(&b.strategy.address, &false) > 19_8000000);
}

#[test]
fn a_second_depositor_in_the_strategy_cannot_drain_the_vault_position() {
    let b = setup();
    // Un tiers depose avant le vault (apres l'entree du vault, le pool est
    // decale d'environ 1,8 % et une nouvelle entree serait refusee tant
    // qu'on ne l'a pas realigne : c'est le role du script d'alignement).
    let intruder = Address::generate(&b.env);
    StellarAssetClient::new(&b.env, &b.usdc.address).mint(&intruder, &1_0000000);
    b.strategy.deposit(&1_0000000, &intruder);
    let vault = funded(&b);

    // L'intrus ne retire que sa part : la position du vault ne lui est pas
    // accessible.
    assert!(b
        .strategy
        .try_withdraw(&5_0000000, &intruder, &intruder)
        .is_err());
    assert!(b.strategy.balance(&vault.address) > 19_8000000);
}

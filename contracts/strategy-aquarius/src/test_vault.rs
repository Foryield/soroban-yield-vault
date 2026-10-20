#![cfg(test)]
// Montants en convention Stellar 7 decimales (X_XXXXXXX, ex. 0_1000000 = 0,1).
#![allow(clippy::inconsistent_digit_grouping, clippy::zero_prefixed_literal)]
//! Le vault D5 sur la strategie Aquarius, pile Aquarius reelle. Seules les
//! signatures de l'appelant de tete (porteur, keeper, admin) sont fournies :
//! toute la chaine vault -> strategie -> router -> pool s'autorise elle-meme.

use super::test::{align_pool, setup, setup_quiet, swap, Bench, PRICE};
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

mod props {
    use super::*;
    use crate::AquaStrategyError;
    use proptest::prelude::*;
    use soroban_sdk::{Error, InvokeError};
    use yield_vault::VaultError;

    /// Un refus doit etre une garde nommee : la perte bornee du vault, son
    /// manque de retrait, ou le glissement de la strategie. Une panique
    /// d'hote, un prix perime ou un depassement ne passent pas pour un revert
    /// legitime.
    fn refused<T: core::fmt::Debug, C: core::fmt::Debug>(
        r: Result<Result<T, C>, Result<Error, InvokeError>>,
    ) -> Result<T, Error> {
        let guards: [Error; 3] = [
            VaultError::RebalanceLoss.into(),
            VaultError::StrategyShortfall.into(),
            Error::from_contract_error(AquaStrategyError::Slippage as u32),
        ];
        match r {
            Ok(Ok(value)) => Ok(value),
            Err(Ok(e)) if guards.contains(&e) => Err(e),
            other => panic!("echec hors des gardes : {other:?}"),
        }
    }

    /// Sortie de `pct` % des parts de `holder` : sa quote-part a 1 % pres,
    /// ou un revert qui ne touche ni ses parts ni l'actif du vault.
    fn exit(
        b: &Bench,
        vault: &YieldVaultClient,
        holder: &Address,
        pct: i128,
    ) -> Result<(), TestCaseError> {
        let held = vault.shares_of(holder);
        let shares = held * pct / 100;
        if shares == 0 {
            return Ok(());
        }
        let (assets, total) = (vault.total_assets(), vault.total_shares());
        let due = shares * assets / total;
        let wallet = b.usdc.balance(holder);
        match refused(vault.try_withdraw(holder, &shares)) {
            Ok(got) => {
                prop_assert!(got <= due, "{got} pour {due}");
                prop_assert!(got * 10_000 >= due * 9_900, "{got} pour {due}");
                prop_assert_eq!(b.usdc.balance(holder), wallet + got);
            }
            Err(_) => {
                prop_assert_eq!(vault.shares_of(holder), held);
                prop_assert_eq!(vault.total_assets(), assets);
            }
        }
        Ok(())
    }

    proptest! {
        // Pile Aquarius reelle par cas : peu de cas, des attaques quelconques.
        #![proptest_config(ProptestConfig::with_cases(24))]

        /// Un tiers deplace le pool juste avant le lot du keeper. Pendant que
        /// le pool est deplace, les porteurs sortent, le second par Aquarius
        /// (sandwich d'un retrait) ; puis le tiers se retire et le reste sort.
        /// Aucun swap ne fait baisser la valeur du vault (la valeur equitable
        /// ne depend que du produit des reserves, que les frais font croitre) ;
        /// le lot passe avec une perte sous 1 % du volume deplace ou revert
        /// sans rien changer ; chaque sortie rend sa quote-part a 1 % pres ou
        /// revert sans rien changer.
        #[test]
        fn a_pool_mover_never_costs_the_vault_beyond_the_margin(
            usdc_in in any::<bool>(),
            // Petits deplacements, pres du seuil de 1 %, et gros, jusqu'a
            // pres de 30 % des reserves du pool.
            size in prop_oneof![0i128..5_0000000, 0i128..300_0000000],
            exit_pct in 1i128..=100,
        ) {
            let b = setup_quiet();
            let vault = funded(&b);
            align_pool(&b, PRICE);
            // Un second depot porte Aquarius sous sa cible au-dela du seuil.
            let other = Address::generate(&b.env);
            StellarAssetClient::new(&b.env, &b.usdc.address).mint(&other, &50_0000000);
            vault.deposit(&other, &50_0000000);

            let (token_in, token_out) = if usdc_in {
                (&b.usdc.address, &b.eurc.address)
            } else {
                (&b.eurc.address, &b.usdc.address)
            };
            let start = vault.total_assets();
            let received = if size > 0 { swap(&b, token_in, token_out, size) } else { 0 };
            let moved_pool = vault.total_assets();
            prop_assert!(moved_pool >= start, "{moved_pool} < {start}");

            let allocation = vault.allocation();
            match refused(vault.try_rebalance(&b.keeper)) {
                Ok(moves) => {
                    let (mut out, mut inn) = (0i128, 0i128);
                    for (_, flow) in moves.iter() {
                        if flow < 0 { out -= flow } else { inn += flow }
                    }
                    let loss = moved_pool - vault.total_assets();
                    prop_assert!(loss * 10_000 <= 100 * out.max(inn), "perte {loss}");
                }
                Err(_) => {
                    prop_assert_eq!(vault.total_assets(), moved_pool);
                    prop_assert_eq!(vault.allocation(), allocation);
                }
            }

            // Sandwich d'un retrait, le pool encore deplace : la sortie
            // complete du premier porteur vide presque l'oisif, celle du
            // second puise alors dans Aquarius.
            exit(&b, &vault, &b.user, 100)?;
            exit(&b, &vault, &other, exit_pct)?;

            let before_unwind = vault.total_assets();
            if received > 0 {
                swap(&b, token_out, token_in, received);
            }
            prop_assert!(vault.total_assets() >= before_unwind);

            // Puis le reste sort, par Aquarius.
            exit(&b, &vault, &b.user, 100)?;
            exit(&b, &vault, &other, 100)?;
        }
    }
}

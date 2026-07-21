#![cfg(test)]
//! Proprietes (proptest) : invariants de la math de parts sur entrees
//! aleatoires. 256 cas generes par propriete a chaque execution.

use super::{YieldVault, YieldVaultClient};
use proptest::prelude::*;
use soroban_sdk::{
    testutils::{Address as _, EnvTestConfig},
    token::{StellarAssetClient, TokenClient},
    Address, Env,
};

fn bench(
    mint: i128,
) -> (
    Env,
    Address,
    TokenClient<'static>,
    YieldVaultClient<'static>,
) {
    // Pas de snapshot par cas : proptest rejouerait 256 ecritures par test.
    let env = Env::new_with_config(EnvTestConfig {
        capture_snapshot_at_drop: false,
    });
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let asset = env
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    StellarAssetClient::new(&env, &asset).mint(&user, &mint);
    let vault_id = env.register(YieldVault, ());
    let vault = YieldVaultClient::new(&env, &vault_id);
    vault.initialize(&admin, &asset, &None);
    (env.clone(), user, TokenClient::new(&env, &asset), vault)
}

fn donate(env: &Env, token: &TokenClient, vault: &YieldVaultClient, amount: i128) {
    if amount > 0 {
        StellarAssetClient::new(env, &token.address).mint(&vault.address, &amount);
    }
}

/// Prix de part non decroissant : compare A2/S2 >= A1/S1 en produits croises
/// (aucune division, aucun arrondi dans la verification elle-meme).
fn price_did_not_decrease(a1: i128, s1: i128, a2: i128, s2: i128) -> bool {
    a2.checked_mul(s1).unwrap() >= a1.checked_mul(s2).unwrap()
}

proptest! {
    /// Sans rendement, un cycle depot -> retrait total ne rend jamais plus
    /// que le depot (pas d'argent gratuit, quelle que soit la troncature).
    #[test]
    fn prop_no_free_lunch(dep in 1_001i128..1_000_000_000_000) {
        let (_env, user, token, vault) = bench(dep);
        let shares = vault.deposit(&user, &dep);
        let got = vault.withdraw(&user, &shares);
        prop_assert!(got <= dep);
        prop_assert_eq!(token.balance(&user), got);
    }

    /// Le prix de la part ne baisse jamais sous l'effet d'un depot, d'un
    /// rendement ou d'un retrait (les arrondis servent les parts en place).
    #[test]
    fn prop_share_price_monotone(
        dep1 in 1_001i128..1_000_000_000,
        yld in 0i128..1_000_000_000,
        dep2 in 1i128..1_000_000_000,
        wfrac in 1u32..=100,
    ) {
        let (env, user, token, vault) = bench(dep1 + dep2);
        vault.deposit(&user, &dep1);

        let (mut a, mut s) = (vault.total_assets(), vault.total_shares());

        donate(&env, &token, &vault, yld);
        let (a2, s2) = (vault.total_assets(), vault.total_shares());
        prop_assert!(price_did_not_decrease(a, s, a2, s2));
        (a, s) = (a2, s2);

        // Depot 2 : peut tronquer a 0 part (rejete), sinon prix preserve.
        if vault.try_deposit(&user, &dep2).is_ok() {
            let (a3, s3) = (vault.total_assets(), vault.total_shares());
            prop_assert!(price_did_not_decrease(a, s, a3, s3));
            (a, s) = (a3, s3);
        }

        // Retrait d'une fraction des parts du user, si non nul apres arrondi.
        let held = vault.shares_of(&user);
        let w = (held * i128::from(wfrac)) / 100;
        if w > 0 && vault.try_withdraw(&user, &w).is_ok() {
            let (a4, s4) = (vault.total_assets(), vault.total_shares());
            prop_assert!(price_did_not_decrease(a, s, a4, s4));
        }
    }

    /// Solvabilite permanente a deux deposants : la creance cumulee des parts
    /// vivantes ne depasse jamais l'actif detenu, et les parts mortes restent.
    #[test]
    fn prop_two_users_solvency(
        dep1 in 1_001i128..1_000_000_000,
        dep2 in 1i128..1_000_000_000,
        yld in 0i128..1_000_000_000,
        w1frac in 0u32..=100,
    ) {
        let (env, user1, token, vault) = bench(dep1);
        let user2 = Address::generate(&env);
        StellarAssetClient::new(&env, &token.address).mint(&user2, &dep2);

        vault.deposit(&user1, &dep1);
        donate(&env, &token, &vault, yld);
        let _ = vault.try_deposit(&user2, &dep2);

        let w = (vault.shares_of(&user1) * i128::from(w1frac)) / 100;
        if w > 0 {
            let _ = vault.try_withdraw(&user1, &w);
        }

        let assets = vault.total_assets();
        let total = vault.total_shares();
        let live = vault.shares_of(&user1) + vault.shares_of(&user2);
        prop_assert!(total > live, "les parts mortes ont disparu");
        prop_assert!(live.checked_mul(assets).unwrap() / total <= assets);
    }
}

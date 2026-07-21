#![cfg(test)]
//! FICHIER GENERE - ne pas editer a la main.
//! Matrice deposit x rendement x fraction de retrait : chaque valeur
//! attendue est un litteral calcule par un oracle Python independant
//! (scripts/generate_matrix_tests.py), pas par la formule du contrat.

use super::{YieldVault, YieldVaultClient};
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
    // Pas de snapshot par test : 200 bancs generes pollueraient le repo.
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

#[test]
fn m_d1001_y0_all() {
    let (_env, user, _token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1001), 1);
    // pas de rendement simule
    assert_eq!(vault.total_assets(), 1001);
    assert_eq!(vault.withdraw(&user, &1), 1);
    assert_eq!(vault.shares_of(&user), 0);
    assert_eq!(vault.total_shares(), 1000);
    assert_eq!(vault.total_assets(), 1000);
    // Solvabilite relue depuis le contrat (l'oracle attend 0).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 0);
}

#[test]
fn m_d1001_y0_half() {
    let (_env, user, _token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1001), 1);
    // pas de rendement simule
    assert_eq!(vault.total_assets(), 1001);
    assert_eq!(vault.withdraw(&user, &1), 1);
    assert_eq!(vault.shares_of(&user), 0);
    assert_eq!(vault.total_shares(), 1000);
    assert_eq!(vault.total_assets(), 1000);
    // Solvabilite relue depuis le contrat (l'oracle attend 0).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 0);
}

#[test]
fn m_d1001_y0_third() {
    let (_env, user, _token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1001), 1);
    // pas de rendement simule
    assert_eq!(vault.total_assets(), 1001);
    assert_eq!(vault.withdraw(&user, &1), 1);
    assert_eq!(vault.shares_of(&user), 0);
    assert_eq!(vault.total_shares(), 1000);
    assert_eq!(vault.total_assets(), 1000);
    // Solvabilite relue depuis le contrat (l'oracle attend 0).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 0);
}

#[test]
fn m_d1001_y0_twothirds() {
    let (_env, user, _token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1001), 1);
    // pas de rendement simule
    assert_eq!(vault.total_assets(), 1001);
    assert_eq!(vault.withdraw(&user, &1), 1);
    assert_eq!(vault.shares_of(&user), 0);
    assert_eq!(vault.total_shares(), 1000);
    assert_eq!(vault.total_assets(), 1000);
    // Solvabilite relue depuis le contrat (l'oracle attend 0).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 0);
}

#[test]
fn m_d1001_y0_one() {
    let (_env, user, _token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1001), 1);
    // pas de rendement simule
    assert_eq!(vault.total_assets(), 1001);
    assert_eq!(vault.withdraw(&user, &1), 1);
    assert_eq!(vault.shares_of(&user), 0);
    assert_eq!(vault.total_shares(), 1000);
    assert_eq!(vault.total_assets(), 1000);
    // Solvabilite relue depuis le contrat (l'oracle attend 0).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 0);
}

#[test]
fn m_d1001_y1_all() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1001), 1);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &1);
    assert_eq!(vault.total_assets(), 1002);
    assert_eq!(vault.withdraw(&user, &1), 1);
    assert_eq!(vault.shares_of(&user), 0);
    assert_eq!(vault.total_shares(), 1000);
    assert_eq!(vault.total_assets(), 1001);
    // Solvabilite relue depuis le contrat (l'oracle attend 0).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 0);
}

#[test]
fn m_d1001_y1_half() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1001), 1);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &1);
    assert_eq!(vault.total_assets(), 1002);
    assert_eq!(vault.withdraw(&user, &1), 1);
    assert_eq!(vault.shares_of(&user), 0);
    assert_eq!(vault.total_shares(), 1000);
    assert_eq!(vault.total_assets(), 1001);
    // Solvabilite relue depuis le contrat (l'oracle attend 0).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 0);
}

#[test]
fn m_d1001_y1_third() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1001), 1);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &1);
    assert_eq!(vault.total_assets(), 1002);
    assert_eq!(vault.withdraw(&user, &1), 1);
    assert_eq!(vault.shares_of(&user), 0);
    assert_eq!(vault.total_shares(), 1000);
    assert_eq!(vault.total_assets(), 1001);
    // Solvabilite relue depuis le contrat (l'oracle attend 0).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 0);
}

#[test]
fn m_d1001_y1_twothirds() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1001), 1);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &1);
    assert_eq!(vault.total_assets(), 1002);
    assert_eq!(vault.withdraw(&user, &1), 1);
    assert_eq!(vault.shares_of(&user), 0);
    assert_eq!(vault.total_shares(), 1000);
    assert_eq!(vault.total_assets(), 1001);
    // Solvabilite relue depuis le contrat (l'oracle attend 0).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 0);
}

#[test]
fn m_d1001_y1_one() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1001), 1);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &1);
    assert_eq!(vault.total_assets(), 1002);
    assert_eq!(vault.withdraw(&user, &1), 1);
    assert_eq!(vault.shares_of(&user), 0);
    assert_eq!(vault.total_shares(), 1000);
    assert_eq!(vault.total_assets(), 1001);
    // Solvabilite relue depuis le contrat (l'oracle attend 0).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 0);
}

#[test]
fn m_d1001_y999_all() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1001), 1);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &999);
    assert_eq!(vault.total_assets(), 2000);
    assert_eq!(vault.withdraw(&user, &1), 1);
    assert_eq!(vault.shares_of(&user), 0);
    assert_eq!(vault.total_shares(), 1000);
    assert_eq!(vault.total_assets(), 1999);
    // Solvabilite relue depuis le contrat (l'oracle attend 0).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 0);
}

#[test]
fn m_d1001_y999_half() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1001), 1);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &999);
    assert_eq!(vault.total_assets(), 2000);
    assert_eq!(vault.withdraw(&user, &1), 1);
    assert_eq!(vault.shares_of(&user), 0);
    assert_eq!(vault.total_shares(), 1000);
    assert_eq!(vault.total_assets(), 1999);
    // Solvabilite relue depuis le contrat (l'oracle attend 0).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 0);
}

#[test]
fn m_d1001_y999_third() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1001), 1);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &999);
    assert_eq!(vault.total_assets(), 2000);
    assert_eq!(vault.withdraw(&user, &1), 1);
    assert_eq!(vault.shares_of(&user), 0);
    assert_eq!(vault.total_shares(), 1000);
    assert_eq!(vault.total_assets(), 1999);
    // Solvabilite relue depuis le contrat (l'oracle attend 0).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 0);
}

#[test]
fn m_d1001_y999_twothirds() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1001), 1);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &999);
    assert_eq!(vault.total_assets(), 2000);
    assert_eq!(vault.withdraw(&user, &1), 1);
    assert_eq!(vault.shares_of(&user), 0);
    assert_eq!(vault.total_shares(), 1000);
    assert_eq!(vault.total_assets(), 1999);
    // Solvabilite relue depuis le contrat (l'oracle attend 0).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 0);
}

#[test]
fn m_d1001_y999_one() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1001), 1);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &999);
    assert_eq!(vault.total_assets(), 2000);
    assert_eq!(vault.withdraw(&user, &1), 1);
    assert_eq!(vault.shares_of(&user), 0);
    assert_eq!(vault.total_shares(), 1000);
    assert_eq!(vault.total_assets(), 1999);
    // Solvabilite relue depuis le contrat (l'oracle attend 0).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 0);
}

#[test]
fn m_d1001_y10000_all() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1001), 1);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &10000);
    assert_eq!(vault.total_assets(), 11001);
    assert_eq!(vault.withdraw(&user, &1), 10);
    assert_eq!(vault.shares_of(&user), 0);
    assert_eq!(vault.total_shares(), 1000);
    assert_eq!(vault.total_assets(), 10991);
    // Solvabilite relue depuis le contrat (l'oracle attend 0).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 0);
}

#[test]
fn m_d1001_y10000_half() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1001), 1);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &10000);
    assert_eq!(vault.total_assets(), 11001);
    assert_eq!(vault.withdraw(&user, &1), 10);
    assert_eq!(vault.shares_of(&user), 0);
    assert_eq!(vault.total_shares(), 1000);
    assert_eq!(vault.total_assets(), 10991);
    // Solvabilite relue depuis le contrat (l'oracle attend 0).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 0);
}

#[test]
fn m_d1001_y10000_third() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1001), 1);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &10000);
    assert_eq!(vault.total_assets(), 11001);
    assert_eq!(vault.withdraw(&user, &1), 10);
    assert_eq!(vault.shares_of(&user), 0);
    assert_eq!(vault.total_shares(), 1000);
    assert_eq!(vault.total_assets(), 10991);
    // Solvabilite relue depuis le contrat (l'oracle attend 0).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 0);
}

#[test]
fn m_d1001_y10000_twothirds() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1001), 1);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &10000);
    assert_eq!(vault.total_assets(), 11001);
    assert_eq!(vault.withdraw(&user, &1), 10);
    assert_eq!(vault.shares_of(&user), 0);
    assert_eq!(vault.total_shares(), 1000);
    assert_eq!(vault.total_assets(), 10991);
    // Solvabilite relue depuis le contrat (l'oracle attend 0).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 0);
}

#[test]
fn m_d1001_y10000_one() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1001), 1);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &10000);
    assert_eq!(vault.total_assets(), 11001);
    assert_eq!(vault.withdraw(&user, &1), 10);
    assert_eq!(vault.shares_of(&user), 0);
    assert_eq!(vault.total_shares(), 1000);
    assert_eq!(vault.total_assets(), 10991);
    // Solvabilite relue depuis le contrat (l'oracle attend 0).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 0);
}

#[test]
fn m_d1001_y123457_all() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1001), 1);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &123457);
    assert_eq!(vault.total_assets(), 124458);
    assert_eq!(vault.withdraw(&user, &1), 124);
    assert_eq!(vault.shares_of(&user), 0);
    assert_eq!(vault.total_shares(), 1000);
    assert_eq!(vault.total_assets(), 124334);
    // Solvabilite relue depuis le contrat (l'oracle attend 0).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 0);
}

#[test]
fn m_d1001_y123457_half() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1001), 1);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &123457);
    assert_eq!(vault.total_assets(), 124458);
    assert_eq!(vault.withdraw(&user, &1), 124);
    assert_eq!(vault.shares_of(&user), 0);
    assert_eq!(vault.total_shares(), 1000);
    assert_eq!(vault.total_assets(), 124334);
    // Solvabilite relue depuis le contrat (l'oracle attend 0).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 0);
}

#[test]
fn m_d1001_y123457_third() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1001), 1);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &123457);
    assert_eq!(vault.total_assets(), 124458);
    assert_eq!(vault.withdraw(&user, &1), 124);
    assert_eq!(vault.shares_of(&user), 0);
    assert_eq!(vault.total_shares(), 1000);
    assert_eq!(vault.total_assets(), 124334);
    // Solvabilite relue depuis le contrat (l'oracle attend 0).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 0);
}

#[test]
fn m_d1001_y123457_twothirds() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1001), 1);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &123457);
    assert_eq!(vault.total_assets(), 124458);
    assert_eq!(vault.withdraw(&user, &1), 124);
    assert_eq!(vault.shares_of(&user), 0);
    assert_eq!(vault.total_shares(), 1000);
    assert_eq!(vault.total_assets(), 124334);
    // Solvabilite relue depuis le contrat (l'oracle attend 0).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 0);
}

#[test]
fn m_d1001_y123457_one() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1001), 1);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &123457);
    assert_eq!(vault.total_assets(), 124458);
    assert_eq!(vault.withdraw(&user, &1), 124);
    assert_eq!(vault.shares_of(&user), 0);
    assert_eq!(vault.total_shares(), 1000);
    assert_eq!(vault.total_assets(), 124334);
    // Solvabilite relue depuis le contrat (l'oracle attend 0).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 0);
}

#[test]
fn m_d1500_y0_all() {
    let (_env, user, _token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1500), 500);
    // pas de rendement simule
    assert_eq!(vault.total_assets(), 1500);
    assert_eq!(vault.withdraw(&user, &500), 500);
    assert_eq!(vault.shares_of(&user), 0);
    assert_eq!(vault.total_shares(), 1000);
    assert_eq!(vault.total_assets(), 1000);
    // Solvabilite relue depuis le contrat (l'oracle attend 0).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 0);
}

#[test]
fn m_d1500_y0_half() {
    let (_env, user, _token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1500), 500);
    // pas de rendement simule
    assert_eq!(vault.total_assets(), 1500);
    assert_eq!(vault.withdraw(&user, &250), 250);
    assert_eq!(vault.shares_of(&user), 250);
    assert_eq!(vault.total_shares(), 1250);
    assert_eq!(vault.total_assets(), 1250);
    // Solvabilite relue depuis le contrat (l'oracle attend 250).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 250);
}

#[test]
fn m_d1500_y0_third() {
    let (_env, user, _token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1500), 500);
    // pas de rendement simule
    assert_eq!(vault.total_assets(), 1500);
    assert_eq!(vault.withdraw(&user, &166), 166);
    assert_eq!(vault.shares_of(&user), 334);
    assert_eq!(vault.total_shares(), 1334);
    assert_eq!(vault.total_assets(), 1334);
    // Solvabilite relue depuis le contrat (l'oracle attend 334).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 334);
}

#[test]
fn m_d1500_y0_twothirds() {
    let (_env, user, _token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1500), 500);
    // pas de rendement simule
    assert_eq!(vault.total_assets(), 1500);
    assert_eq!(vault.withdraw(&user, &333), 333);
    assert_eq!(vault.shares_of(&user), 167);
    assert_eq!(vault.total_shares(), 1167);
    assert_eq!(vault.total_assets(), 1167);
    // Solvabilite relue depuis le contrat (l'oracle attend 167).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 167);
}

#[test]
fn m_d1500_y0_one() {
    let (_env, user, _token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1500), 500);
    // pas de rendement simule
    assert_eq!(vault.total_assets(), 1500);
    assert_eq!(vault.withdraw(&user, &1), 1);
    assert_eq!(vault.shares_of(&user), 499);
    assert_eq!(vault.total_shares(), 1499);
    assert_eq!(vault.total_assets(), 1499);
    // Solvabilite relue depuis le contrat (l'oracle attend 499).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 499);
}

#[test]
fn m_d1500_y1_all() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1500), 500);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &1);
    assert_eq!(vault.total_assets(), 1501);
    assert_eq!(vault.withdraw(&user, &500), 500);
    assert_eq!(vault.shares_of(&user), 0);
    assert_eq!(vault.total_shares(), 1000);
    assert_eq!(vault.total_assets(), 1001);
    // Solvabilite relue depuis le contrat (l'oracle attend 0).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 0);
}

#[test]
fn m_d1500_y1_half() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1500), 500);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &1);
    assert_eq!(vault.total_assets(), 1501);
    assert_eq!(vault.withdraw(&user, &250), 250);
    assert_eq!(vault.shares_of(&user), 250);
    assert_eq!(vault.total_shares(), 1250);
    assert_eq!(vault.total_assets(), 1251);
    // Solvabilite relue depuis le contrat (l'oracle attend 250).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 250);
}

#[test]
fn m_d1500_y1_third() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1500), 500);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &1);
    assert_eq!(vault.total_assets(), 1501);
    assert_eq!(vault.withdraw(&user, &166), 166);
    assert_eq!(vault.shares_of(&user), 334);
    assert_eq!(vault.total_shares(), 1334);
    assert_eq!(vault.total_assets(), 1335);
    // Solvabilite relue depuis le contrat (l'oracle attend 334).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 334);
}

#[test]
fn m_d1500_y1_twothirds() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1500), 500);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &1);
    assert_eq!(vault.total_assets(), 1501);
    assert_eq!(vault.withdraw(&user, &333), 333);
    assert_eq!(vault.shares_of(&user), 167);
    assert_eq!(vault.total_shares(), 1167);
    assert_eq!(vault.total_assets(), 1168);
    // Solvabilite relue depuis le contrat (l'oracle attend 167).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 167);
}

#[test]
fn m_d1500_y1_one() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1500), 500);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &1);
    assert_eq!(vault.total_assets(), 1501);
    assert_eq!(vault.withdraw(&user, &1), 1);
    assert_eq!(vault.shares_of(&user), 499);
    assert_eq!(vault.total_shares(), 1499);
    assert_eq!(vault.total_assets(), 1500);
    // Solvabilite relue depuis le contrat (l'oracle attend 499).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 499);
}

#[test]
fn m_d1500_y999_all() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1500), 500);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &999);
    assert_eq!(vault.total_assets(), 2499);
    assert_eq!(vault.withdraw(&user, &500), 833);
    assert_eq!(vault.shares_of(&user), 0);
    assert_eq!(vault.total_shares(), 1000);
    assert_eq!(vault.total_assets(), 1666);
    // Solvabilite relue depuis le contrat (l'oracle attend 0).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 0);
}

#[test]
fn m_d1500_y999_half() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1500), 500);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &999);
    assert_eq!(vault.total_assets(), 2499);
    assert_eq!(vault.withdraw(&user, &250), 416);
    assert_eq!(vault.shares_of(&user), 250);
    assert_eq!(vault.total_shares(), 1250);
    assert_eq!(vault.total_assets(), 2083);
    // Solvabilite relue depuis le contrat (l'oracle attend 416).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 416);
}

#[test]
fn m_d1500_y999_third() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1500), 500);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &999);
    assert_eq!(vault.total_assets(), 2499);
    assert_eq!(vault.withdraw(&user, &166), 276);
    assert_eq!(vault.shares_of(&user), 334);
    assert_eq!(vault.total_shares(), 1334);
    assert_eq!(vault.total_assets(), 2223);
    // Solvabilite relue depuis le contrat (l'oracle attend 556).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 556);
}

#[test]
fn m_d1500_y999_twothirds() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1500), 500);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &999);
    assert_eq!(vault.total_assets(), 2499);
    assert_eq!(vault.withdraw(&user, &333), 554);
    assert_eq!(vault.shares_of(&user), 167);
    assert_eq!(vault.total_shares(), 1167);
    assert_eq!(vault.total_assets(), 1945);
    // Solvabilite relue depuis le contrat (l'oracle attend 278).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 278);
}

#[test]
fn m_d1500_y999_one() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1500), 500);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &999);
    assert_eq!(vault.total_assets(), 2499);
    assert_eq!(vault.withdraw(&user, &1), 1);
    assert_eq!(vault.shares_of(&user), 499);
    assert_eq!(vault.total_shares(), 1499);
    assert_eq!(vault.total_assets(), 2498);
    // Solvabilite relue depuis le contrat (l'oracle attend 831).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 831);
}

#[test]
fn m_d1500_y10000_all() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1500), 500);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &10000);
    assert_eq!(vault.total_assets(), 11500);
    assert_eq!(vault.withdraw(&user, &500), 3833);
    assert_eq!(vault.shares_of(&user), 0);
    assert_eq!(vault.total_shares(), 1000);
    assert_eq!(vault.total_assets(), 7667);
    // Solvabilite relue depuis le contrat (l'oracle attend 0).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 0);
}

#[test]
fn m_d1500_y10000_half() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1500), 500);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &10000);
    assert_eq!(vault.total_assets(), 11500);
    assert_eq!(vault.withdraw(&user, &250), 1916);
    assert_eq!(vault.shares_of(&user), 250);
    assert_eq!(vault.total_shares(), 1250);
    assert_eq!(vault.total_assets(), 9584);
    // Solvabilite relue depuis le contrat (l'oracle attend 1916).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 1916);
}

#[test]
fn m_d1500_y10000_third() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1500), 500);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &10000);
    assert_eq!(vault.total_assets(), 11500);
    assert_eq!(vault.withdraw(&user, &166), 1272);
    assert_eq!(vault.shares_of(&user), 334);
    assert_eq!(vault.total_shares(), 1334);
    assert_eq!(vault.total_assets(), 10228);
    // Solvabilite relue depuis le contrat (l'oracle attend 2560).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 2560);
}

#[test]
fn m_d1500_y10000_twothirds() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1500), 500);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &10000);
    assert_eq!(vault.total_assets(), 11500);
    assert_eq!(vault.withdraw(&user, &333), 2553);
    assert_eq!(vault.shares_of(&user), 167);
    assert_eq!(vault.total_shares(), 1167);
    assert_eq!(vault.total_assets(), 8947);
    // Solvabilite relue depuis le contrat (l'oracle attend 1280).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 1280);
}

#[test]
fn m_d1500_y10000_one() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1500), 500);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &10000);
    assert_eq!(vault.total_assets(), 11500);
    assert_eq!(vault.withdraw(&user, &1), 7);
    assert_eq!(vault.shares_of(&user), 499);
    assert_eq!(vault.total_shares(), 1499);
    assert_eq!(vault.total_assets(), 11493);
    // Solvabilite relue depuis le contrat (l'oracle attend 3825).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 3825);
}

#[test]
fn m_d1500_y123457_all() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1500), 500);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &123457);
    assert_eq!(vault.total_assets(), 124957);
    assert_eq!(vault.withdraw(&user, &500), 41652);
    assert_eq!(vault.shares_of(&user), 0);
    assert_eq!(vault.total_shares(), 1000);
    assert_eq!(vault.total_assets(), 83305);
    // Solvabilite relue depuis le contrat (l'oracle attend 0).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 0);
}

#[test]
fn m_d1500_y123457_half() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1500), 500);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &123457);
    assert_eq!(vault.total_assets(), 124957);
    assert_eq!(vault.withdraw(&user, &250), 20826);
    assert_eq!(vault.shares_of(&user), 250);
    assert_eq!(vault.total_shares(), 1250);
    assert_eq!(vault.total_assets(), 104131);
    // Solvabilite relue depuis le contrat (l'oracle attend 20826).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 20826);
}

#[test]
fn m_d1500_y123457_third() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1500), 500);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &123457);
    assert_eq!(vault.total_assets(), 124957);
    assert_eq!(vault.withdraw(&user, &166), 13828);
    assert_eq!(vault.shares_of(&user), 334);
    assert_eq!(vault.total_shares(), 1334);
    assert_eq!(vault.total_assets(), 111129);
    // Solvabilite relue depuis le contrat (l'oracle attend 27823).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 27823);
}

#[test]
fn m_d1500_y123457_twothirds() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1500), 500);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &123457);
    assert_eq!(vault.total_assets(), 124957);
    assert_eq!(vault.withdraw(&user, &333), 27740);
    assert_eq!(vault.shares_of(&user), 167);
    assert_eq!(vault.total_shares(), 1167);
    assert_eq!(vault.total_assets(), 97217);
    // Solvabilite relue depuis le contrat (l'oracle attend 13911).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 13911);
}

#[test]
fn m_d1500_y123457_one() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1500), 500);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &123457);
    assert_eq!(vault.total_assets(), 124957);
    assert_eq!(vault.withdraw(&user, &1), 83);
    assert_eq!(vault.shares_of(&user), 499);
    assert_eq!(vault.total_shares(), 1499);
    assert_eq!(vault.total_assets(), 124874);
    // Solvabilite relue depuis le contrat (l'oracle attend 41569).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 41569);
}

#[test]
fn m_d2000_y0_all() {
    let (_env, user, _token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &2000), 1000);
    // pas de rendement simule
    assert_eq!(vault.total_assets(), 2000);
    assert_eq!(vault.withdraw(&user, &1000), 1000);
    assert_eq!(vault.shares_of(&user), 0);
    assert_eq!(vault.total_shares(), 1000);
    assert_eq!(vault.total_assets(), 1000);
    // Solvabilite relue depuis le contrat (l'oracle attend 0).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 0);
}

#[test]
fn m_d2000_y0_half() {
    let (_env, user, _token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &2000), 1000);
    // pas de rendement simule
    assert_eq!(vault.total_assets(), 2000);
    assert_eq!(vault.withdraw(&user, &500), 500);
    assert_eq!(vault.shares_of(&user), 500);
    assert_eq!(vault.total_shares(), 1500);
    assert_eq!(vault.total_assets(), 1500);
    // Solvabilite relue depuis le contrat (l'oracle attend 500).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 500);
}

#[test]
fn m_d2000_y0_third() {
    let (_env, user, _token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &2000), 1000);
    // pas de rendement simule
    assert_eq!(vault.total_assets(), 2000);
    assert_eq!(vault.withdraw(&user, &333), 333);
    assert_eq!(vault.shares_of(&user), 667);
    assert_eq!(vault.total_shares(), 1667);
    assert_eq!(vault.total_assets(), 1667);
    // Solvabilite relue depuis le contrat (l'oracle attend 667).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 667);
}

#[test]
fn m_d2000_y0_twothirds() {
    let (_env, user, _token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &2000), 1000);
    // pas de rendement simule
    assert_eq!(vault.total_assets(), 2000);
    assert_eq!(vault.withdraw(&user, &666), 666);
    assert_eq!(vault.shares_of(&user), 334);
    assert_eq!(vault.total_shares(), 1334);
    assert_eq!(vault.total_assets(), 1334);
    // Solvabilite relue depuis le contrat (l'oracle attend 334).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 334);
}

#[test]
fn m_d2000_y0_one() {
    let (_env, user, _token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &2000), 1000);
    // pas de rendement simule
    assert_eq!(vault.total_assets(), 2000);
    assert_eq!(vault.withdraw(&user, &1), 1);
    assert_eq!(vault.shares_of(&user), 999);
    assert_eq!(vault.total_shares(), 1999);
    assert_eq!(vault.total_assets(), 1999);
    // Solvabilite relue depuis le contrat (l'oracle attend 999).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 999);
}

#[test]
fn m_d2000_y1_all() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &2000), 1000);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &1);
    assert_eq!(vault.total_assets(), 2001);
    assert_eq!(vault.withdraw(&user, &1000), 1000);
    assert_eq!(vault.shares_of(&user), 0);
    assert_eq!(vault.total_shares(), 1000);
    assert_eq!(vault.total_assets(), 1001);
    // Solvabilite relue depuis le contrat (l'oracle attend 0).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 0);
}

#[test]
fn m_d2000_y1_half() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &2000), 1000);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &1);
    assert_eq!(vault.total_assets(), 2001);
    assert_eq!(vault.withdraw(&user, &500), 500);
    assert_eq!(vault.shares_of(&user), 500);
    assert_eq!(vault.total_shares(), 1500);
    assert_eq!(vault.total_assets(), 1501);
    // Solvabilite relue depuis le contrat (l'oracle attend 500).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 500);
}

#[test]
fn m_d2000_y1_third() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &2000), 1000);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &1);
    assert_eq!(vault.total_assets(), 2001);
    assert_eq!(vault.withdraw(&user, &333), 333);
    assert_eq!(vault.shares_of(&user), 667);
    assert_eq!(vault.total_shares(), 1667);
    assert_eq!(vault.total_assets(), 1668);
    // Solvabilite relue depuis le contrat (l'oracle attend 667).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 667);
}

#[test]
fn m_d2000_y1_twothirds() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &2000), 1000);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &1);
    assert_eq!(vault.total_assets(), 2001);
    assert_eq!(vault.withdraw(&user, &666), 666);
    assert_eq!(vault.shares_of(&user), 334);
    assert_eq!(vault.total_shares(), 1334);
    assert_eq!(vault.total_assets(), 1335);
    // Solvabilite relue depuis le contrat (l'oracle attend 334).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 334);
}

#[test]
fn m_d2000_y1_one() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &2000), 1000);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &1);
    assert_eq!(vault.total_assets(), 2001);
    assert_eq!(vault.withdraw(&user, &1), 1);
    assert_eq!(vault.shares_of(&user), 999);
    assert_eq!(vault.total_shares(), 1999);
    assert_eq!(vault.total_assets(), 2000);
    // Solvabilite relue depuis le contrat (l'oracle attend 999).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 999);
}

#[test]
fn m_d2000_y999_all() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &2000), 1000);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &999);
    assert_eq!(vault.total_assets(), 2999);
    assert_eq!(vault.withdraw(&user, &1000), 1499);
    assert_eq!(vault.shares_of(&user), 0);
    assert_eq!(vault.total_shares(), 1000);
    assert_eq!(vault.total_assets(), 1500);
    // Solvabilite relue depuis le contrat (l'oracle attend 0).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 0);
}

#[test]
fn m_d2000_y999_half() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &2000), 1000);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &999);
    assert_eq!(vault.total_assets(), 2999);
    assert_eq!(vault.withdraw(&user, &500), 749);
    assert_eq!(vault.shares_of(&user), 500);
    assert_eq!(vault.total_shares(), 1500);
    assert_eq!(vault.total_assets(), 2250);
    // Solvabilite relue depuis le contrat (l'oracle attend 750).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 750);
}

#[test]
fn m_d2000_y999_third() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &2000), 1000);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &999);
    assert_eq!(vault.total_assets(), 2999);
    assert_eq!(vault.withdraw(&user, &333), 499);
    assert_eq!(vault.shares_of(&user), 667);
    assert_eq!(vault.total_shares(), 1667);
    assert_eq!(vault.total_assets(), 2500);
    // Solvabilite relue depuis le contrat (l'oracle attend 1000).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 1000);
}

#[test]
fn m_d2000_y999_twothirds() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &2000), 1000);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &999);
    assert_eq!(vault.total_assets(), 2999);
    assert_eq!(vault.withdraw(&user, &666), 998);
    assert_eq!(vault.shares_of(&user), 334);
    assert_eq!(vault.total_shares(), 1334);
    assert_eq!(vault.total_assets(), 2001);
    // Solvabilite relue depuis le contrat (l'oracle attend 501).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 501);
}

#[test]
fn m_d2000_y999_one() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &2000), 1000);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &999);
    assert_eq!(vault.total_assets(), 2999);
    assert_eq!(vault.withdraw(&user, &1), 1);
    assert_eq!(vault.shares_of(&user), 999);
    assert_eq!(vault.total_shares(), 1999);
    assert_eq!(vault.total_assets(), 2998);
    // Solvabilite relue depuis le contrat (l'oracle attend 1498).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 1498);
}

#[test]
fn m_d2000_y10000_all() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &2000), 1000);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &10000);
    assert_eq!(vault.total_assets(), 12000);
    assert_eq!(vault.withdraw(&user, &1000), 6000);
    assert_eq!(vault.shares_of(&user), 0);
    assert_eq!(vault.total_shares(), 1000);
    assert_eq!(vault.total_assets(), 6000);
    // Solvabilite relue depuis le contrat (l'oracle attend 0).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 0);
}

#[test]
fn m_d2000_y10000_half() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &2000), 1000);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &10000);
    assert_eq!(vault.total_assets(), 12000);
    assert_eq!(vault.withdraw(&user, &500), 3000);
    assert_eq!(vault.shares_of(&user), 500);
    assert_eq!(vault.total_shares(), 1500);
    assert_eq!(vault.total_assets(), 9000);
    // Solvabilite relue depuis le contrat (l'oracle attend 3000).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 3000);
}

#[test]
fn m_d2000_y10000_third() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &2000), 1000);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &10000);
    assert_eq!(vault.total_assets(), 12000);
    assert_eq!(vault.withdraw(&user, &333), 1998);
    assert_eq!(vault.shares_of(&user), 667);
    assert_eq!(vault.total_shares(), 1667);
    assert_eq!(vault.total_assets(), 10002);
    // Solvabilite relue depuis le contrat (l'oracle attend 4002).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 4002);
}

#[test]
fn m_d2000_y10000_twothirds() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &2000), 1000);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &10000);
    assert_eq!(vault.total_assets(), 12000);
    assert_eq!(vault.withdraw(&user, &666), 3996);
    assert_eq!(vault.shares_of(&user), 334);
    assert_eq!(vault.total_shares(), 1334);
    assert_eq!(vault.total_assets(), 8004);
    // Solvabilite relue depuis le contrat (l'oracle attend 2004).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 2004);
}

#[test]
fn m_d2000_y10000_one() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &2000), 1000);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &10000);
    assert_eq!(vault.total_assets(), 12000);
    assert_eq!(vault.withdraw(&user, &1), 6);
    assert_eq!(vault.shares_of(&user), 999);
    assert_eq!(vault.total_shares(), 1999);
    assert_eq!(vault.total_assets(), 11994);
    // Solvabilite relue depuis le contrat (l'oracle attend 5994).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 5994);
}

#[test]
fn m_d2000_y123457_all() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &2000), 1000);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &123457);
    assert_eq!(vault.total_assets(), 125457);
    assert_eq!(vault.withdraw(&user, &1000), 62728);
    assert_eq!(vault.shares_of(&user), 0);
    assert_eq!(vault.total_shares(), 1000);
    assert_eq!(vault.total_assets(), 62729);
    // Solvabilite relue depuis le contrat (l'oracle attend 0).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 0);
}

#[test]
fn m_d2000_y123457_half() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &2000), 1000);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &123457);
    assert_eq!(vault.total_assets(), 125457);
    assert_eq!(vault.withdraw(&user, &500), 31364);
    assert_eq!(vault.shares_of(&user), 500);
    assert_eq!(vault.total_shares(), 1500);
    assert_eq!(vault.total_assets(), 94093);
    // Solvabilite relue depuis le contrat (l'oracle attend 31364).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 31364);
}

#[test]
fn m_d2000_y123457_third() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &2000), 1000);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &123457);
    assert_eq!(vault.total_assets(), 125457);
    assert_eq!(vault.withdraw(&user, &333), 20888);
    assert_eq!(vault.shares_of(&user), 667);
    assert_eq!(vault.total_shares(), 1667);
    assert_eq!(vault.total_assets(), 104569);
    // Solvabilite relue depuis le contrat (l'oracle attend 41840).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 41840);
}

#[test]
fn m_d2000_y123457_twothirds() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &2000), 1000);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &123457);
    assert_eq!(vault.total_assets(), 125457);
    assert_eq!(vault.withdraw(&user, &666), 41777);
    assert_eq!(vault.shares_of(&user), 334);
    assert_eq!(vault.total_shares(), 1334);
    assert_eq!(vault.total_assets(), 83680);
    // Solvabilite relue depuis le contrat (l'oracle attend 20951).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 20951);
}

#[test]
fn m_d2000_y123457_one() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &2000), 1000);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &123457);
    assert_eq!(vault.total_assets(), 125457);
    assert_eq!(vault.withdraw(&user, &1), 62);
    assert_eq!(vault.shares_of(&user), 999);
    assert_eq!(vault.total_shares(), 1999);
    assert_eq!(vault.total_assets(), 125395);
    // Solvabilite relue depuis le contrat (l'oracle attend 62666).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 62666);
}

#[test]
fn m_d10000_y0_all() {
    let (_env, user, _token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &10000), 9000);
    // pas de rendement simule
    assert_eq!(vault.total_assets(), 10000);
    assert_eq!(vault.withdraw(&user, &9000), 9000);
    assert_eq!(vault.shares_of(&user), 0);
    assert_eq!(vault.total_shares(), 1000);
    assert_eq!(vault.total_assets(), 1000);
    // Solvabilite relue depuis le contrat (l'oracle attend 0).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 0);
}

#[test]
fn m_d10000_y0_half() {
    let (_env, user, _token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &10000), 9000);
    // pas de rendement simule
    assert_eq!(vault.total_assets(), 10000);
    assert_eq!(vault.withdraw(&user, &4500), 4500);
    assert_eq!(vault.shares_of(&user), 4500);
    assert_eq!(vault.total_shares(), 5500);
    assert_eq!(vault.total_assets(), 5500);
    // Solvabilite relue depuis le contrat (l'oracle attend 4500).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 4500);
}

#[test]
fn m_d10000_y0_third() {
    let (_env, user, _token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &10000), 9000);
    // pas de rendement simule
    assert_eq!(vault.total_assets(), 10000);
    assert_eq!(vault.withdraw(&user, &3000), 3000);
    assert_eq!(vault.shares_of(&user), 6000);
    assert_eq!(vault.total_shares(), 7000);
    assert_eq!(vault.total_assets(), 7000);
    // Solvabilite relue depuis le contrat (l'oracle attend 6000).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 6000);
}

#[test]
fn m_d10000_y0_twothirds() {
    let (_env, user, _token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &10000), 9000);
    // pas de rendement simule
    assert_eq!(vault.total_assets(), 10000);
    assert_eq!(vault.withdraw(&user, &6000), 6000);
    assert_eq!(vault.shares_of(&user), 3000);
    assert_eq!(vault.total_shares(), 4000);
    assert_eq!(vault.total_assets(), 4000);
    // Solvabilite relue depuis le contrat (l'oracle attend 3000).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 3000);
}

#[test]
fn m_d10000_y0_one() {
    let (_env, user, _token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &10000), 9000);
    // pas de rendement simule
    assert_eq!(vault.total_assets(), 10000);
    assert_eq!(vault.withdraw(&user, &1), 1);
    assert_eq!(vault.shares_of(&user), 8999);
    assert_eq!(vault.total_shares(), 9999);
    assert_eq!(vault.total_assets(), 9999);
    // Solvabilite relue depuis le contrat (l'oracle attend 8999).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 8999);
}

#[test]
fn m_d10000_y1_all() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &10000), 9000);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &1);
    assert_eq!(vault.total_assets(), 10001);
    assert_eq!(vault.withdraw(&user, &9000), 9000);
    assert_eq!(vault.shares_of(&user), 0);
    assert_eq!(vault.total_shares(), 1000);
    assert_eq!(vault.total_assets(), 1001);
    // Solvabilite relue depuis le contrat (l'oracle attend 0).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 0);
}

#[test]
fn m_d10000_y1_half() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &10000), 9000);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &1);
    assert_eq!(vault.total_assets(), 10001);
    assert_eq!(vault.withdraw(&user, &4500), 4500);
    assert_eq!(vault.shares_of(&user), 4500);
    assert_eq!(vault.total_shares(), 5500);
    assert_eq!(vault.total_assets(), 5501);
    // Solvabilite relue depuis le contrat (l'oracle attend 4500).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 4500);
}

#[test]
fn m_d10000_y1_third() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &10000), 9000);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &1);
    assert_eq!(vault.total_assets(), 10001);
    assert_eq!(vault.withdraw(&user, &3000), 3000);
    assert_eq!(vault.shares_of(&user), 6000);
    assert_eq!(vault.total_shares(), 7000);
    assert_eq!(vault.total_assets(), 7001);
    // Solvabilite relue depuis le contrat (l'oracle attend 6000).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 6000);
}

#[test]
fn m_d10000_y1_twothirds() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &10000), 9000);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &1);
    assert_eq!(vault.total_assets(), 10001);
    assert_eq!(vault.withdraw(&user, &6000), 6000);
    assert_eq!(vault.shares_of(&user), 3000);
    assert_eq!(vault.total_shares(), 4000);
    assert_eq!(vault.total_assets(), 4001);
    // Solvabilite relue depuis le contrat (l'oracle attend 3000).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 3000);
}

#[test]
fn m_d10000_y1_one() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &10000), 9000);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &1);
    assert_eq!(vault.total_assets(), 10001);
    assert_eq!(vault.withdraw(&user, &1), 1);
    assert_eq!(vault.shares_of(&user), 8999);
    assert_eq!(vault.total_shares(), 9999);
    assert_eq!(vault.total_assets(), 10000);
    // Solvabilite relue depuis le contrat (l'oracle attend 8999).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 8999);
}

#[test]
fn m_d10000_y999_all() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &10000), 9000);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &999);
    assert_eq!(vault.total_assets(), 10999);
    assert_eq!(vault.withdraw(&user, &9000), 9899);
    assert_eq!(vault.shares_of(&user), 0);
    assert_eq!(vault.total_shares(), 1000);
    assert_eq!(vault.total_assets(), 1100);
    // Solvabilite relue depuis le contrat (l'oracle attend 0).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 0);
}

#[test]
fn m_d10000_y999_half() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &10000), 9000);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &999);
    assert_eq!(vault.total_assets(), 10999);
    assert_eq!(vault.withdraw(&user, &4500), 4949);
    assert_eq!(vault.shares_of(&user), 4500);
    assert_eq!(vault.total_shares(), 5500);
    assert_eq!(vault.total_assets(), 6050);
    // Solvabilite relue depuis le contrat (l'oracle attend 4950).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 4950);
}

#[test]
fn m_d10000_y999_third() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &10000), 9000);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &999);
    assert_eq!(vault.total_assets(), 10999);
    assert_eq!(vault.withdraw(&user, &3000), 3299);
    assert_eq!(vault.shares_of(&user), 6000);
    assert_eq!(vault.total_shares(), 7000);
    assert_eq!(vault.total_assets(), 7700);
    // Solvabilite relue depuis le contrat (l'oracle attend 6600).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 6600);
}

#[test]
fn m_d10000_y999_twothirds() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &10000), 9000);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &999);
    assert_eq!(vault.total_assets(), 10999);
    assert_eq!(vault.withdraw(&user, &6000), 6599);
    assert_eq!(vault.shares_of(&user), 3000);
    assert_eq!(vault.total_shares(), 4000);
    assert_eq!(vault.total_assets(), 4400);
    // Solvabilite relue depuis le contrat (l'oracle attend 3300).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 3300);
}

#[test]
fn m_d10000_y999_one() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &10000), 9000);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &999);
    assert_eq!(vault.total_assets(), 10999);
    assert_eq!(vault.withdraw(&user, &1), 1);
    assert_eq!(vault.shares_of(&user), 8999);
    assert_eq!(vault.total_shares(), 9999);
    assert_eq!(vault.total_assets(), 10998);
    // Solvabilite relue depuis le contrat (l'oracle attend 9898).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 9898);
}

#[test]
fn m_d10000_y10000_all() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &10000), 9000);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &10000);
    assert_eq!(vault.total_assets(), 20000);
    assert_eq!(vault.withdraw(&user, &9000), 18000);
    assert_eq!(vault.shares_of(&user), 0);
    assert_eq!(vault.total_shares(), 1000);
    assert_eq!(vault.total_assets(), 2000);
    // Solvabilite relue depuis le contrat (l'oracle attend 0).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 0);
}

#[test]
fn m_d10000_y10000_half() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &10000), 9000);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &10000);
    assert_eq!(vault.total_assets(), 20000);
    assert_eq!(vault.withdraw(&user, &4500), 9000);
    assert_eq!(vault.shares_of(&user), 4500);
    assert_eq!(vault.total_shares(), 5500);
    assert_eq!(vault.total_assets(), 11000);
    // Solvabilite relue depuis le contrat (l'oracle attend 9000).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 9000);
}

#[test]
fn m_d10000_y10000_third() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &10000), 9000);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &10000);
    assert_eq!(vault.total_assets(), 20000);
    assert_eq!(vault.withdraw(&user, &3000), 6000);
    assert_eq!(vault.shares_of(&user), 6000);
    assert_eq!(vault.total_shares(), 7000);
    assert_eq!(vault.total_assets(), 14000);
    // Solvabilite relue depuis le contrat (l'oracle attend 12000).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 12000);
}

#[test]
fn m_d10000_y10000_twothirds() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &10000), 9000);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &10000);
    assert_eq!(vault.total_assets(), 20000);
    assert_eq!(vault.withdraw(&user, &6000), 12000);
    assert_eq!(vault.shares_of(&user), 3000);
    assert_eq!(vault.total_shares(), 4000);
    assert_eq!(vault.total_assets(), 8000);
    // Solvabilite relue depuis le contrat (l'oracle attend 6000).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 6000);
}

#[test]
fn m_d10000_y10000_one() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &10000), 9000);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &10000);
    assert_eq!(vault.total_assets(), 20000);
    assert_eq!(vault.withdraw(&user, &1), 2);
    assert_eq!(vault.shares_of(&user), 8999);
    assert_eq!(vault.total_shares(), 9999);
    assert_eq!(vault.total_assets(), 19998);
    // Solvabilite relue depuis le contrat (l'oracle attend 17998).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 17998);
}

#[test]
fn m_d10000_y123457_all() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &10000), 9000);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &123457);
    assert_eq!(vault.total_assets(), 133457);
    assert_eq!(vault.withdraw(&user, &9000), 120111);
    assert_eq!(vault.shares_of(&user), 0);
    assert_eq!(vault.total_shares(), 1000);
    assert_eq!(vault.total_assets(), 13346);
    // Solvabilite relue depuis le contrat (l'oracle attend 0).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 0);
}

#[test]
fn m_d10000_y123457_half() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &10000), 9000);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &123457);
    assert_eq!(vault.total_assets(), 133457);
    assert_eq!(vault.withdraw(&user, &4500), 60055);
    assert_eq!(vault.shares_of(&user), 4500);
    assert_eq!(vault.total_shares(), 5500);
    assert_eq!(vault.total_assets(), 73402);
    // Solvabilite relue depuis le contrat (l'oracle attend 60056).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 60056);
}

#[test]
fn m_d10000_y123457_third() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &10000), 9000);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &123457);
    assert_eq!(vault.total_assets(), 133457);
    assert_eq!(vault.withdraw(&user, &3000), 40037);
    assert_eq!(vault.shares_of(&user), 6000);
    assert_eq!(vault.total_shares(), 7000);
    assert_eq!(vault.total_assets(), 93420);
    // Solvabilite relue depuis le contrat (l'oracle attend 80074).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 80074);
}

#[test]
fn m_d10000_y123457_twothirds() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &10000), 9000);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &123457);
    assert_eq!(vault.total_assets(), 133457);
    assert_eq!(vault.withdraw(&user, &6000), 80074);
    assert_eq!(vault.shares_of(&user), 3000);
    assert_eq!(vault.total_shares(), 4000);
    assert_eq!(vault.total_assets(), 53383);
    // Solvabilite relue depuis le contrat (l'oracle attend 40037).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 40037);
}

#[test]
fn m_d10000_y123457_one() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &10000), 9000);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &123457);
    assert_eq!(vault.total_assets(), 133457);
    assert_eq!(vault.withdraw(&user, &1), 13);
    assert_eq!(vault.shares_of(&user), 8999);
    assert_eq!(vault.total_shares(), 9999);
    assert_eq!(vault.total_assets(), 133444);
    // Solvabilite relue depuis le contrat (l'oracle attend 120098).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 120098);
}

#[test]
fn m_d33333_y0_all() {
    let (_env, user, _token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &33333), 32333);
    // pas de rendement simule
    assert_eq!(vault.total_assets(), 33333);
    assert_eq!(vault.withdraw(&user, &32333), 32333);
    assert_eq!(vault.shares_of(&user), 0);
    assert_eq!(vault.total_shares(), 1000);
    assert_eq!(vault.total_assets(), 1000);
    // Solvabilite relue depuis le contrat (l'oracle attend 0).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 0);
}

#[test]
fn m_d33333_y0_half() {
    let (_env, user, _token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &33333), 32333);
    // pas de rendement simule
    assert_eq!(vault.total_assets(), 33333);
    assert_eq!(vault.withdraw(&user, &16166), 16166);
    assert_eq!(vault.shares_of(&user), 16167);
    assert_eq!(vault.total_shares(), 17167);
    assert_eq!(vault.total_assets(), 17167);
    // Solvabilite relue depuis le contrat (l'oracle attend 16167).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 16167);
}

#[test]
fn m_d33333_y0_third() {
    let (_env, user, _token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &33333), 32333);
    // pas de rendement simule
    assert_eq!(vault.total_assets(), 33333);
    assert_eq!(vault.withdraw(&user, &10777), 10777);
    assert_eq!(vault.shares_of(&user), 21556);
    assert_eq!(vault.total_shares(), 22556);
    assert_eq!(vault.total_assets(), 22556);
    // Solvabilite relue depuis le contrat (l'oracle attend 21556).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 21556);
}

#[test]
fn m_d33333_y0_twothirds() {
    let (_env, user, _token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &33333), 32333);
    // pas de rendement simule
    assert_eq!(vault.total_assets(), 33333);
    assert_eq!(vault.withdraw(&user, &21555), 21555);
    assert_eq!(vault.shares_of(&user), 10778);
    assert_eq!(vault.total_shares(), 11778);
    assert_eq!(vault.total_assets(), 11778);
    // Solvabilite relue depuis le contrat (l'oracle attend 10778).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 10778);
}

#[test]
fn m_d33333_y0_one() {
    let (_env, user, _token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &33333), 32333);
    // pas de rendement simule
    assert_eq!(vault.total_assets(), 33333);
    assert_eq!(vault.withdraw(&user, &1), 1);
    assert_eq!(vault.shares_of(&user), 32332);
    assert_eq!(vault.total_shares(), 33332);
    assert_eq!(vault.total_assets(), 33332);
    // Solvabilite relue depuis le contrat (l'oracle attend 32332).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 32332);
}

#[test]
fn m_d33333_y1_all() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &33333), 32333);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &1);
    assert_eq!(vault.total_assets(), 33334);
    assert_eq!(vault.withdraw(&user, &32333), 32333);
    assert_eq!(vault.shares_of(&user), 0);
    assert_eq!(vault.total_shares(), 1000);
    assert_eq!(vault.total_assets(), 1001);
    // Solvabilite relue depuis le contrat (l'oracle attend 0).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 0);
}

#[test]
fn m_d33333_y1_half() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &33333), 32333);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &1);
    assert_eq!(vault.total_assets(), 33334);
    assert_eq!(vault.withdraw(&user, &16166), 16166);
    assert_eq!(vault.shares_of(&user), 16167);
    assert_eq!(vault.total_shares(), 17167);
    assert_eq!(vault.total_assets(), 17168);
    // Solvabilite relue depuis le contrat (l'oracle attend 16167).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 16167);
}

#[test]
fn m_d33333_y1_third() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &33333), 32333);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &1);
    assert_eq!(vault.total_assets(), 33334);
    assert_eq!(vault.withdraw(&user, &10777), 10777);
    assert_eq!(vault.shares_of(&user), 21556);
    assert_eq!(vault.total_shares(), 22556);
    assert_eq!(vault.total_assets(), 22557);
    // Solvabilite relue depuis le contrat (l'oracle attend 21556).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 21556);
}

#[test]
fn m_d33333_y1_twothirds() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &33333), 32333);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &1);
    assert_eq!(vault.total_assets(), 33334);
    assert_eq!(vault.withdraw(&user, &21555), 21555);
    assert_eq!(vault.shares_of(&user), 10778);
    assert_eq!(vault.total_shares(), 11778);
    assert_eq!(vault.total_assets(), 11779);
    // Solvabilite relue depuis le contrat (l'oracle attend 10778).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 10778);
}

#[test]
fn m_d33333_y1_one() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &33333), 32333);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &1);
    assert_eq!(vault.total_assets(), 33334);
    assert_eq!(vault.withdraw(&user, &1), 1);
    assert_eq!(vault.shares_of(&user), 32332);
    assert_eq!(vault.total_shares(), 33332);
    assert_eq!(vault.total_assets(), 33333);
    // Solvabilite relue depuis le contrat (l'oracle attend 32332).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 32332);
}

#[test]
fn m_d33333_y999_all() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &33333), 32333);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &999);
    assert_eq!(vault.total_assets(), 34332);
    assert_eq!(vault.withdraw(&user, &32333), 33302);
    assert_eq!(vault.shares_of(&user), 0);
    assert_eq!(vault.total_shares(), 1000);
    assert_eq!(vault.total_assets(), 1030);
    // Solvabilite relue depuis le contrat (l'oracle attend 0).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 0);
}

#[test]
fn m_d33333_y999_half() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &33333), 32333);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &999);
    assert_eq!(vault.total_assets(), 34332);
    assert_eq!(vault.withdraw(&user, &16166), 16650);
    assert_eq!(vault.shares_of(&user), 16167);
    assert_eq!(vault.total_shares(), 17167);
    assert_eq!(vault.total_assets(), 17682);
    // Solvabilite relue depuis le contrat (l'oracle attend 16652).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 16652);
}

#[test]
fn m_d33333_y999_third() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &33333), 32333);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &999);
    assert_eq!(vault.total_assets(), 34332);
    assert_eq!(vault.withdraw(&user, &10777), 11099);
    assert_eq!(vault.shares_of(&user), 21556);
    assert_eq!(vault.total_shares(), 22556);
    assert_eq!(vault.total_assets(), 23233);
    // Solvabilite relue depuis le contrat (l'oracle attend 22202).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 22202);
}

#[test]
fn m_d33333_y999_twothirds() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &33333), 32333);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &999);
    assert_eq!(vault.total_assets(), 34332);
    assert_eq!(vault.withdraw(&user, &21555), 22201);
    assert_eq!(vault.shares_of(&user), 10778);
    assert_eq!(vault.total_shares(), 11778);
    assert_eq!(vault.total_assets(), 12131);
    // Solvabilite relue depuis le contrat (l'oracle attend 11101).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 11101);
}

#[test]
fn m_d33333_y999_one() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &33333), 32333);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &999);
    assert_eq!(vault.total_assets(), 34332);
    assert_eq!(vault.withdraw(&user, &1), 1);
    assert_eq!(vault.shares_of(&user), 32332);
    assert_eq!(vault.total_shares(), 33332);
    assert_eq!(vault.total_assets(), 34331);
    // Solvabilite relue depuis le contrat (l'oracle attend 33301).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 33301);
}

#[test]
fn m_d33333_y10000_all() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &33333), 32333);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &10000);
    assert_eq!(vault.total_assets(), 43333);
    assert_eq!(vault.withdraw(&user, &32333), 42032);
    assert_eq!(vault.shares_of(&user), 0);
    assert_eq!(vault.total_shares(), 1000);
    assert_eq!(vault.total_assets(), 1301);
    // Solvabilite relue depuis le contrat (l'oracle attend 0).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 0);
}

#[test]
fn m_d33333_y10000_half() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &33333), 32333);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &10000);
    assert_eq!(vault.total_assets(), 43333);
    assert_eq!(vault.withdraw(&user, &16166), 21015);
    assert_eq!(vault.shares_of(&user), 16167);
    assert_eq!(vault.total_shares(), 17167);
    assert_eq!(vault.total_assets(), 22318);
    // Solvabilite relue depuis le contrat (l'oracle attend 21017).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 21017);
}

#[test]
fn m_d33333_y10000_third() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &33333), 32333);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &10000);
    assert_eq!(vault.total_assets(), 43333);
    assert_eq!(vault.withdraw(&user, &10777), 14010);
    assert_eq!(vault.shares_of(&user), 21556);
    assert_eq!(vault.total_shares(), 22556);
    assert_eq!(vault.total_assets(), 29323);
    // Solvabilite relue depuis le contrat (l'oracle attend 28022).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 28022);
}

#[test]
fn m_d33333_y10000_twothirds() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &33333), 32333);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &10000);
    assert_eq!(vault.total_assets(), 43333);
    assert_eq!(vault.withdraw(&user, &21555), 28021);
    assert_eq!(vault.shares_of(&user), 10778);
    assert_eq!(vault.total_shares(), 11778);
    assert_eq!(vault.total_assets(), 15312);
    // Solvabilite relue depuis le contrat (l'oracle attend 14011).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 14011);
}

#[test]
fn m_d33333_y10000_one() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &33333), 32333);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &10000);
    assert_eq!(vault.total_assets(), 43333);
    assert_eq!(vault.withdraw(&user, &1), 1);
    assert_eq!(vault.shares_of(&user), 32332);
    assert_eq!(vault.total_shares(), 33332);
    assert_eq!(vault.total_assets(), 43332);
    // Solvabilite relue depuis le contrat (l'oracle attend 42031).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 42031);
}

#[test]
fn m_d33333_y123457_all() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &33333), 32333);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &123457);
    assert_eq!(vault.total_assets(), 156790);
    assert_eq!(vault.withdraw(&user, &32333), 152086);
    assert_eq!(vault.shares_of(&user), 0);
    assert_eq!(vault.total_shares(), 1000);
    assert_eq!(vault.total_assets(), 4704);
    // Solvabilite relue depuis le contrat (l'oracle attend 0).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 0);
}

#[test]
fn m_d33333_y123457_half() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &33333), 32333);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &123457);
    assert_eq!(vault.total_assets(), 156790);
    assert_eq!(vault.withdraw(&user, &16166), 76040);
    assert_eq!(vault.shares_of(&user), 16167);
    assert_eq!(vault.total_shares(), 17167);
    assert_eq!(vault.total_assets(), 80750);
    // Solvabilite relue depuis le contrat (l'oracle attend 76046).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 76046);
}

#[test]
fn m_d33333_y123457_third() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &33333), 32333);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &123457);
    assert_eq!(vault.total_assets(), 156790);
    assert_eq!(vault.withdraw(&user, &10777), 50692);
    assert_eq!(vault.shares_of(&user), 21556);
    assert_eq!(vault.total_shares(), 22556);
    assert_eq!(vault.total_assets(), 106098);
    // Solvabilite relue depuis le contrat (l'oracle attend 101394).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 101394);
}

#[test]
fn m_d33333_y123457_twothirds() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &33333), 32333);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &123457);
    assert_eq!(vault.total_assets(), 156790);
    assert_eq!(vault.withdraw(&user, &21555), 101389);
    assert_eq!(vault.shares_of(&user), 10778);
    assert_eq!(vault.total_shares(), 11778);
    assert_eq!(vault.total_assets(), 55401);
    // Solvabilite relue depuis le contrat (l'oracle attend 50697).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 50697);
}

#[test]
fn m_d33333_y123457_one() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &33333), 32333);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &123457);
    assert_eq!(vault.total_assets(), 156790);
    assert_eq!(vault.withdraw(&user, &1), 4);
    assert_eq!(vault.shares_of(&user), 32332);
    assert_eq!(vault.total_shares(), 33332);
    assert_eq!(vault.total_assets(), 156786);
    // Solvabilite relue depuis le contrat (l'oracle attend 152082).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 152082);
}

#[test]
fn m_d100000_y0_all() {
    let (_env, user, _token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &100000), 99000);
    // pas de rendement simule
    assert_eq!(vault.total_assets(), 100000);
    assert_eq!(vault.withdraw(&user, &99000), 99000);
    assert_eq!(vault.shares_of(&user), 0);
    assert_eq!(vault.total_shares(), 1000);
    assert_eq!(vault.total_assets(), 1000);
    // Solvabilite relue depuis le contrat (l'oracle attend 0).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 0);
}

#[test]
fn m_d100000_y0_half() {
    let (_env, user, _token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &100000), 99000);
    // pas de rendement simule
    assert_eq!(vault.total_assets(), 100000);
    assert_eq!(vault.withdraw(&user, &49500), 49500);
    assert_eq!(vault.shares_of(&user), 49500);
    assert_eq!(vault.total_shares(), 50500);
    assert_eq!(vault.total_assets(), 50500);
    // Solvabilite relue depuis le contrat (l'oracle attend 49500).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 49500);
}

#[test]
fn m_d100000_y0_third() {
    let (_env, user, _token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &100000), 99000);
    // pas de rendement simule
    assert_eq!(vault.total_assets(), 100000);
    assert_eq!(vault.withdraw(&user, &33000), 33000);
    assert_eq!(vault.shares_of(&user), 66000);
    assert_eq!(vault.total_shares(), 67000);
    assert_eq!(vault.total_assets(), 67000);
    // Solvabilite relue depuis le contrat (l'oracle attend 66000).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 66000);
}

#[test]
fn m_d100000_y0_twothirds() {
    let (_env, user, _token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &100000), 99000);
    // pas de rendement simule
    assert_eq!(vault.total_assets(), 100000);
    assert_eq!(vault.withdraw(&user, &66000), 66000);
    assert_eq!(vault.shares_of(&user), 33000);
    assert_eq!(vault.total_shares(), 34000);
    assert_eq!(vault.total_assets(), 34000);
    // Solvabilite relue depuis le contrat (l'oracle attend 33000).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 33000);
}

#[test]
fn m_d100000_y0_one() {
    let (_env, user, _token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &100000), 99000);
    // pas de rendement simule
    assert_eq!(vault.total_assets(), 100000);
    assert_eq!(vault.withdraw(&user, &1), 1);
    assert_eq!(vault.shares_of(&user), 98999);
    assert_eq!(vault.total_shares(), 99999);
    assert_eq!(vault.total_assets(), 99999);
    // Solvabilite relue depuis le contrat (l'oracle attend 98999).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 98999);
}

#[test]
fn m_d100000_y1_all() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &100000), 99000);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &1);
    assert_eq!(vault.total_assets(), 100001);
    assert_eq!(vault.withdraw(&user, &99000), 99000);
    assert_eq!(vault.shares_of(&user), 0);
    assert_eq!(vault.total_shares(), 1000);
    assert_eq!(vault.total_assets(), 1001);
    // Solvabilite relue depuis le contrat (l'oracle attend 0).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 0);
}

#[test]
fn m_d100000_y1_half() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &100000), 99000);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &1);
    assert_eq!(vault.total_assets(), 100001);
    assert_eq!(vault.withdraw(&user, &49500), 49500);
    assert_eq!(vault.shares_of(&user), 49500);
    assert_eq!(vault.total_shares(), 50500);
    assert_eq!(vault.total_assets(), 50501);
    // Solvabilite relue depuis le contrat (l'oracle attend 49500).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 49500);
}

#[test]
fn m_d100000_y1_third() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &100000), 99000);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &1);
    assert_eq!(vault.total_assets(), 100001);
    assert_eq!(vault.withdraw(&user, &33000), 33000);
    assert_eq!(vault.shares_of(&user), 66000);
    assert_eq!(vault.total_shares(), 67000);
    assert_eq!(vault.total_assets(), 67001);
    // Solvabilite relue depuis le contrat (l'oracle attend 66000).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 66000);
}

#[test]
fn m_d100000_y1_twothirds() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &100000), 99000);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &1);
    assert_eq!(vault.total_assets(), 100001);
    assert_eq!(vault.withdraw(&user, &66000), 66000);
    assert_eq!(vault.shares_of(&user), 33000);
    assert_eq!(vault.total_shares(), 34000);
    assert_eq!(vault.total_assets(), 34001);
    // Solvabilite relue depuis le contrat (l'oracle attend 33000).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 33000);
}

#[test]
fn m_d100000_y1_one() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &100000), 99000);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &1);
    assert_eq!(vault.total_assets(), 100001);
    assert_eq!(vault.withdraw(&user, &1), 1);
    assert_eq!(vault.shares_of(&user), 98999);
    assert_eq!(vault.total_shares(), 99999);
    assert_eq!(vault.total_assets(), 100000);
    // Solvabilite relue depuis le contrat (l'oracle attend 98999).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 98999);
}

#[test]
fn m_d100000_y999_all() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &100000), 99000);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &999);
    assert_eq!(vault.total_assets(), 100999);
    assert_eq!(vault.withdraw(&user, &99000), 99989);
    assert_eq!(vault.shares_of(&user), 0);
    assert_eq!(vault.total_shares(), 1000);
    assert_eq!(vault.total_assets(), 1010);
    // Solvabilite relue depuis le contrat (l'oracle attend 0).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 0);
}

#[test]
fn m_d100000_y999_half() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &100000), 99000);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &999);
    assert_eq!(vault.total_assets(), 100999);
    assert_eq!(vault.withdraw(&user, &49500), 49994);
    assert_eq!(vault.shares_of(&user), 49500);
    assert_eq!(vault.total_shares(), 50500);
    assert_eq!(vault.total_assets(), 51005);
    // Solvabilite relue depuis le contrat (l'oracle attend 49995).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 49995);
}

#[test]
fn m_d100000_y999_third() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &100000), 99000);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &999);
    assert_eq!(vault.total_assets(), 100999);
    assert_eq!(vault.withdraw(&user, &33000), 33329);
    assert_eq!(vault.shares_of(&user), 66000);
    assert_eq!(vault.total_shares(), 67000);
    assert_eq!(vault.total_assets(), 67670);
    // Solvabilite relue depuis le contrat (l'oracle attend 66660).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 66660);
}

#[test]
fn m_d100000_y999_twothirds() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &100000), 99000);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &999);
    assert_eq!(vault.total_assets(), 100999);
    assert_eq!(vault.withdraw(&user, &66000), 66659);
    assert_eq!(vault.shares_of(&user), 33000);
    assert_eq!(vault.total_shares(), 34000);
    assert_eq!(vault.total_assets(), 34340);
    // Solvabilite relue depuis le contrat (l'oracle attend 33330).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 33330);
}

#[test]
fn m_d100000_y999_one() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &100000), 99000);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &999);
    assert_eq!(vault.total_assets(), 100999);
    assert_eq!(vault.withdraw(&user, &1), 1);
    assert_eq!(vault.shares_of(&user), 98999);
    assert_eq!(vault.total_shares(), 99999);
    assert_eq!(vault.total_assets(), 100998);
    // Solvabilite relue depuis le contrat (l'oracle attend 99988).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 99988);
}

#[test]
fn m_d100000_y10000_all() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &100000), 99000);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &10000);
    assert_eq!(vault.total_assets(), 110000);
    assert_eq!(vault.withdraw(&user, &99000), 108900);
    assert_eq!(vault.shares_of(&user), 0);
    assert_eq!(vault.total_shares(), 1000);
    assert_eq!(vault.total_assets(), 1100);
    // Solvabilite relue depuis le contrat (l'oracle attend 0).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 0);
}

#[test]
fn m_d100000_y10000_half() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &100000), 99000);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &10000);
    assert_eq!(vault.total_assets(), 110000);
    assert_eq!(vault.withdraw(&user, &49500), 54450);
    assert_eq!(vault.shares_of(&user), 49500);
    assert_eq!(vault.total_shares(), 50500);
    assert_eq!(vault.total_assets(), 55550);
    // Solvabilite relue depuis le contrat (l'oracle attend 54450).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 54450);
}

#[test]
fn m_d100000_y10000_third() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &100000), 99000);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &10000);
    assert_eq!(vault.total_assets(), 110000);
    assert_eq!(vault.withdraw(&user, &33000), 36300);
    assert_eq!(vault.shares_of(&user), 66000);
    assert_eq!(vault.total_shares(), 67000);
    assert_eq!(vault.total_assets(), 73700);
    // Solvabilite relue depuis le contrat (l'oracle attend 72600).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 72600);
}

#[test]
fn m_d100000_y10000_twothirds() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &100000), 99000);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &10000);
    assert_eq!(vault.total_assets(), 110000);
    assert_eq!(vault.withdraw(&user, &66000), 72600);
    assert_eq!(vault.shares_of(&user), 33000);
    assert_eq!(vault.total_shares(), 34000);
    assert_eq!(vault.total_assets(), 37400);
    // Solvabilite relue depuis le contrat (l'oracle attend 36300).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 36300);
}

#[test]
fn m_d100000_y10000_one() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &100000), 99000);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &10000);
    assert_eq!(vault.total_assets(), 110000);
    assert_eq!(vault.withdraw(&user, &1), 1);
    assert_eq!(vault.shares_of(&user), 98999);
    assert_eq!(vault.total_shares(), 99999);
    assert_eq!(vault.total_assets(), 109999);
    // Solvabilite relue depuis le contrat (l'oracle attend 108898).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 108898);
}

#[test]
fn m_d100000_y123457_all() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &100000), 99000);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &123457);
    assert_eq!(vault.total_assets(), 223457);
    assert_eq!(vault.withdraw(&user, &99000), 221222);
    assert_eq!(vault.shares_of(&user), 0);
    assert_eq!(vault.total_shares(), 1000);
    assert_eq!(vault.total_assets(), 2235);
    // Solvabilite relue depuis le contrat (l'oracle attend 0).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 0);
}

#[test]
fn m_d100000_y123457_half() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &100000), 99000);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &123457);
    assert_eq!(vault.total_assets(), 223457);
    assert_eq!(vault.withdraw(&user, &49500), 110611);
    assert_eq!(vault.shares_of(&user), 49500);
    assert_eq!(vault.total_shares(), 50500);
    assert_eq!(vault.total_assets(), 112846);
    // Solvabilite relue depuis le contrat (l'oracle attend 110611).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 110611);
}

#[test]
fn m_d100000_y123457_third() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &100000), 99000);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &123457);
    assert_eq!(vault.total_assets(), 223457);
    assert_eq!(vault.withdraw(&user, &33000), 73740);
    assert_eq!(vault.shares_of(&user), 66000);
    assert_eq!(vault.total_shares(), 67000);
    assert_eq!(vault.total_assets(), 149717);
    // Solvabilite relue depuis le contrat (l'oracle attend 147482).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 147482);
}

#[test]
fn m_d100000_y123457_twothirds() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &100000), 99000);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &123457);
    assert_eq!(vault.total_assets(), 223457);
    assert_eq!(vault.withdraw(&user, &66000), 147481);
    assert_eq!(vault.shares_of(&user), 33000);
    assert_eq!(vault.total_shares(), 34000);
    assert_eq!(vault.total_assets(), 75976);
    // Solvabilite relue depuis le contrat (l'oracle attend 73741).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 73741);
}

#[test]
fn m_d100000_y123457_one() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &100000), 99000);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &123457);
    assert_eq!(vault.total_assets(), 223457);
    assert_eq!(vault.withdraw(&user, &1), 2);
    assert_eq!(vault.shares_of(&user), 98999);
    assert_eq!(vault.total_shares(), 99999);
    assert_eq!(vault.total_assets(), 223455);
    // Solvabilite relue depuis le contrat (l'oracle attend 221220).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 221220);
}

#[test]
fn m_d1234567_y0_all() {
    let (_env, user, _token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1234567), 1233567);
    // pas de rendement simule
    assert_eq!(vault.total_assets(), 1234567);
    assert_eq!(vault.withdraw(&user, &1233567), 1233567);
    assert_eq!(vault.shares_of(&user), 0);
    assert_eq!(vault.total_shares(), 1000);
    assert_eq!(vault.total_assets(), 1000);
    // Solvabilite relue depuis le contrat (l'oracle attend 0).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 0);
}

#[test]
fn m_d1234567_y0_half() {
    let (_env, user, _token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1234567), 1233567);
    // pas de rendement simule
    assert_eq!(vault.total_assets(), 1234567);
    assert_eq!(vault.withdraw(&user, &616783), 616783);
    assert_eq!(vault.shares_of(&user), 616784);
    assert_eq!(vault.total_shares(), 617784);
    assert_eq!(vault.total_assets(), 617784);
    // Solvabilite relue depuis le contrat (l'oracle attend 616784).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 616784);
}

#[test]
fn m_d1234567_y0_third() {
    let (_env, user, _token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1234567), 1233567);
    // pas de rendement simule
    assert_eq!(vault.total_assets(), 1234567);
    assert_eq!(vault.withdraw(&user, &411189), 411189);
    assert_eq!(vault.shares_of(&user), 822378);
    assert_eq!(vault.total_shares(), 823378);
    assert_eq!(vault.total_assets(), 823378);
    // Solvabilite relue depuis le contrat (l'oracle attend 822378).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 822378);
}

#[test]
fn m_d1234567_y0_twothirds() {
    let (_env, user, _token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1234567), 1233567);
    // pas de rendement simule
    assert_eq!(vault.total_assets(), 1234567);
    assert_eq!(vault.withdraw(&user, &822378), 822378);
    assert_eq!(vault.shares_of(&user), 411189);
    assert_eq!(vault.total_shares(), 412189);
    assert_eq!(vault.total_assets(), 412189);
    // Solvabilite relue depuis le contrat (l'oracle attend 411189).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 411189);
}

#[test]
fn m_d1234567_y0_one() {
    let (_env, user, _token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1234567), 1233567);
    // pas de rendement simule
    assert_eq!(vault.total_assets(), 1234567);
    assert_eq!(vault.withdraw(&user, &1), 1);
    assert_eq!(vault.shares_of(&user), 1233566);
    assert_eq!(vault.total_shares(), 1234566);
    assert_eq!(vault.total_assets(), 1234566);
    // Solvabilite relue depuis le contrat (l'oracle attend 1233566).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 1233566);
}

#[test]
fn m_d1234567_y1_all() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1234567), 1233567);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &1);
    assert_eq!(vault.total_assets(), 1234568);
    assert_eq!(vault.withdraw(&user, &1233567), 1233567);
    assert_eq!(vault.shares_of(&user), 0);
    assert_eq!(vault.total_shares(), 1000);
    assert_eq!(vault.total_assets(), 1001);
    // Solvabilite relue depuis le contrat (l'oracle attend 0).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 0);
}

#[test]
fn m_d1234567_y1_half() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1234567), 1233567);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &1);
    assert_eq!(vault.total_assets(), 1234568);
    assert_eq!(vault.withdraw(&user, &616783), 616783);
    assert_eq!(vault.shares_of(&user), 616784);
    assert_eq!(vault.total_shares(), 617784);
    assert_eq!(vault.total_assets(), 617785);
    // Solvabilite relue depuis le contrat (l'oracle attend 616784).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 616784);
}

#[test]
fn m_d1234567_y1_third() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1234567), 1233567);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &1);
    assert_eq!(vault.total_assets(), 1234568);
    assert_eq!(vault.withdraw(&user, &411189), 411189);
    assert_eq!(vault.shares_of(&user), 822378);
    assert_eq!(vault.total_shares(), 823378);
    assert_eq!(vault.total_assets(), 823379);
    // Solvabilite relue depuis le contrat (l'oracle attend 822378).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 822378);
}

#[test]
fn m_d1234567_y1_twothirds() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1234567), 1233567);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &1);
    assert_eq!(vault.total_assets(), 1234568);
    assert_eq!(vault.withdraw(&user, &822378), 822378);
    assert_eq!(vault.shares_of(&user), 411189);
    assert_eq!(vault.total_shares(), 412189);
    assert_eq!(vault.total_assets(), 412190);
    // Solvabilite relue depuis le contrat (l'oracle attend 411189).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 411189);
}

#[test]
fn m_d1234567_y1_one() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1234567), 1233567);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &1);
    assert_eq!(vault.total_assets(), 1234568);
    assert_eq!(vault.withdraw(&user, &1), 1);
    assert_eq!(vault.shares_of(&user), 1233566);
    assert_eq!(vault.total_shares(), 1234566);
    assert_eq!(vault.total_assets(), 1234567);
    // Solvabilite relue depuis le contrat (l'oracle attend 1233566).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 1233566);
}

#[test]
fn m_d1234567_y999_all() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1234567), 1233567);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &999);
    assert_eq!(vault.total_assets(), 1235566);
    assert_eq!(vault.withdraw(&user, &1233567), 1234565);
    assert_eq!(vault.shares_of(&user), 0);
    assert_eq!(vault.total_shares(), 1000);
    assert_eq!(vault.total_assets(), 1001);
    // Solvabilite relue depuis le contrat (l'oracle attend 0).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 0);
}

#[test]
fn m_d1234567_y999_half() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1234567), 1233567);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &999);
    assert_eq!(vault.total_assets(), 1235566);
    assert_eq!(vault.withdraw(&user, &616783), 617282);
    assert_eq!(vault.shares_of(&user), 616784);
    assert_eq!(vault.total_shares(), 617784);
    assert_eq!(vault.total_assets(), 618284);
    // Solvabilite relue depuis le contrat (l'oracle attend 617283).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 617283);
}

#[test]
fn m_d1234567_y999_third() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1234567), 1233567);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &999);
    assert_eq!(vault.total_assets(), 1235566);
    assert_eq!(vault.withdraw(&user, &411189), 411521);
    assert_eq!(vault.shares_of(&user), 822378);
    assert_eq!(vault.total_shares(), 823378);
    assert_eq!(vault.total_assets(), 824045);
    // Solvabilite relue depuis le contrat (l'oracle attend 823044).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 823044);
}

#[test]
fn m_d1234567_y999_twothirds() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1234567), 1233567);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &999);
    assert_eq!(vault.total_assets(), 1235566);
    assert_eq!(vault.withdraw(&user, &822378), 823043);
    assert_eq!(vault.shares_of(&user), 411189);
    assert_eq!(vault.total_shares(), 412189);
    assert_eq!(vault.total_assets(), 412523);
    // Solvabilite relue depuis le contrat (l'oracle attend 411522).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 411522);
}

#[test]
fn m_d1234567_y999_one() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1234567), 1233567);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &999);
    assert_eq!(vault.total_assets(), 1235566);
    assert_eq!(vault.withdraw(&user, &1), 1);
    assert_eq!(vault.shares_of(&user), 1233566);
    assert_eq!(vault.total_shares(), 1234566);
    assert_eq!(vault.total_assets(), 1235565);
    // Solvabilite relue depuis le contrat (l'oracle attend 1234564).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 1234564);
}

#[test]
fn m_d1234567_y10000_all() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1234567), 1233567);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &10000);
    assert_eq!(vault.total_assets(), 1244567);
    assert_eq!(vault.withdraw(&user, &1233567), 1243558);
    assert_eq!(vault.shares_of(&user), 0);
    assert_eq!(vault.total_shares(), 1000);
    assert_eq!(vault.total_assets(), 1009);
    // Solvabilite relue depuis le contrat (l'oracle attend 0).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 0);
}

#[test]
fn m_d1234567_y10000_half() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1234567), 1233567);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &10000);
    assert_eq!(vault.total_assets(), 1244567);
    assert_eq!(vault.withdraw(&user, &616783), 621778);
    assert_eq!(vault.shares_of(&user), 616784);
    assert_eq!(vault.total_shares(), 617784);
    assert_eq!(vault.total_assets(), 622789);
    // Solvabilite relue depuis le contrat (l'oracle attend 621780).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 621780);
}

#[test]
fn m_d1234567_y10000_third() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1234567), 1233567);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &10000);
    assert_eq!(vault.total_assets(), 1244567);
    assert_eq!(vault.withdraw(&user, &411189), 414519);
    assert_eq!(vault.shares_of(&user), 822378);
    assert_eq!(vault.total_shares(), 823378);
    assert_eq!(vault.total_assets(), 830048);
    // Solvabilite relue depuis le contrat (l'oracle attend 829039).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 829039);
}

#[test]
fn m_d1234567_y10000_twothirds() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1234567), 1233567);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &10000);
    assert_eq!(vault.total_assets(), 1244567);
    assert_eq!(vault.withdraw(&user, &822378), 829039);
    assert_eq!(vault.shares_of(&user), 411189);
    assert_eq!(vault.total_shares(), 412189);
    assert_eq!(vault.total_assets(), 415528);
    // Solvabilite relue depuis le contrat (l'oracle attend 414519).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 414519);
}

#[test]
fn m_d1234567_y10000_one() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1234567), 1233567);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &10000);
    assert_eq!(vault.total_assets(), 1244567);
    assert_eq!(vault.withdraw(&user, &1), 1);
    assert_eq!(vault.shares_of(&user), 1233566);
    assert_eq!(vault.total_shares(), 1234566);
    assert_eq!(vault.total_assets(), 1244566);
    // Solvabilite relue depuis le contrat (l'oracle attend 1243557).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 1243557);
}

#[test]
fn m_d1234567_y123457_all() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1234567), 1233567);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &123457);
    assert_eq!(vault.total_assets(), 1358024);
    assert_eq!(vault.withdraw(&user, &1233567), 1356923);
    assert_eq!(vault.shares_of(&user), 0);
    assert_eq!(vault.total_shares(), 1000);
    assert_eq!(vault.total_assets(), 1101);
    // Solvabilite relue depuis le contrat (l'oracle attend 0).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 0);
}

#[test]
fn m_d1234567_y123457_half() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1234567), 1233567);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &123457);
    assert_eq!(vault.total_assets(), 1358024);
    assert_eq!(vault.withdraw(&user, &616783), 678461);
    assert_eq!(vault.shares_of(&user), 616784);
    assert_eq!(vault.total_shares(), 617784);
    assert_eq!(vault.total_assets(), 679563);
    // Solvabilite relue depuis le contrat (l'oracle attend 678462).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 678462);
}

#[test]
fn m_d1234567_y123457_third() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1234567), 1233567);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &123457);
    assert_eq!(vault.total_assets(), 1358024);
    assert_eq!(vault.withdraw(&user, &411189), 452307);
    assert_eq!(vault.shares_of(&user), 822378);
    assert_eq!(vault.total_shares(), 823378);
    assert_eq!(vault.total_assets(), 905717);
    // Solvabilite relue depuis le contrat (l'oracle attend 904616).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 904616);
}

#[test]
fn m_d1234567_y123457_twothirds() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1234567), 1233567);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &123457);
    assert_eq!(vault.total_assets(), 1358024);
    assert_eq!(vault.withdraw(&user, &822378), 904615);
    assert_eq!(vault.shares_of(&user), 411189);
    assert_eq!(vault.total_shares(), 412189);
    assert_eq!(vault.total_assets(), 453409);
    // Solvabilite relue depuis le contrat (l'oracle attend 452308).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 452308);
}

#[test]
fn m_d1234567_y123457_one() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &1234567), 1233567);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &123457);
    assert_eq!(vault.total_assets(), 1358024);
    assert_eq!(vault.withdraw(&user, &1), 1);
    assert_eq!(vault.shares_of(&user), 1233566);
    assert_eq!(vault.total_shares(), 1234566);
    assert_eq!(vault.total_assets(), 1358023);
    // Solvabilite relue depuis le contrat (l'oracle attend 1356922).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 1356922);
}

#[test]
fn m_d999999937_y0_all() {
    let (_env, user, _token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &999999937), 999998937);
    // pas de rendement simule
    assert_eq!(vault.total_assets(), 999999937);
    assert_eq!(vault.withdraw(&user, &999998937), 999998937);
    assert_eq!(vault.shares_of(&user), 0);
    assert_eq!(vault.total_shares(), 1000);
    assert_eq!(vault.total_assets(), 1000);
    // Solvabilite relue depuis le contrat (l'oracle attend 0).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 0);
}

#[test]
fn m_d999999937_y0_half() {
    let (_env, user, _token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &999999937), 999998937);
    // pas de rendement simule
    assert_eq!(vault.total_assets(), 999999937);
    assert_eq!(vault.withdraw(&user, &499999468), 499999468);
    assert_eq!(vault.shares_of(&user), 499999469);
    assert_eq!(vault.total_shares(), 500000469);
    assert_eq!(vault.total_assets(), 500000469);
    // Solvabilite relue depuis le contrat (l'oracle attend 499999469).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 499999469);
}

#[test]
fn m_d999999937_y0_third() {
    let (_env, user, _token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &999999937), 999998937);
    // pas de rendement simule
    assert_eq!(vault.total_assets(), 999999937);
    assert_eq!(vault.withdraw(&user, &333332979), 333332979);
    assert_eq!(vault.shares_of(&user), 666665958);
    assert_eq!(vault.total_shares(), 666666958);
    assert_eq!(vault.total_assets(), 666666958);
    // Solvabilite relue depuis le contrat (l'oracle attend 666665958).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 666665958);
}

#[test]
fn m_d999999937_y0_twothirds() {
    let (_env, user, _token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &999999937), 999998937);
    // pas de rendement simule
    assert_eq!(vault.total_assets(), 999999937);
    assert_eq!(vault.withdraw(&user, &666665958), 666665958);
    assert_eq!(vault.shares_of(&user), 333332979);
    assert_eq!(vault.total_shares(), 333333979);
    assert_eq!(vault.total_assets(), 333333979);
    // Solvabilite relue depuis le contrat (l'oracle attend 333332979).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 333332979);
}

#[test]
fn m_d999999937_y0_one() {
    let (_env, user, _token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &999999937), 999998937);
    // pas de rendement simule
    assert_eq!(vault.total_assets(), 999999937);
    assert_eq!(vault.withdraw(&user, &1), 1);
    assert_eq!(vault.shares_of(&user), 999998936);
    assert_eq!(vault.total_shares(), 999999936);
    assert_eq!(vault.total_assets(), 999999936);
    // Solvabilite relue depuis le contrat (l'oracle attend 999998936).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 999998936);
}

#[test]
fn m_d999999937_y1_all() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &999999937), 999998937);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &1);
    assert_eq!(vault.total_assets(), 999999938);
    assert_eq!(vault.withdraw(&user, &999998937), 999998937);
    assert_eq!(vault.shares_of(&user), 0);
    assert_eq!(vault.total_shares(), 1000);
    assert_eq!(vault.total_assets(), 1001);
    // Solvabilite relue depuis le contrat (l'oracle attend 0).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 0);
}

#[test]
fn m_d999999937_y1_half() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &999999937), 999998937);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &1);
    assert_eq!(vault.total_assets(), 999999938);
    assert_eq!(vault.withdraw(&user, &499999468), 499999468);
    assert_eq!(vault.shares_of(&user), 499999469);
    assert_eq!(vault.total_shares(), 500000469);
    assert_eq!(vault.total_assets(), 500000470);
    // Solvabilite relue depuis le contrat (l'oracle attend 499999469).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 499999469);
}

#[test]
fn m_d999999937_y1_third() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &999999937), 999998937);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &1);
    assert_eq!(vault.total_assets(), 999999938);
    assert_eq!(vault.withdraw(&user, &333332979), 333332979);
    assert_eq!(vault.shares_of(&user), 666665958);
    assert_eq!(vault.total_shares(), 666666958);
    assert_eq!(vault.total_assets(), 666666959);
    // Solvabilite relue depuis le contrat (l'oracle attend 666665958).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 666665958);
}

#[test]
fn m_d999999937_y1_twothirds() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &999999937), 999998937);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &1);
    assert_eq!(vault.total_assets(), 999999938);
    assert_eq!(vault.withdraw(&user, &666665958), 666665958);
    assert_eq!(vault.shares_of(&user), 333332979);
    assert_eq!(vault.total_shares(), 333333979);
    assert_eq!(vault.total_assets(), 333333980);
    // Solvabilite relue depuis le contrat (l'oracle attend 333332979).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 333332979);
}

#[test]
fn m_d999999937_y1_one() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &999999937), 999998937);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &1);
    assert_eq!(vault.total_assets(), 999999938);
    assert_eq!(vault.withdraw(&user, &1), 1);
    assert_eq!(vault.shares_of(&user), 999998936);
    assert_eq!(vault.total_shares(), 999999936);
    assert_eq!(vault.total_assets(), 999999937);
    // Solvabilite relue depuis le contrat (l'oracle attend 999998936).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 999998936);
}

#[test]
fn m_d999999937_y999_all() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &999999937), 999998937);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &999);
    assert_eq!(vault.total_assets(), 1000000936);
    assert_eq!(vault.withdraw(&user, &999998937), 999999935);
    assert_eq!(vault.shares_of(&user), 0);
    assert_eq!(vault.total_shares(), 1000);
    assert_eq!(vault.total_assets(), 1001);
    // Solvabilite relue depuis le contrat (l'oracle attend 0).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 0);
}

#[test]
fn m_d999999937_y999_half() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &999999937), 999998937);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &999);
    assert_eq!(vault.total_assets(), 1000000936);
    assert_eq!(vault.withdraw(&user, &499999468), 499999967);
    assert_eq!(vault.shares_of(&user), 499999469);
    assert_eq!(vault.total_shares(), 500000469);
    assert_eq!(vault.total_assets(), 500000969);
    // Solvabilite relue depuis le contrat (l'oracle attend 499999968).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 499999968);
}

#[test]
fn m_d999999937_y999_third() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &999999937), 999998937);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &999);
    assert_eq!(vault.total_assets(), 1000000936);
    assert_eq!(vault.withdraw(&user, &333332979), 333333311);
    assert_eq!(vault.shares_of(&user), 666665958);
    assert_eq!(vault.total_shares(), 666666958);
    assert_eq!(vault.total_assets(), 666667625);
    // Solvabilite relue depuis le contrat (l'oracle attend 666666624).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 666666624);
}

#[test]
fn m_d999999937_y999_twothirds() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &999999937), 999998937);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &999);
    assert_eq!(vault.total_assets(), 1000000936);
    assert_eq!(vault.withdraw(&user, &666665958), 666666623);
    assert_eq!(vault.shares_of(&user), 333332979);
    assert_eq!(vault.total_shares(), 333333979);
    assert_eq!(vault.total_assets(), 333334313);
    // Solvabilite relue depuis le contrat (l'oracle attend 333333312).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 333333312);
}

#[test]
fn m_d999999937_y999_one() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &999999937), 999998937);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &999);
    assert_eq!(vault.total_assets(), 1000000936);
    assert_eq!(vault.withdraw(&user, &1), 1);
    assert_eq!(vault.shares_of(&user), 999998936);
    assert_eq!(vault.total_shares(), 999999936);
    assert_eq!(vault.total_assets(), 1000000935);
    // Solvabilite relue depuis le contrat (l'oracle attend 999999934).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 999999934);
}

#[test]
fn m_d999999937_y10000_all() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &999999937), 999998937);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &10000);
    assert_eq!(vault.total_assets(), 1000009937);
    assert_eq!(vault.withdraw(&user, &999998937), 1000008936);
    assert_eq!(vault.shares_of(&user), 0);
    assert_eq!(vault.total_shares(), 1000);
    assert_eq!(vault.total_assets(), 1001);
    // Solvabilite relue depuis le contrat (l'oracle attend 0).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 0);
}

#[test]
fn m_d999999937_y10000_half() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &999999937), 999998937);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &10000);
    assert_eq!(vault.total_assets(), 1000009937);
    assert_eq!(vault.withdraw(&user, &499999468), 500004467);
    assert_eq!(vault.shares_of(&user), 499999469);
    assert_eq!(vault.total_shares(), 500000469);
    assert_eq!(vault.total_assets(), 500005470);
    // Solvabilite relue depuis le contrat (l'oracle attend 500004469).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 500004469);
}

#[test]
fn m_d999999937_y10000_third() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &999999937), 999998937);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &10000);
    assert_eq!(vault.total_assets(), 1000009937);
    assert_eq!(vault.withdraw(&user, &333332979), 333336312);
    assert_eq!(vault.shares_of(&user), 666665958);
    assert_eq!(vault.total_shares(), 666666958);
    assert_eq!(vault.total_assets(), 666673625);
    // Solvabilite relue depuis le contrat (l'oracle attend 666672624).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 666672624);
}

#[test]
fn m_d999999937_y10000_twothirds() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &999999937), 999998937);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &10000);
    assert_eq!(vault.total_assets(), 1000009937);
    assert_eq!(vault.withdraw(&user, &666665958), 666672624);
    assert_eq!(vault.shares_of(&user), 333332979);
    assert_eq!(vault.total_shares(), 333333979);
    assert_eq!(vault.total_assets(), 333337313);
    // Solvabilite relue depuis le contrat (l'oracle attend 333336312).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 333336312);
}

#[test]
fn m_d999999937_y10000_one() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &999999937), 999998937);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &10000);
    assert_eq!(vault.total_assets(), 1000009937);
    assert_eq!(vault.withdraw(&user, &1), 1);
    assert_eq!(vault.shares_of(&user), 999998936);
    assert_eq!(vault.total_shares(), 999999936);
    assert_eq!(vault.total_assets(), 1000009936);
    // Solvabilite relue depuis le contrat (l'oracle attend 1000008935).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 1000008935);
}

#[test]
fn m_d999999937_y123457_all() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &999999937), 999998937);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &123457);
    assert_eq!(vault.total_assets(), 1000123394);
    assert_eq!(vault.withdraw(&user, &999998937), 1000122393);
    assert_eq!(vault.shares_of(&user), 0);
    assert_eq!(vault.total_shares(), 1000);
    assert_eq!(vault.total_assets(), 1001);
    // Solvabilite relue depuis le contrat (l'oracle attend 0).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 0);
}

#[test]
fn m_d999999937_y123457_half() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &999999937), 999998937);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &123457);
    assert_eq!(vault.total_assets(), 1000123394);
    assert_eq!(vault.withdraw(&user, &499999468), 500061196);
    assert_eq!(vault.shares_of(&user), 499999469);
    assert_eq!(vault.total_shares(), 500000469);
    assert_eq!(vault.total_assets(), 500062198);
    // Solvabilite relue depuis le contrat (l'oracle attend 500061197).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 500061197);
}

#[test]
fn m_d999999937_y123457_third() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &999999937), 999998937);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &123457);
    assert_eq!(vault.total_assets(), 1000123394);
    assert_eq!(vault.withdraw(&user, &333332979), 333374131);
    assert_eq!(vault.shares_of(&user), 666665958);
    assert_eq!(vault.total_shares(), 666666958);
    assert_eq!(vault.total_assets(), 666749263);
    // Solvabilite relue depuis le contrat (l'oracle attend 666748262).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 666748262);
}

#[test]
fn m_d999999937_y123457_twothirds() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &999999937), 999998937);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &123457);
    assert_eq!(vault.total_assets(), 1000123394);
    assert_eq!(vault.withdraw(&user, &666665958), 666748262);
    assert_eq!(vault.shares_of(&user), 333332979);
    assert_eq!(vault.total_shares(), 333333979);
    assert_eq!(vault.total_assets(), 333375132);
    // Solvabilite relue depuis le contrat (l'oracle attend 333374131).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 333374131);
}

#[test]
fn m_d999999937_y123457_one() {
    let (env, user, token, vault) = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &999999937), 999998937);
    StellarAssetClient::new(&env, &token.address).mint(&vault.address, &123457);
    assert_eq!(vault.total_assets(), 1000123394);
    assert_eq!(vault.withdraw(&user, &1), 1);
    assert_eq!(vault.shares_of(&user), 999998936);
    assert_eq!(vault.total_shares(), 999999936);
    assert_eq!(vault.total_assets(), 1000123393);
    // Solvabilite relue depuis le contrat (l'oracle attend 1000122392).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, 1000122392);
}

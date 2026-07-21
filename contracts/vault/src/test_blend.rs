#![cfg(test)]
// Montants ecrits en convention Stellar 7 decimales (X_XXXXXXX, ex. 0_1000000
// = 0,1) : le groupement d'underscores suit les decimales de l'actif, pas les
// milliers, comme dans blend-contract-sdk.
#![allow(clippy::inconsistent_digit_grouping, clippy::zero_prefixed_literal)]
//! Integration du vault avec un pool Blend v2 reel (stack deployee par
//! BlendFixture dans l'env de test : emitter, backstop, comet, pool factory).

use super::{YieldVault, YieldVaultClient};
use blend_contract_sdk::pool as blend_pool;
use blend_contract_sdk::testutils::{default_reserve_config, BlendFixture};
use soroban_sdk::{
    testutils::{Address as _, BytesN as _, Ledger},
    token::{StellarAssetClient, TokenClient},
    vec, Address, BytesN, Env, String,
};

/// Oracle SEP-40 minimal : prix constant 1 $ (7 decimales) pour tout actif.
/// Necessaire des qu'un emprunt existe (controle de sante du pool).
mod mock_oracle {
    use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, Symbol};

    #[contracttype]
    pub enum Asset {
        Stellar(Address),
        Other(Symbol),
    }

    #[contracttype]
    pub struct PriceData {
        pub price: i128,
        pub timestamp: u64,
    }

    #[contract]
    pub struct MockOracle;

    #[contractimpl]
    impl MockOracle {
        pub fn decimals(_env: Env) -> u32 {
            7
        }

        pub fn lastprice(env: Env, _asset: Asset) -> Option<PriceData> {
            Some(PriceData {
                price: 1_0000000,
                timestamp: env.ledger().timestamp(),
            })
        }
    }
}

struct BlendBench<'a> {
    env: Env,
    user: Address,
    usdc: TokenClient<'a>,
    pool: blend_pool::Client<'a>,
    vault: YieldVaultClient<'a>,
}

fn setup_blend<'a>(initial_mint: i128) -> BlendBench<'a> {
    setup_blend_config(initial_mint, default_reserve_config())
}

fn setup_blend_config<'a>(
    initial_mint: i128,
    reserve_config: blend_pool::ReserveConfig,
) -> BlendBench<'a> {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    let blnd = env
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let usdc_sac = env.register_stellar_asset_contract_v2(admin.clone());
    let usdc = usdc_sac.address();

    let blend = BlendFixture::deploy(&env, &admin, &blnd, &usdc);
    // La fixture remet le budget par defaut ; nos scenarios multi-submit
    // depassent le budget CPU de test, on le leve.
    env.cost_estimate().budget().reset_unlimited();

    let oracle = env.register(mock_oracle::MockOracle, ());
    let pool_id = blend.pool_factory.deploy(
        &admin,
        &String::from_str(&env, "foryield-test"),
        &BytesN::<32>::random(&env),
        &oracle,    // consulte seulement si un emprunt existe (test d'accrual)
        &0_1000000, // take rate 10 %
        &4,
        &1_0000000,
    );
    let pool = blend_pool::Client::new(&env, &pool_id);
    pool.queue_set_reserve(&usdc, &reserve_config);
    pool.set_reserve(&usdc);

    blend.backstop.deposit(&admin, &pool_id, &50_000_0000000);
    pool.set_status(&3); // sortie du statut setup
    pool.update_status(); // -> actif (backstop au-dessus du seuil)

    let usdc_token = TokenClient::new(&env, &usdc);
    StellarAssetClient::new(&env, &usdc).mint(&user, &initial_mint);

    let vault_id = env.register(YieldVault, ());
    let vault = YieldVaultClient::new(&env, &vault_id);
    vault.initialize(&admin, &usdc, &Some(pool_id));

    BlendBench {
        env,
        user,
        usdc: usdc_token,
        pool,
        vault,
    }
}

/// bTokens detenus par le vault dans le pool sur la reserve USDC.
fn vault_b_tokens(b: &BlendBench) -> i128 {
    let reserve = b.pool.get_reserve(&b.usdc.address);
    let positions = b.pool.get_positions(&b.vault.address);
    positions.supply.get(reserve.config.index).unwrap_or(0)
}

#[test]
fn deposit_supplies_to_blend_pool() {
    let b = setup_blend(100_000_0000000);

    let shares = b.vault.deposit(&b.user, &10_000_0000000);

    // Parts inchangees par l'allocation (b_rate initial = 1).
    assert_eq!(shares, 10_000_0000000 - 1_000);
    // L'actif ne dort pas sur le vault : tout est fourni au pool Blend.
    assert_eq!(b.usdc.balance(&b.vault.address), 0);
    assert!(vault_b_tokens(&b) > 0);
    // La position Blend est valorisee dans total_assets (b_rate = 1 a t0).
    assert_eq!(b.vault.total_assets(), 10_000_0000000);
}

#[test]
fn withdraw_pulls_back_from_blend_pool() {
    let b = setup_blend(100_000_0000000);
    b.vault.deposit(&b.user, &10_000_0000000);
    let user_before = b.usdc.balance(&b.user);

    let amount = b.vault.withdraw(&b.user, &4_000_0000000);

    assert_eq!(amount, 4_000_0000000);
    assert_eq!(b.usdc.balance(&b.user), user_before + 4_000_0000000);
    // Le retrait est servi depuis le pool, le vault ne garde rien d'inactif.
    assert_eq!(b.usdc.balance(&b.vault.address), 0);
    assert_eq!(b.vault.total_assets(), 6_000_0000000);
}

#[test]
fn deposit_beyond_pool_supply_cap_reverts_atomically() {
    let mut config = default_reserve_config();
    config.supply_cap = 5_000_0000000;
    let b = setup_blend_config(100_000_0000000, config);

    let result = b.vault.try_deposit(&b.user, &10_000_0000000);

    // Le refus du pool (ExceededSupplyCap) fait echouer TOUTE la transaction :
    // aucune part emise, aucun token deplace (atomicite Soroban).
    assert!(result.is_err());
    assert_eq!(b.vault.total_shares(), 0);
    assert_eq!(b.usdc.balance(&b.user), 100_000_0000000);
    assert_eq!(b.vault.total_assets(), 0);
}

#[test]
fn frozen_pool_blocks_deposits_but_not_exits() {
    let b = setup_blend(100_000_0000000);
    b.vault.deposit(&b.user, &10_000_0000000);

    b.pool.set_status(&4); // Admin Frozen : Blend refuse les nouveaux supplies

    assert!(b.vault.try_deposit(&b.user, &1_000_0000000).is_err());
    assert_eq!(b.vault.total_shares(), 10_000_0000000); // rien n'a bouge

    // La sortie reste possible : Blend gele les entrees, pas les retraits.
    let amount = b.vault.withdraw(&b.user, &4_000_0000000);
    assert_eq!(amount, 4_000_0000000);
}

#[test]
fn donation_before_first_deposit_with_pool_absorbed_into_genesis() {
    let b = setup_blend(100_000_0000000);
    // Donation avant toute part : elle reste oisive sur le vault.
    StellarAssetClient::new(&b.env, &b.usdc.address).mint(&b.vault.address, &5_000_0000000);

    let shares = b.vault.deposit(&b.user, &10_000_0000000);

    // La genese absorbe l'oisif + le depot ; seul le depot part au pool.
    assert_eq!(shares, 15_000_0000000 - 1_000);
    assert_eq!(b.vault.total_shares(), 15_000_0000000);
    assert_eq!(b.usdc.balance(&b.vault.address), 5_000_0000000);
    assert_eq!(b.vault.total_assets(), 15_000_0000000);
}

#[test]
fn withdraw_blocked_by_pool_utilization_reverts_atomically() {
    let b = setup_blend(100_000_0000000);
    b.vault.deposit(&b.user, &10_000_0000000);

    // Second actif de collateral, ajoute comme reserve au pool actif
    // (saut de ledger pour purger le timelock de queue_set_reserve).
    let collat = b
        .env
        .register_stellar_asset_contract_v2(Address::generate(&b.env))
        .address();
    b.pool.queue_set_reserve(&collat, &default_reserve_config());
    b.env.ledger().with_mut(|li| li.timestamp += 8 * 24 * 3600);
    b.pool.set_reserve(&collat);

    // Emprunteur : collateral dans le second actif, emprunt de 90 % de la
    // reserve USDC (fournie uniquement par le vault).
    let borrower = Address::generate(&b.env);
    StellarAssetClient::new(&b.env, &collat).mint(&borrower, &40_000_0000000);
    b.pool.submit(
        &borrower,
        &borrower,
        &borrower,
        &vec![
            &b.env,
            blend_pool::Request {
                address: collat.clone(),
                amount: 40_000_0000000,
                request_type: 2, // SupplyCollateral
            },
            blend_pool::Request {
                address: b.usdc.address.clone(),
                amount: 9_000_0000000,
                request_type: 4, // Borrow
            },
        ],
    );

    // Retirer 8k des 10k fournis porterait l'utilisation au-dela du max :
    // le pool refuse, TOUT le retrait echoue, aucune part n'est brulee.
    let shares_before = b.vault.shares_of(&b.user);
    let balance_before = b.usdc.balance(&b.user);

    assert!(b.vault.try_withdraw(&b.user, &8_000_0000000).is_err());
    assert_eq!(b.vault.shares_of(&b.user), shares_before);
    assert_eq!(b.usdc.balance(&b.user), balance_before);

    // Un retrait plus modeste, compatible avec l'utilisation max, passe.
    let amount = b.vault.withdraw(&b.user, &100_0000000);
    assert!(amount >= 100_0000000); // >= : l'interet a pu commencer a courir
}

#[test]
fn blend_interest_accrues_into_total_assets_and_share_price() {
    let b = setup_blend(100_000_0000000);
    b.vault.deposit(&b.user, &10_000_0000000);

    // Emprunteur reel sur la reserve USDC : collateral 30k (c_factor 0,75
    // => capacite 22,5k), emprunt 10k. L'utilisation genere de l'interet.
    let borrower = Address::generate(&b.env);
    StellarAssetClient::new(&b.env, &b.usdc.address).mint(&borrower, &50_000_0000000);
    b.pool.submit(
        &borrower,
        &borrower,
        &borrower,
        &vec![
            &b.env,
            blend_pool::Request {
                address: b.usdc.address.clone(),
                amount: 30_000_0000000,
                request_type: 2, // SupplyCollateral
            },
            blend_pool::Request {
                address: b.usdc.address.clone(),
                amount: 10_000_0000000,
                request_type: 4, // Borrow
            },
        ],
    );

    // Un an plus tard : un remboursement minime declenche l'accrual on-chain.
    b.env
        .ledger()
        .with_mut(|li| li.timestamp += 365 * 24 * 3600);
    b.pool.submit(
        &borrower,
        &borrower,
        &borrower,
        &vec![
            &b.env,
            blend_pool::Request {
                address: b.usdc.address.clone(),
                amount: 1_0000000,
                request_type: 5, // Repay
            },
        ],
    );

    // La position du vault vaut plus que le principal : l'interet est dans
    // total_assets via b_rate (le vault n'a rien fait).
    let assets = b.vault.total_assets();
    assert!(
        assets > 10_000_0000000,
        "expected accrued interest, got {assets}"
    );

    // Un nouveau deposant paie le prix de part accru : strictement moins
    // d'une part par unite deposee.
    let user2 = Address::generate(&b.env);
    StellarAssetClient::new(&b.env, &b.usdc.address).mint(&user2, &10_000_0000000);
    let shares = b.vault.deposit(&user2, &10_000_0000000);
    assert!(shares > 0);
    assert!(
        shares < 10_000_0000000 - 1_000,
        "share price should exceed 1, got {shares} shares"
    );
}

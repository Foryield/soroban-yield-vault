#![cfg(test)]
// Montants ecrits en convention Stellar 7 decimales (X_XXXXXXX, ex. 0_1000000
// = 0,1) : le groupement d'underscores suit les decimales de l'actif, pas les
// milliers, comme dans blend-contract-sdk.
#![allow(clippy::inconsistent_digit_grouping, clippy::zero_prefixed_literal)]
//! Strategie Blend de DeFindex (wasm vendorise, test_wasms/README.md) sur un
//! pool Blend v2 reel (stack deployee par BlendFixture : emitter, backstop,
//! comet, pool factory).

use super::{StrategyConfig, VaultError, YieldVault, YieldVaultClient};
use blend_contract_sdk::pool as blend_pool;
use blend_contract_sdk::testutils::{default_reserve_config, BlendFixture};
use soroban_sdk::{
    testutils::{Address as _, BytesN as _, Ledger},
    token::{StellarAssetClient, TokenClient},
    vec, Address, Bytes, BytesN, Env, IntoVal, String, Val, Vec,
};

mod blend_strategy {
    soroban_sdk::contractimport!(file = "test_wasms/blend_strategy.wasm");
}

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

/// Seuil de reinvestissement des BLND passe au constructeur de la strategie.
const REWARD_THRESHOLD: i128 = 1_0000000;

/// Parts mortes de la strategie : son premier deposant cede 1000 unites de
/// parts, verrouillees contre l'inflation du prix de la premiere part (meme
/// modele que MINIMUM_LIQUIDITY du vault, `strategies/blend/src/reserves.rs`).
const STRATEGY_DEAD_SHARES: i128 = 1000;

pub(crate) struct BlendBench<'a> {
    pub env: Env,
    pub admin: Address,
    pub user: Address,
    pub blnd: Address,
    pub usdc: TokenClient<'a>,
    pub pool: blend_pool::Client<'a>,
    pub strategy: Address,
}

pub(crate) fn setup_blend<'a>(initial_mint: i128) -> BlendBench<'a> {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    let blnd = env
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let usdc = env
        .register_stellar_asset_contract_v2(admin.clone())
        .address();

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
    pool.queue_set_reserve(&usdc, &default_reserve_config());
    pool.set_reserve(&usdc);

    blend.backstop.deposit(&admin, &pool_id, &50_000_0000000);
    pool.set_status(&3); // sortie du statut setup
    pool.update_status(); // -> actif (backstop au-dessus du seuil)

    StellarAssetClient::new(&env, &usdc).mint(&user, &initial_mint);

    // Routeur de reinvestissement : une adresse sans `router_pair_for`, ce qui
    // rend le chemin Soroswap de `harvest` inatteignable (plan D5, section 4).
    let no_router = Address::generate(&env);
    let init_args: Vec<Val> = vec![
        &env,
        pool_id.into_val(&env),
        blnd.into_val(&env),
        no_router.into_val(&env),
        REWARD_THRESHOLD.into_val(&env),
        admin.into_val(&env),
    ];
    let strategy = env.register(blend_strategy::WASM, (usdc.clone(), init_args));

    BlendBench {
        usdc: TokenClient::new(&env, &usdc),
        env,
        admin,
        user,
        blnd,
        pool,
        strategy,
    }
}

fn strategy_client<'a>(b: &BlendBench<'a>) -> blend_strategy::Client<'a> {
    blend_strategy::Client::new(&b.env, &b.strategy)
}

/// Emprunteur reel sur la reserve USDC : collateral 30k (c_factor 0,75 =>
/// capacite 22,5k), emprunt 10k, puis un an d'interet constate on-chain par
/// un remboursement minime.
fn accrue_one_year_of_interest(b: &BlendBench) {
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
}

/// Empreinte du wasm vendorise, confrontee a SHA256SUMS : un binaire altere
/// apres son epinglage fait echouer la suite.
#[test]
fn vendored_wasms_match_sha256sums() {
    let manifest = include_str!("../test_wasms/SHA256SUMS");
    let (hex, name) = manifest
        .trim()
        .split_once("  ")
        .expect("ligne SHA256SUMS invalide (attendu : empreinte, deux espaces, nom)");
    assert_eq!(name, "blend_strategy.wasm");
    let env = Env::default();
    let digest = env
        .crypto()
        .sha256(&Bytes::from_slice(
            &env,
            include_bytes!("../test_wasms/blend_strategy.wasm"),
        ))
        .to_array();
    let mut expected = [0_u8; 32];
    for (i, byte) in expected.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).expect("hex invalide");
    }
    assert_eq!(digest, expected, "empreinte divergente pour {name}");
}

#[test]
fn strategy_reports_the_pool_asset() {
    let b = setup_blend(1_000_0000000);
    assert_eq!(strategy_client(&b).asset(), b.usdc.address);
}

#[test]
fn deposit_supplies_the_pool_and_balance_reads_it_back() {
    let b = setup_blend(10_000_0000000);
    let s = strategy_client(&b);

    let reported = s.deposit(&1_000_0000000, &b.user);

    // b_rate initial = 1 : la valeur rendue est le montant depose, moins les
    // parts mortes cedees par le premier deposant.
    assert_eq!(reported, 1_000_0000000 - STRATEGY_DEAD_SHARES);
    assert_eq!(s.balance(&b.user), 1_000_0000000 - STRATEGY_DEAD_SHARES);
    assert_eq!(
        b.usdc.balance(&b.strategy),
        0,
        "rien ne dort sur la strategie"
    );
}

#[test]
fn withdrawing_the_whole_balance_returns_it_to_the_unit() {
    let b = setup_blend(10_000_0000000);
    let s = strategy_client(&b);
    s.deposit(&1_000_0000000, &b.user);
    let before = b.usdc.balance(&b.user);

    let balance = s.balance(&b.user);
    s.withdraw(&balance, &b.user, &b.user);

    assert_eq!(b.usdc.balance(&b.user) - before, balance);
    assert_eq!(s.balance(&b.user), 0);
}

#[test]
fn interest_accrues_into_the_strategy_balance() {
    let b = setup_blend(10_000_0000000);
    let s = strategy_client(&b);
    s.deposit(&10_000_0000000, &b.user);

    accrue_one_year_of_interest(&b);

    let balance = s.balance(&b.user);
    assert!(
        balance > 10_000_0000000,
        "interet attendu, obtenu {balance}"
    );
}

#[test]
fn harvest_below_the_reward_threshold_never_swaps() {
    let b = setup_blend(10_000_0000000);
    let s = strategy_client(&b);
    s.deposit(&1_000_0000000, &b.user);
    // Aucune emission reclamable ici : harvest reclame zero BLND, reste sous
    // le seuil et s'arrete avant le chemin de swap.
    s.harvest(&b.admin, &None);
    assert_eq!(s.balance(&b.user), 1_000_0000000 - STRATEGY_DEAD_SHARES);
}

#[test]
fn harvest_above_the_reward_threshold_fails_before_any_transfer() {
    let b = setup_blend(10_000_0000000);
    let s = strategy_client(&b);
    s.deposit(&1_000_0000000, &b.user);
    // BLND au-dessus du seuil sur la strategie : harvest atteint le chemin de
    // swap, qui demande `router_pair_for` a une adresse qui ne l'expose pas.
    StellarAssetClient::new(&b.env, &b.blnd).mint(&b.strategy, &REWARD_THRESHOLD);

    assert!(s.try_harvest(&b.admin, &None).is_err());
    // Revert entier : les BLND n'ont pas bouge, la position non plus.
    assert_eq!(
        TokenClient::new(&b.env, &b.blnd).balance(&b.strategy),
        REWARD_THRESHOLD
    );
    assert_eq!(s.balance(&b.user), 1_000_0000000 - STRATEGY_DEAD_SHARES);
}

#[test]
fn withdrawals_after_interest_deliver_exactly_the_amount_asked() {
    // b_rate s'ecarte de 1 apres un an d'interet : la strategie convertit le
    // montant demande en bTokens et arrondit en sa faveur. Mesure faite le
    // 20/10/2026 sur des montants non ronds : aucun ecart, d'ou le controle
    // strict du vault (StrategyShortfall), sans tolerance de poussiere. Si une
    // version de la strategie rendait une unite de moins, ce test le dirait
    // avant qu'un retrait reel ne revert.
    let b = setup_blend(100_000_0000000);
    let s = strategy_client(&b);
    s.deposit(&10_000_0000000, &b.user);
    accrue_one_year_of_interest(&b);

    for ask in [1_0000000_i128, 3_3333333, 777_7777777, 1_234_5678901, 17] {
        let before = b.usdc.balance(&b.user);
        s.withdraw(&ask, &b.user, &b.user);
        assert_eq!(b.usdc.balance(&b.user) - before, ask, "ecart pour {ask}");
    }

    // Sortie complete : tout le solde annonce est rendu.
    let rest = s.balance(&b.user);
    let before = b.usdc.balance(&b.user);
    s.withdraw(&rest, &b.user, &b.user);
    assert_eq!(b.usdc.balance(&b.user) - before, rest);
    assert_eq!(s.balance(&b.user), 0);
}

// --- Le vault sur la strategie Blend : scenarios D1 portes au travers du
// reequilibrage. Cible 100 % : tout l'actif part au pool, comme en D1. ---

fn vault_on_blend<'a>(b: &BlendBench<'a>) -> YieldVaultClient<'a> {
    let list = vec![
        &b.env,
        StrategyConfig {
            address: b.strategy.clone(),
            target_bps: 10_000,
            cap_bps: 10_000,
            active: true,
        },
    ];
    let id = b.env.register(
        YieldVault,
        (
            b.admin.clone(),
            b.admin.clone(),
            b.usdc.address.clone(),
            list,
            500u32,
        ),
    );
    YieldVaultClient::new(&b.env, &id)
}

#[test]
fn deposits_reach_the_blend_pool_through_a_rebalance() {
    let b = setup_blend(100_000_0000000);
    let vault = vault_on_blend(&b);
    let s = strategy_client(&b);

    let shares = vault.deposit(&b.user, &10_000_0000000);
    assert_eq!(shares, 10_000_0000000 - 1_000);
    // Oisif jusqu'au lot.
    assert_eq!(b.usdc.balance(&vault.address), 10_000_0000000);

    vault.rebalance(&b.admin);

    assert_eq!(b.usdc.balance(&vault.address), 0);
    // Premier deposant de la strategie : ses parts mortes sont la seule perte.
    assert_eq!(
        s.balance(&vault.address),
        10_000_0000000 - STRATEGY_DEAD_SHARES
    );
    assert_eq!(vault.total_assets(), 10_000_0000000 - STRATEGY_DEAD_SHARES);
}

#[test]
fn withdrawals_pull_back_from_the_blend_pool() {
    let b = setup_blend(100_000_0000000);
    let vault = vault_on_blend(&b);
    vault.deposit(&b.user, &10_000_0000000);
    vault.rebalance(&b.admin);
    let before = b.usdc.balance(&b.user);

    let amount = vault.withdraw(&b.user, &4_000_0000000);

    assert_eq!(b.usdc.balance(&b.user) - before, amount);
    assert_eq!(b.usdc.balance(&vault.address), 0);

    // Sortie complete du porteur : tout ce que ses parts valent est rendu,
    // sans ecart d'arrondi qui ferait revert le retrait.
    let rest = vault.shares_of(&b.user);
    vault.withdraw(&b.user, &rest);
    assert_eq!(vault.shares_of(&b.user), 0);
}

#[test]
fn blend_interest_raises_the_vault_share_price() {
    let b = setup_blend(100_000_0000000);
    let vault = vault_on_blend(&b);
    vault.deposit(&b.user, &10_000_0000000);
    vault.rebalance(&b.admin);

    accrue_one_year_of_interest(&b);

    let assets = vault.total_assets();
    assert!(assets > 10_000_0000000, "interet attendu, obtenu {assets}");
    // Un nouveau deposant paie le prix de part accru.
    let user2 = Address::generate(&b.env);
    StellarAssetClient::new(&b.env, &b.usdc.address).mint(&user2, &10_000_0000000);
    let shares = vault.deposit(&user2, &10_000_0000000);
    assert!(shares < 10_000_0000000 - 1_000, "prix de part > 1 attendu");
}

#[test]
fn a_frozen_pool_blocks_the_rebalance_but_not_the_exits() {
    let b = setup_blend(100_000_0000000);
    let vault = vault_on_blend(&b);
    vault.deposit(&b.user, &10_000_0000000);
    vault.rebalance(&b.admin);

    b.pool.set_status(&4); // Admin Frozen : Blend refuse les nouveaux supplies

    // Le depot passe (il reste oisif), le lot qui voudrait l'investir non.
    vault.deposit(&b.user, &5_000_0000000);
    assert!(vault.try_rebalance(&b.admin).is_err());
    assert_eq!(b.usdc.balance(&vault.address), 5_000_0000000);

    // La sortie reste possible : Blend gele les entrees, pas les retraits.
    let before = b.usdc.balance(&b.user);
    vault.withdraw(&b.user, &8_000_0000000);
    assert!(b.usdc.balance(&b.user) > before);
}

#[test]
fn a_withdrawal_beyond_pool_liquidity_reverts_without_burning_shares() {
    let b = setup_blend(100_000_0000000);
    let vault = vault_on_blend(&b);
    vault.deposit(&b.user, &10_000_0000000);
    vault.rebalance(&b.admin);

    // Second actif de collateral, ajoute comme reserve au pool actif (saut de
    // ledger pour purger le timelock de queue_set_reserve), puis emprunt de
    // 90 % de la reserve USDC, fournie par le seul vault.
    let collat = b
        .env
        .register_stellar_asset_contract_v2(Address::generate(&b.env))
        .address();
    b.pool.queue_set_reserve(&collat, &default_reserve_config());
    b.env.ledger().with_mut(|li| li.timestamp += 8 * 24 * 3600);
    b.pool.set_reserve(&collat);
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

    let shares_before = vault.shares_of(&b.user);
    let balance_before = b.usdc.balance(&b.user);
    assert!(vault.try_withdraw(&b.user, &8_000_0000000).is_err());
    assert_eq!(vault.shares_of(&b.user), shares_before);
    assert_eq!(b.usdc.balance(&b.user), balance_before);

    // Un retrait compatible avec l'utilisation maximale passe. Le prix de la
    // part frole 1 : les parts mortes de la strategie (1000 unites sur 10k)
    // le tirent a peine sous 1, l'interet a peine au-dessus.
    let before = b.usdc.balance(&b.user);
    let amount = vault.withdraw(&b.user, &100_0000000);
    assert_eq!(b.usdc.balance(&b.user) - before, amount);
    assert!((amount - 100_0000000).abs() <= 100, "obtenu {amount}");
}

#[test]
fn a_rebalance_beyond_the_pool_supply_cap_reverts_atomically() {
    let b = setup_blend(100_000_0000000);
    let vault = vault_on_blend(&b);
    // Le pool plafonne l'offre a 5k, sous les 10k que le lot voudrait verser.
    let mut config = default_reserve_config();
    config.supply_cap = 5_000_0000000;
    b.pool.queue_set_reserve(&b.usdc.address, &config);
    b.env.ledger().with_mut(|li| li.timestamp += 8 * 24 * 3600);
    b.pool.set_reserve(&b.usdc.address);

    vault.deposit(&b.user, &10_000_0000000);
    assert!(vault.try_rebalance(&b.admin).is_err());
    assert_eq!(b.usdc.balance(&vault.address), 10_000_0000000);
    assert_eq!(strategy_client(&b).balance(&vault.address), 0);
}

#[test]
fn emergency_exit_brings_the_whole_blend_position_home() {
    let b = setup_blend(100_000_0000000);
    let vault = vault_on_blend(&b);
    vault.deposit(&b.user, &10_000_0000000);
    vault.rebalance(&b.admin);
    let invested = strategy_client(&b).balance(&vault.address);

    let recovered = vault.emergency_exit(&b.strategy, &false);

    assert_eq!(recovered, invested);
    assert_eq!(strategy_client(&b).balance(&vault.address), 0);
    assert_eq!(vault.total_assets(), invested);
    let err = vault.try_rebalance(&b.admin).unwrap_err().unwrap();
    assert_eq!(err, VaultError::NothingToRebalance.into());
}

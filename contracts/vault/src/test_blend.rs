#![cfg(test)]
// Montants ecrits en convention Stellar 7 decimales (X_XXXXXXX, ex. 0_1000000
// = 0,1) : le groupement d'underscores suit les decimales de l'actif, pas les
// milliers, comme dans blend-contract-sdk.
#![allow(clippy::inconsistent_digit_grouping, clippy::zero_prefixed_literal)]
//! Strategie Blend de DeFindex (wasm vendorise, test_wasms/README.md) sur un
//! pool Blend v2 reel (stack deployee par BlendFixture : emitter, backstop,
//! comet, pool factory).

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

#![cfg(test)]
// Montants en convention Stellar 7 decimales (X_XXXXXXX, ex. 0_1000000 = 0,1).
#![allow(clippy::inconsistent_digit_grouping, clippy::zero_prefixed_literal)]
// L'arite des clients generes par contractimport! est dictee par les ABI
// Aquarius (8-9 arguments), meme justification que dans le routeur.
#![allow(clippy::too_many_arguments)]
//! Strategie Aquarius sur la pile Aquarius reelle, depuis les wasm vendorises
//! du routeur (contracts/router/test_wasms, empreintes verifiees par la suite
//! du routeur). Pool a produit constant 30 pb, seme au prix de reference.
//!
//! Les operations de la strategie tournent SANS mock_all_auths : seule la
//! signature de l'utilisateur est fournie, la strategie doit pre-autoriser
//! elle-meme, exactement, ce que le pool et le router lui demandent.

extern crate std;

use super::{AquaStrategyError, AquariusLpStrategy, AquariusLpStrategyClient, DEAD_SHARES};
use soroban_sdk::{
    contract, contractimpl,
    testutils::{Address as _, EnvTestConfig, Ledger, MockAuth, MockAuthInvoke},
    token::{StellarAssetClient, TokenClient},
    vec, Address, BytesN, Env, IntoVal, InvokeError, Val, Vec,
};

mod aqua_router_wasm {
    soroban_sdk::contractimport!(
        file = "../router/test_wasms/soroban_liquidity_pool_router_contract.wasm"
    );
}
pub(crate) mod aqua_pool_wasm {
    soroban_sdk::contractimport!(
        file = "../router/test_wasms/soroban_liquidity_pool_contract.wasm"
    );
}
mod aqua_plane_wasm {
    soroban_sdk::contractimport!(
        file = "../router/test_wasms/soroban_liquidity_pool_plane_contract.wasm"
    );
}
mod aqua_calculator_wasm {
    soroban_sdk::contractimport!(
        file = "../router/test_wasms/soroban_liquidity_pool_liquidity_calculator_contract.wasm"
    );
}
mod aqua_token_wasm {
    soroban_sdk::contractimport!(file = "../router/test_wasms/soroban_token_contract.wasm");
}

/// Feed de boost factice exige par la chaine d'init du router Aqua (meme
/// fixture que le routeur, test_stack_common.rs) : total_supply = 0 rend le
/// boost neutre.
#[contract]
struct MockBoostFeed;

#[contractimpl]
impl MockBoostFeed {
    pub fn total_supply(_env: Env) -> u128 {
        0
    }
}

/// Prix de reference : 1,1221 USDC par EURC (cours du 20/10), echelle 1e9.
pub(crate) const PRICE: i128 = 1_122_100_000;
/// Reserves du pool au prix de reference : 1000 EURC, 1122,1 USDC.
pub(crate) const RESERVE_EURC: i128 = 1_000_0000000;
const RESERVE_USDC: i128 = 1_122_1000000;
const LEDGER_TIME: u64 = 1_700_000_000;
const MAX_AGE: u64 = 26 * 3_600;

pub(crate) struct Bench<'a> {
    pub env: Env,
    pub admin: Address,
    pub keeper: Address,
    pub user: Address,
    pub usdc: TokenClient<'a>,
    pub eurc: TokenClient<'a>,
    pub router: aqua_router_wasm::Client<'a>,
    pub tokens: Vec<Address>,
    pub pool_hash: BytesN<32>,
    pub strategy: AquariusLpStrategyClient<'a>,
}

fn sorted(env: &Env, a: &Address, b: &Address) -> Vec<Address> {
    if a < b {
        vec![env, a.clone(), b.clone()]
    } else {
        vec![env, b.clone(), a.clone()]
    }
}

/// Pile Aquarius complete, pool seme au prix de reference, strategie
/// deployee avec `price` comme prix initial.
pub(crate) fn setup_with_price<'a>(price: i128) -> Bench<'a> {
    setup_with(price, RESERVE_EURC, RESERVE_USDC)
}

/// Idem, reserves choisies.
fn setup_with<'a>(price: i128, reserve_eurc: i128, reserve_usdc: i128) -> Bench<'a> {
    setup_in(Env::default(), price, reserve_eurc, reserve_usdc)
}

/// Sans snapshot : un test de propriete en ecrirait un par cas, avec des
/// valeurs aleatoires (meme motif que test_props.rs du vault).
pub(crate) fn setup_quiet<'a>() -> Bench<'a> {
    let env = Env::new_with_config(EnvTestConfig {
        capture_snapshot_at_drop: false,
    });
    setup_in(env, PRICE, RESERVE_EURC, RESERVE_USDC)
}

fn setup_in<'a>(env: Env, price: i128, reserve_eurc: i128, reserve_usdc: i128) -> Bench<'a> {
    env.mock_all_auths();
    env.ledger().with_mut(|li| li.timestamp = LEDGER_TIME);
    env.cost_estimate().budget().reset_unlimited();

    let admin = Address::generate(&env);
    let keeper = Address::generate(&env);
    let user = Address::generate(&env);
    let usdc = env
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let eurc = env
        .register_stellar_asset_contract_v2(admin.clone())
        .address();

    // Chaine d'init Aqua, reprise de la fixture du routeur.
    let pool_wasm = env.deployer().upload_contract_wasm(aqua_pool_wasm::WASM);
    let token_wasm = env.deployer().upload_contract_wasm(aqua_token_wasm::WASM);
    let router = aqua_router_wasm::Client::new(&env, &env.register(aqua_router_wasm::WASM, ()));
    router.init_admin(&admin);
    router.set_pool_hash(&admin, &pool_wasm);
    router.set_token_hash(&admin, &token_wasm);
    let reward_token = env
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let boost_token = env
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let boost_feed = env.register(MockBoostFeed, ());
    router.set_reward_token(&admin, &reward_token);
    router.set_reward_boost_config(&admin, &boost_token, &boost_feed);
    router.configure_init_pool_payment(&admin, &reward_token, &0, &0, &admin);
    let plane = env.register(aqua_plane_wasm::WASM, ());
    router.set_pools_plane(&admin, &plane);
    let calculator =
        aqua_calculator_wasm::Client::new(&env, &env.register(aqua_calculator_wasm::WASM, ()));
    calculator.init_admin(&admin);
    calculator.set_pools_plane(&admin, &plane);
    router.set_liquidity_calculator(&admin, &calculator.address);

    let tokens = sorted(&env, &usdc, &eurc);
    let (pool_hash, _pool) = router.init_standard_pool(&admin, &tokens, &30);

    // Semis au prix de reference, par un tiers (le pool n'est pas a nous).
    let lp = Address::generate(&env);
    StellarAssetClient::new(&env, &usdc).mint(&lp, &reserve_usdc);
    StellarAssetClient::new(&env, &eurc).mint(&lp, &reserve_eurc);
    let desired = if tokens.get(0).unwrap() == usdc {
        vec![&env, reserve_usdc as u128, reserve_eurc as u128]
    } else {
        vec![&env, reserve_eurc as u128, reserve_usdc as u128]
    };
    router.deposit(&lp, &tokens, &pool_hash, &desired, &0);

    StellarAssetClient::new(&env, &usdc).mint(&user, &(reserve_usdc / 100).max(1_000_0000000));

    let init_args: Vec<Val> = vec![
        &env,
        admin.into_val(&env),
        keeper.into_val(&env),
        router.address.into_val(&env),
        pool_hash.into_val(&env),
        eurc.into_val(&env),
        price.into_val(&env),
        MAX_AGE.into_val(&env),
        100u32.into_val(&env),
        100u32.into_val(&env),
    ];
    let strategy = AquariusLpStrategyClient::new(
        &env,
        &env.register(AquariusLpStrategy, (usdc.clone(), init_args)),
    );

    Bench {
        usdc: TokenClient::new(&env, &usdc),
        eurc: TokenClient::new(&env, &eurc),
        env,
        admin,
        keeper,
        user,
        router,
        tokens,
        pool_hash,
        strategy,
    }
}

pub(crate) fn setup<'a>() -> Bench<'a> {
    setup_with_price(PRICE)
}

/// Depot dans la strategie avec la seule signature de `from` : le transfert
/// from -> strategie est son seul sous-appel autorise.
fn deposit(b: &Bench, from: &Address, amount: i128) -> Result<i128, ()> {
    b.env.mock_auths(&[MockAuth {
        address: from,
        invoke: &MockAuthInvoke {
            contract: &b.strategy.address,
            fn_name: "deposit",
            args: (amount, from.clone()).into_val(&b.env),
            sub_invokes: &[MockAuthInvoke {
                contract: &b.usdc.address,
                fn_name: "transfer",
                args: (from.clone(), b.strategy.address.clone(), amount).into_val(&b.env),
                sub_invokes: &[],
            }],
        },
    }]);
    let result = b.strategy.try_deposit(&amount, from);
    b.env.mock_all_auths();
    match result {
        Ok(Ok(balance)) => Ok(balance),
        _ => Err(()),
    }
}

/// Retrait avec la seule signature de `from`.
fn withdraw(b: &Bench, from: &Address, amount: i128) -> Result<i128, ()> {
    b.env.mock_auths(&[MockAuth {
        address: from,
        invoke: &MockAuthInvoke {
            contract: &b.strategy.address,
            fn_name: "withdraw",
            args: (amount, from.clone(), from.clone()).into_val(&b.env),
            sub_invokes: &[],
        },
    }]);
    let result = b.strategy.try_withdraw(&amount, from, from);
    b.env.mock_all_auths();
    match result {
        Ok(Ok(balance)) => Ok(balance),
        _ => Err(()),
    }
}

/// Swap d'un tiers dans le pool, sous mock_all_auths (ce n'est pas la
/// strategie qu'on eprouve ici).
fn swap_in_pool(b: &Bench, token_in: &Address, token_out: &Address, amount: i128) {
    swap(b, token_in, token_out, amount);
}

/// Idem, rend ce que le tiers a recu.
pub(crate) fn swap(b: &Bench, token_in: &Address, token_out: &Address, amount: i128) -> i128 {
    let trader = Address::generate(&b.env);
    StellarAssetClient::new(&b.env, token_in).mint(&trader, &amount);
    let chain = vec![
        &b.env,
        (b.tokens.clone(), b.pool_hash.clone(), token_out.clone()),
    ];
    b.router
        .swap_chained(&trader, &chain, token_in, &(amount as u128), &0) as i128
}

/// Reserves du pool, (EURC, USDC).
fn pool_reserves(b: &Bench) -> (f64, f64) {
    let reserves = aqua_pool_wasm::Client::new(&b.env, &b.strategy.config().pool).get_reserves();
    let (first, second) = (
        reserves.get(0).unwrap() as f64,
        reserves.get(1).unwrap() as f64,
    );
    if b.tokens.get(0).unwrap() == b.eurc.address {
        (first, second)
    } else {
        (second, first)
    }
}

/// Ramene le prix du pool sur le prix de reference par un swap de tiers, a
/// la maniere de align_aquarius_pool.sh : sans lui, l'entree qui suit celle
/// du vault paierait le decalage laisse par la premiere. Precision du
/// flottant largement suffisante pour un swap de test.
pub(crate) fn align_pool(b: &Bench, price: i128) {
    let (r_eurc, r_usdc) = pool_reserves(b);
    let k = r_eurc * r_usdc;
    let target_eurc = (k * 1e9 / price as f64).sqrt();
    // Frais de 30 pb preleves a l'entree : on les ajoute a la quantite.
    let gross = |x: f64| (x / 0.997) as i128;
    if target_eurc > r_eurc {
        swap(
            b,
            &b.eurc.address,
            &b.usdc.address,
            gross(target_eurc - r_eurc),
        );
    } else if target_eurc < r_eurc {
        let target_usdc = k / target_eurc;
        swap(
            b,
            &b.usdc.address,
            &b.eurc.address,
            gross(target_usdc - r_usdc),
        );
    }
    // Post-condition : prix du pool a 0,1 % du prix de reference.
    let (r_eurc, r_usdc) = pool_reserves(b);
    let gap = (r_usdc / r_eurc * 1e9 / price as f64 - 1.0).abs();
    assert!(gap < 0.001, "pool a {gap} du prix de reference");
}

fn after_hours(b: &Bench, hours: u64) {
    b.env.ledger().with_mut(|li| li.timestamp += hours * 3_600);
}

#[test]
fn deposit_values_the_position_at_its_fair_price() {
    let b = setup();
    let balance = deposit(&b, &b.user, 10_0000000).expect("depot");
    // Entree : swap d'un peu moins de la moitie (0,3 % de frais, impact sur
    // un pool de 1000) puis depot des deux jambes. Perte d'entree bien sous
    // 1 %, parts mortes en plus.
    assert!(balance < 10_0000000 - DEAD_SHARES, "{balance}");
    assert!(balance > 9_9500000, "{balance}");
    assert_eq!(b.strategy.balance(&b.user), balance);
    // Presque rien ne dort sur la strategie.
    assert!(b.usdc.balance(&b.strategy.address) < 1_0000000 / 1000);
}

/// `deposit` et `withdraw` rendent leur valeur par calcul, sans relire le
/// pool : elle doit egaler `balance` dans tous les cas, depot hors genese,
/// sortie partielle et sortie totale comprises.
#[test]
fn deposit_and_withdraw_return_exactly_what_balance_reads() {
    let b = setup();
    let other = Address::generate(&b.env);
    StellarAssetClient::new(&b.env, &b.usdc.address).mint(&other, &10_0000000);
    deposit(&b, &b.user, 10_0000000).expect("depot de genese");

    let after_deposit = deposit(&b, &other, 5_0000000).expect("second depot");
    assert_eq!(after_deposit, b.strategy.balance(&other));

    let after_partial = withdraw(&b, &other, 2_0000000).expect("sortie partielle");
    assert_eq!(after_partial, b.strategy.balance(&other));

    let after_full = withdraw(&b, &other, after_partial).expect("sortie totale");
    assert_eq!(after_full, b.strategy.balance(&other));
}

#[test]
fn a_large_swap_in_the_pool_does_not_move_the_fair_value() {
    let b = setup();
    deposit(&b, &b.user, 10_0000000).expect("depot");
    let before = b.strategy.balance(&b.user);

    // Un tiers deplace le prix du pool de pres de 20 % : la valeur equitable
    // ne bouge que des frais encaisses par le pool.
    swap_in_pool(&b, &b.usdc.address, &b.eurc.address, 100_0000000);
    let during = b.strategy.balance(&b.user);
    assert!(during >= before, "{during} < {before}");
    assert!(during - before < before / 1000, "{during} vs {before}");

    // Et le prix revenu, toujours rien a gagner pour personne.
    swap_in_pool(&b, &b.eurc.address, &b.usdc.address, 89_0000000);
    assert!(b.strategy.balance(&b.user) >= before);
}

#[test]
fn withdraw_delivers_within_the_margin_and_the_leaver_pays_the_exit() {
    let b = setup();
    deposit(&b, &b.user, 10_0000000).expect("depot");
    let shares_before = b.strategy.shares_of(&b.user);
    let usdc_before = b.usdc.balance(&b.user);

    withdraw(&b, &b.user, 4_0000000).expect("retrait");

    let received = b.usdc.balance(&b.user) - usdc_before;
    assert!((3_9600000..=4_0000000).contains(&received), "{received}");
    assert!(b.strategy.shares_of(&b.user) < shares_before);
}

#[test]
fn a_full_exit_returns_the_whole_position_within_the_margin() {
    let b = setup();
    deposit(&b, &b.user, 10_0000000).expect("depot");
    let all = b.strategy.balance(&b.user);
    let usdc_before = b.usdc.balance(&b.user);

    withdraw(&b, &b.user, all).expect("sortie complete");

    let received = b.usdc.balance(&b.user) - usdc_before;
    assert!(received * 100 >= all * 99, "{received} pour {all}");
    // Ne reste qu'une poussiere d'arrondi entre les parts LP brulees et la
    // valeur sortie, sous 0,1 % de la position.
    assert!(b.strategy.balance(&b.user) * 1000 < all);
}

#[test]
fn a_depositor_cannot_take_more_than_their_share() {
    let b = setup();
    let other = Address::generate(&b.env);
    StellarAssetClient::new(&b.env, &b.usdc.address).mint(&other, &10_0000000);
    deposit(&b, &b.user, 10_0000000).expect("depot");
    deposit(&b, &other, 1_0000000).expect("depot");
    // L'autre ne peut pas retirer la position du premier.
    assert!(withdraw(&b, &other, 5_0000000).is_err());
}

#[test]
fn entering_a_pool_priced_above_the_reference_reverts() {
    // Prix de reference 1,00 contre un pool a 1,1221 : l'EURC y est 12 %
    // trop cher, l'entree detruirait de la valeur.
    let b = setup_with_price(1_000_000_000);
    assert!(deposit(&b, &b.user, 10_0000000).is_err());
    assert_eq!(b.usdc.balance(&b.user), 1_000_0000000);
}

#[test]
fn a_stale_reference_price_blocks_every_valuation() {
    let b = setup();
    deposit(&b, &b.user, 10_0000000).expect("depot");
    after_hours(&b, 27);
    // L'erreur propre a la strategie n'est pas une StrategyError du trait :
    // le client la rend comme code de contrat brut.
    assert_eq!(
        b.strategy.try_balance(&b.user).unwrap_err(),
        Err(InvokeError::Contract(AquaStrategyError::StalePrice as u32))
    );

    // Le keeper republie : tout repart.
    b.strategy.set_reference_price(&b.keeper, &PRICE, &0);
    assert!(b.strategy.balance(&b.user) > 0);
}

#[test]
fn reference_price_publication_is_bounded() {
    let b = setup();
    after_hours(&b, 1);
    // Plus de 1 % d'ecart avec le precedent : refuse.
    let err = b
        .strategy
        .try_set_reference_price(&b.keeper, &(PRICE * 102 / 100), &0)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, AquaStrategyError::PriceStepTooLarge.into());
    // Dans le pas : accepte.
    b.strategy
        .set_reference_price(&b.keeper, &(PRICE * 1005 / 1000), &1_700_000_000);
    // Moins d'une heure apres : refuse.
    let err = b
        .strategy
        .try_set_reference_price(&b.keeper, &PRICE, &0)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, AquaStrategyError::PriceTooSoon.into());
    // Un inconnu : refuse.
    after_hours(&b, 1);
    let outsider = Address::generate(&b.env);
    let err = b
        .strategy
        .try_set_reference_price(&outsider, &PRICE, &0)
        .unwrap_err()
        .unwrap();
    assert_eq!(err, AquaStrategyError::NotAuthorized.into());
    // L'admin publie, et peut remplacer le keeper.
    b.strategy.set_reference_price(&b.admin, &PRICE, &0);
    b.strategy.set_keeper(&outsider);
    assert_eq!(b.strategy.keeper(), outsider);
    assert_eq!(b.strategy.reference_price().price, PRICE);
}

#[test]
#[should_panic(expected = "Error(Contract, #507)")]
fn the_constructor_rejects_an_unknown_pool() {
    let b = setup();
    let init_args: Vec<Val> = vec![
        &b.env,
        b.admin.into_val(&b.env),
        b.keeper.into_val(&b.env),
        b.router.address.into_val(&b.env),
        BytesN::<32>::from_array(&b.env, &[7; 32]).into_val(&b.env),
        b.eurc.address.into_val(&b.env),
        PRICE.into_val(&b.env),
        MAX_AGE.into_val(&b.env),
        100u32.into_val(&b.env),
        100u32.into_val(&b.env),
    ];
    b.env
        .register(AquariusLpStrategy, (b.usdc.address.clone(), init_args));
}

#[test]
fn a_pool_of_1e15_units_per_side_values_and_trades_without_overflow() {
    // 100 millions d'EURC a 7 decimales par jambe : bien au-dela du seuil de
    // depassement (5e14) de la premiere version du calcul.
    let reserve_eurc: i128 = 1_000_000_000_000_000;
    let b = setup_with(PRICE, reserve_eurc, reserve_eurc * PRICE / 1_000_000_000);
    let amount = 1_000_000_0000000; // 1 million d'USDC
    let value = deposit(&b, &b.user, amount).expect("depot");
    // 1 % du pool : l'impact d'entree (~0,4 %) reste dans la marge de 1 %.
    assert!(value * 100 > amount * 99, "{value}");
    withdraw(&b, &b.user, value / 2).expect("retrait");
}

#[test]
fn a_full_exit_succeeds_beside_another_depositor_after_fees_accrued() {
    let b = setup();
    let other = Address::generate(&b.env);
    StellarAssetClient::new(&b.env, &b.usdc.address).mint(&other, &10_0000000);
    deposit(&b, &b.user, 10_0000000).expect("depot");
    deposit(&b, &other, 3_0000000).expect("depot");
    // Allers-retours de tiers : le pool encaisse des frais, k grandit.
    for _ in 0..3 {
        swap_in_pool(&b, &b.usdc.address, &b.eurc.address, 20_0000000);
        swap_in_pool(&b, &b.eurc.address, &b.usdc.address, 17_8000000);
    }
    let other_before = b.strategy.balance(&other);

    let all = b.strategy.balance(&b.user);
    withdraw(&b, &b.user, all).expect("sortie complete");

    // L'autre deposant n'a rien perdu : le cout de sortie est au partant.
    assert!(b.strategy.balance(&other) >= other_before);
    assert!(b.strategy.balance(&b.user) * 1000 < all);
}

#[test]
fn donations_to_the_strategy_benefit_its_depositors_only() {
    let b = setup();
    deposit(&b, &b.user, 10_0000000).expect("depot");
    let before = b.strategy.balance(&b.user);
    // Dons d'USDC et d'EURC directement sur la strategie.
    StellarAssetClient::new(&b.env, &b.usdc.address).mint(&b.strategy.address, &1_0000000);
    StellarAssetClient::new(&b.env, &b.eurc.address).mint(&b.strategy.address, &1_0000000);
    let after = b.strategy.balance(&b.user);
    assert!(after > before + 2_0000000, "{after} vs {before}");

    // Un nouvel entrant paie le prix de part accru : il ne capte pas le don.
    let late = Address::generate(&b.env);
    StellarAssetClient::new(&b.env, &b.usdc.address).mint(&late, &5_0000000);
    let late_value = deposit(&b, &late, 5_0000000).expect("depot");
    assert!(late_value <= 5_0000000, "{late_value}");
}

#[test]
fn a_reference_price_move_between_entry_and_exit_is_absorbed_within_the_margin() {
    let b = setup();
    deposit(&b, &b.user, 10_0000000).expect("depot");
    // L'EUR prend 0,9 % : la position, a moitie en EURC, vaut ~0,45 % de
    // plus. Le pool n'est pas realigne : la sortie le paie, dans la marge.
    after_hours(&b, 1);
    b.strategy
        .set_reference_price(&b.keeper, &(PRICE * 1009 / 1000), &0);
    let value = b.strategy.balance(&b.user);
    withdraw(&b, &b.user, value / 2).expect("retrait");
}

#[test]
fn a_pool_with_swaps_killed_refuses_the_exit_and_keeps_the_shares() {
    let b = setup();
    deposit(&b, &b.user, 10_0000000).expect("depot");
    let shares = b.strategy.shares_of(&b.user);
    let pool = b
        .router
        .get_pools(&b.tokens)
        .get(b.pool_hash.clone())
        .unwrap();
    aqua_pool_wasm::Client::new(&b.env, &pool).kill_swap(&b.admin);

    // Risque accepte (journal D4) : l'admin Aquarius peut couper les swaps,
    // et la sortie revend l'EURC. Echec propre, parts intactes.
    assert!(withdraw(&b, &b.user, 1_0000000).is_err());
    assert_eq!(b.strategy.shares_of(&b.user), shares);
}

mod props {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        // Chaque cas monte la pile Aquarius reelle : peu de cas, mais des
        // montants et des historiques de frais quelconques.
        #![proptest_config(ProptestConfig::with_cases(24))]

        /// Une sortie complete reussit toujours, a cote d'un autre
        /// deposant, et ne laisse jamais de parts negatives : l'arrondi
        /// vers le haut des parts brulees est plafonne aux parts detenues.
        #[test]
        fn a_full_exit_always_succeeds_and_never_overdraws_shares(
            // Jusqu'a 1 % du pool par depot : au-dela, l'entree coute plus que
            // la marge de 1 % et elle est refusee, a juste titre (un depot de
            // 3,3 % du pool l'est).
            first in 1_0000000i128..11_0000000,
            second in 1_0000000i128..11_0000000,
            trades in 0usize..4,
            trade in 1_0000000i128..30_0000000,
        ) {
            let b = setup_quiet();
            let other = Address::generate(&b.env);
            StellarAssetClient::new(&b.env, &b.usdc.address).mint(&other, &second);
            deposit(&b, &b.user, first).expect("depot");
            deposit(&b, &other, second).expect("depot");
            for _ in 0..trades {
                swap_in_pool(&b, &b.usdc.address, &b.eurc.address, trade);
                swap_in_pool(&b, &b.eurc.address, &b.usdc.address, trade * 89 / 100);
            }
            let all = b.strategy.balance(&b.user);
            withdraw(&b, &b.user, all).expect("sortie complete");
            prop_assert!(b.strategy.shares_of(&b.user) >= 0);
            prop_assert!(b.strategy.shares_of(&other) > 0);
            let rest = b.strategy.balance(&other);
            withdraw(&b, &other, rest).expect("sortie complete de l'autre");
            prop_assert!(b.strategy.shares_of(&other) >= 0);
        }
    }
}

#![cfg(test)]
// Montants ecrits en convention Stellar 7 decimales (X_XXXXXXX, ex. 5_0000000
// = 5,0) : le groupement d'underscores suit les decimales de l'actif, pas les
// milliers, comme dans test_blend.rs du vault.
#![allow(clippy::inconsistent_digit_grouping, clippy::zero_prefixed_literal)]
// L'arite des clients generes par contractimport! est dictee par les ABI
// externes (jusqu'a 8 arguments), meme justification que dans venues/.
#![allow(clippy::too_many_arguments)]
//! Integration du routeur avec le stack Aquarius REEL (task 11, wasm
//! vendorises au commit epingle 84de10e0 de soroswap/aggregator, cf.
//! test_wasms/README.md) : router + plane + calculator + pool standard
//! (constant product) deployes depuis les wasm, pool USDC/EURC cree par
//! init_standard_pool puis alimente par deposit.
//!
//! Sources d'interface et de semantique (le repo canonique
//! AquaToken/soroban-amm est en 404) :
//! - spec embarque des wasm vendorises, lu par
//!   `stellar contract info interface --wasm ...` (signatures init_admin,
//!   set_pool_hash, set_token_hash, set_reward_token, set_reward_boost_config,
//!   configure_init_pool_payment, set_pools_plane, set_liquidity_calculator,
//!   init_standard_pool, deposit, get_reserves, swap_chained) ;
//! - miroir des sources : github.com/calc1f4r/soroban-amm@f9d4a5e0 (copie de
//!   la generation sdk 22 du canonique, meme perimetre que les wasm
//!   vendorises -- rssdkver 22.0.6 dans leur meta : boost config, liquidity
//!   calculator, locker feed), verifie le 22/07/2026 ;
//! - fixture aqua_setup.rs de soroswap/aggregator au commit epingle (chaine
//!   d'init de reference de leurs propres tests d'adapter).
//!
//! Ce que ce fichier prouve : la chaine d'init Aqua complete depuis les wasm,
//! la math constant-product avec fee 0,3 % SUR LA SORTIE du pool reel, la
//! convention d'appel de notre client swap_chained contre l'ABI reelle, la
//! topologie d'auth reelle de la venue (escrow transfer(user -> router Aqua),
//! couverte par la pre-autorisation generique authorize_venue_pull), et le
//! fallback REEL pool Aqua vide -> bascule Soroswap (les deux stacks reels
//! dans la meme fixture). Reste couvert par la demo testnet PR C : le
//! comportement du router Aquarius DEPLOYE (versions on-chain vs vendorees).

extern crate std;

use super::test_soroswap_stack::{EXPECTED_OUT as EXPECTED_OUT_SOROSWAP, FEE as FEE_SOROSWAP};
use super::test_stack_common::{self as common, AMOUNT_IN, AQUARIUS_FEE_BPS, MIN_OUT, RESERVE};
use super::{PairStats, SwapResult, SwapRouterClient, Venue};
use soroban_sdk::{
    contract, contractimpl,
    testutils::{Address as _, AuthorizedFunction, AuthorizedInvocation},
    token::{StellarAssetClient, TokenClient},
    vec, Address, BytesN, Env, IntoVal, Symbol, Vec,
};

mod aqua_router_wasm {
    soroban_sdk::contractimport!(file = "test_wasms/soroban_liquidity_pool_router_contract.wasm");
}
mod aqua_pool_wasm {
    soroban_sdk::contractimport!(file = "test_wasms/soroban_liquidity_pool_contract.wasm");
}
mod aqua_plane_wasm {
    soroban_sdk::contractimport!(file = "test_wasms/soroban_liquidity_pool_plane_contract.wasm");
}
mod aqua_calculator_wasm {
    soroban_sdk::contractimport!(
        file = "test_wasms/soroban_liquidity_pool_liquidity_calculator_contract.wasm"
    );
}
mod aqua_token_wasm {
    soroban_sdk::contractimport!(file = "test_wasms/soroban_token_contract.wasm");
}

/// fee_fraction du pool standard, en unites de 1/10 000 : 30 = 0,3 %. Le
/// router Aqua n'accepte que la liste blanche [10, 30, 100] (miroir,
/// liquidity_pool_router/src/constants.rs, CONSTANT_PRODUCT_FEE_AVAILABLE ;
/// erreur BadFee=302 du spec embarque sinon). 30 = meme taux nominal que
/// Soroswap, mais assiette et arrondi different (cf. EXPECTED_OUT_AQUA).
const AQUA_FEE_FRACTION: u32 = 30;

/// Montant sorti attendu, DERIVE du miroir des sources
/// (calc1f4r/soroban-amm@f9d4a5e0, liquidity_pool/src/pool.rs,
/// get_amount_out -- fee sur la SORTIE, arrondi plafond ; contraste
/// Soroswap : fee sur l'ENTREE) et calcule a la main :
///
///   out_brut = floor(in * reserve_out / (reserve_in + in))
///            = floor(50_000_000 * 10_000_000_000 / 10_050_000_000)
///            = floor(500_000_000_000_000_000 / 10_050_000_000)
///            = 49_751_243      (reste 7_850_000_000, troncature)
///   fee      = ceil(out_brut * fee_fraction / 10_000)
///            = ceil(49_751_243 * 30 / 10_000)
///            = ceil(149_253,729) = 149_254
///   out      = 49_751_243 - 149_254 = 49_601_989
///
/// Le fee LP reste dans la reserve du pool : apres swap, les reserves valent
/// (reserve_in + in, reserve_out - out) -- source : liquidity_pool/src/
/// contract.rs, fn swap (put_reserve du cote achete = reserve - out net).
const EXPECTED_OUT_AQUA: i128 = 4_9601989;

/// Frais COMPTABLES du routeur ForYield sur la venue Aquarius :
/// amount_in x 10 bps / 10 000 = 50_000. Sans rapport avec le fee LP du pool
/// (0,3 % sur la sortie) : pure comptabilite du routeur.
const FEE_AQUA: i128 = AMOUNT_IN * AQUARIUS_FEE_BPS as i128 / 10_000;

/// Feed de boost factice : la chaine d'init du router Aqua EXIGE un feed
/// (set_reward_boost_config, lu sans garde par init_standard_pool -- miroir,
/// pool_utils.rs), et le checkpoint de rewards du deposit invoque
/// feed.total_supply() sans try_ (miroir, rewards/src/manager.rs,
/// get_total_locked) : l'adresse doit porter un CONTRAT exportant
/// total_supply. Le locker feed canonique n'est pas vendorise (absent du
/// perimetre Task 9) ; total_supply = 0 rend le boost neutre (miroir,
/// calculate_effective_balance : total_locked = 0 -> balance effective =
/// balance de parts, aucun effet sur les rewards ni sur le swap).
#[contract]
struct MockBoostFeed;

#[contractimpl]
impl MockBoostFeed {
    pub fn total_supply(_env: Env) -> u128 {
        0
    }
}

struct AquaStack<'a> {
    router: aqua_router_wasm::Client<'a>,
    pool_index: BytesN<32>,
}

/// Chaine d'init Aqua complete depuis les wasm vendorises. Chaque etape est
/// OBLIGATOIRE : init_standard_pool lit token_hash, reward_token, boost
/// token/feed, plane et la config de paiement sans garde (absents ->
/// StorageError 501, miroir pool_utils.rs / rewards/src/storage.rs). Les
/// roles privilegies (rewards/operations/pause/emergency) retombent sur
/// l'admin via get_role_safe : set_privileged_addrs est omis a dessein.
/// Le calculator n'est pas exige par init_standard_pool mais fait partie du
/// cablage de reference (aqua_setup.rs) : branche pour rester conforme.
fn deploy_aqua_stack<'a>(base: &common::BaseFixture, with_liquidity: bool) -> AquaStack<'a> {
    let env = &base.env;
    let admin = &base.admin;

    let pool_hash = env.deployer().upload_contract_wasm(aqua_pool_wasm::WASM);
    let token_hash = env.deployer().upload_contract_wasm(aqua_token_wasm::WASM);

    let aqua_router = aqua_router_wasm::Client::new(env, &env.register(aqua_router_wasm::WASM, ()));
    aqua_router.init_admin(admin);
    aqua_router.set_pool_hash(admin, &pool_hash);
    aqua_router.set_token_hash(admin, &token_hash);

    let reward_token = env
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let boost_token = env
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let boost_feed = env.register(MockBoostFeed, ());
    aqua_router.set_reward_token(admin, &reward_token);
    aqua_router.set_reward_boost_config(admin, &boost_token, &boost_feed);
    // Le token de paiement est lu meme a montant nul (miroir, contract.rs,
    // init_standard_pool) : configure a 0, aucun transfert a la creation.
    aqua_router.configure_init_pool_payment(admin, &reward_token, &0, &0, admin);

    let plane = env.register(aqua_plane_wasm::WASM, ());
    aqua_router.set_pools_plane(admin, &plane);
    let calculator =
        aqua_calculator_wasm::Client::new(env, &env.register(aqua_calculator_wasm::WASM, ()));
    calculator.init_admin(admin);
    calculator.set_pools_plane(admin, &plane);
    aqua_router.set_liquidity_calculator(admin, &calculator.address);

    // Paire TRIEE par adresse : convention du router Aqua
    // (assert_tokens_sorted, erreur TokensNotSorted=2002 du spec embarque),
    // la meme que notre venue et notre registre appliquent.
    let tokens = sorted_pair_vec(env, &base.usdc.address, &base.eurc.address);
    let (pool_index, _pool) = aqua_router.init_standard_pool(admin, &tokens, &AQUA_FEE_FRACTION);

    if with_liquidity {
        StellarAssetClient::new(env, &base.usdc.address).mint(admin, &RESERVE);
        StellarAssetClient::new(env, &base.eurc.address).mint(admin, &RESERVE);
        aqua_router.deposit(
            admin,
            &tokens,
            &pool_index,
            &vec![env, RESERVE as u128, RESERVE as u128],
            &0,
        );
    }

    AquaStack {
        router: aqua_router,
        pool_index,
    }
}

fn sorted_pair_vec(env: &Env, a: &Address, b: &Address) -> Vec<Address> {
    if a < b {
        vec![env, a.clone(), b.clone()]
    } else {
        vec![env, b.clone(), a.clone()]
    }
}

struct AquaFixture<'a> {
    env: Env,
    user: Address,
    usdc: TokenClient<'a>,
    eurc: TokenClient<'a>,
    aqua: AquaStack<'a>,
    router: SwapRouterClient<'a>,
}

/// Socle commun + stack Aqua reel alimente RESERVE/RESERVE + routeur ForYield
/// branche sur le router Aqua, registre de pool renseigne (set_aqua_pool =
/// pool_index rendu par init_standard_pool, la cle de get_pools). La venue
/// Soroswap est une adresse sans contrat : jamais atteinte, la venue Aqua
/// preferee sert. SOROSWAP_FEE_BPS est donc INERTE ici : exige par
/// initialize, jamais lu (aucun swap ne se conclut sur la venue Soroswap
/// dans cette fixture).
fn setup_aqua_fixture<'a>() -> AquaFixture<'a> {
    let base = common::setup_base();
    let aqua = deploy_aqua_stack(&base, true);
    let router = common::init_router(&base, &Address::generate(&base.env), &aqua.router.address);
    router.set_aqua_pool(&base.usdc.address, &base.eurc.address, &aqua.pool_index);

    AquaFixture {
        env: base.env,
        user: base.user,
        usdc: base.usdc,
        eurc: base.eurc,
        aqua,
        router,
    }
}

/// Fixture du fallback REEL : les DEUX stacks reels cote a cote. Le pool
/// Aqua existe et est enregistre (registre non vide : la venue est bien
/// TENTEE) mais sans liquidite : le pool reel refuse (EmptyPool, spec
/// embarque LiquidityPoolValidationError), l'escrow du router Aqua est
/// annule par le revert de frame, et la bascule sert via Soroswap.
fn setup_fallback_fixture<'a>() -> (AquaFixture<'a>, common::SoroswapStack<'a>) {
    let base = common::setup_base();
    let aqua = deploy_aqua_stack(&base, false);
    let soroswap = common::deploy_soroswap_stack(&base);
    let router = common::init_router(&base, &soroswap.aggregator, &aqua.router.address);
    router.set_aqua_pool(&base.usdc.address, &base.eurc.address, &aqua.pool_index);

    (
        AquaFixture {
            env: base.env,
            user: base.user,
            usdc: base.usdc,
            eurc: base.eurc,
            aqua,
            router,
        },
        soroswap,
    )
}

/// Reserves du pool Aqua reordonnees en (usdc, eurc) : get_reserves suit
/// l'ordre des tokens TRIES par adresse, non deterministe entre deux runs.
fn aqua_reserves_usdc_eurc(f: &AquaFixture) -> (i128, i128) {
    let tokens = sorted_pair_vec(&f.env, &f.usdc.address, &f.eurc.address);
    let reserves = f.aqua.router.get_reserves(&tokens, &f.aqua.pool_index);
    let (reserve_0, reserve_1) = (
        reserves.get(0).unwrap() as i128,
        reserves.get(1).unwrap() as i128,
    );
    if f.usdc.address < f.eurc.address {
        (reserve_0, reserve_1)
    } else {
        (reserve_1, reserve_0)
    }
}

#[test]
fn swap_exact_in_serves_through_real_aqua_stack() {
    let f = setup_aqua_fixture();
    // Sanite de fixture : les reserves sont exactement celles de la
    // derivation de EXPECTED_OUT_AQUA.
    assert_eq!(aqua_reserves_usdc_eurc(&f), (RESERVE, RESERVE));

    let result = f.router.swap_exact_in(
        &f.user,
        &f.usdc.address,
        &f.eurc.address,
        &AMOUNT_IN,
        &MIN_OUT,
        &Venue::AquariusRouter,
    );

    // Montant sorti EXACT du constant product avec 0,3 % sur la sortie
    // (derive en tete de fichier), venue effective Aquarius, frais
    // comptables du routeur.
    assert_eq!(
        result,
        SwapResult {
            amount_out: EXPECTED_OUT_AQUA,
            venue: Venue::AquariusRouter,
            fee: FEE_AQUA,
        }
    );
    // `from` debite et credite ; invariant : solde du routeur NUL hors
    // transaction (sur les deux tokens).
    assert_eq!(f.usdc.balance(&f.user), 0);
    assert_eq!(f.eurc.balance(&f.user), EXPECTED_OUT_AQUA);
    assert_eq!(f.usdc.balance(&f.router.address), 0);
    assert_eq!(f.eurc.balance(&f.router.address), 0);
    // Contrepartie dans le pool : tout amount_in y entre, le net en sort,
    // le fee LP reste en reserve (cf. derivation).
    assert_eq!(
        aqua_reserves_usdc_eurc(&f),
        (RESERVE + AMOUNT_IN, RESERVE - EXPECTED_OUT_AQUA)
    );
    // Stats de la paire ordonnee enregistrees.
    assert_eq!(
        f.router.pair_stats(&f.usdc.address, &f.eurc.address),
        PairStats {
            volume_in: AMOUNT_IN,
            volume_out: EXPECTED_OUT_AQUA,
            fees: FEE_AQUA,
            swaps: 1,
        }
    );
}

// Fallback REEL (task 11 step 2) : preferred = Aquarius, pool Aqua reel
// EXISTANT mais vide -> le pool refuse (EmptyPool), le try_ de la venue
// absorbe, l'escrow deja tire par le router Aqua est annule par le revert de
// sa frame, et Soroswap sert dans la MEME transaction. La venue effective et
// les frais comptables sont ceux de Soroswap ; le montant servi est la
// derivation x*y=k de test_soroswap_stack.rs (memes reserves). Les reserves
// Aqua restent nulles : rien n'a ete execute sur la venue preferee.
#[test]
fn real_fallback_empty_aqua_pool_served_by_soroswap() {
    let (f, soroswap) = setup_fallback_fixture();
    assert_eq!(aqua_reserves_usdc_eurc(&f), (0, 0));

    let result = f.router.swap_exact_in(
        &f.user,
        &f.usdc.address,
        &f.eurc.address,
        &AMOUNT_IN,
        &MIN_OUT,
        &Venue::AquariusRouter,
    );

    assert_eq!(
        result,
        SwapResult {
            amount_out: EXPECTED_OUT_SOROSWAP,
            venue: Venue::SoroswapAggregator,
            fee: FEE_SOROSWAP,
        }
    );
    assert_eq!(f.usdc.balance(&f.user), 0);
    assert_eq!(f.eurc.balance(&f.user), EXPECTED_OUT_SOROSWAP);
    assert_eq!(f.usdc.balance(&f.router.address), 0);
    assert_eq!(f.eurc.balance(&f.router.address), 0);
    // Le pool Aqua vide n'a pas bouge ; la paire Soroswap a servi.
    assert_eq!(aqua_reserves_usdc_eurc(&f), (0, 0));
    let (pair_reserve_0, pair_reserve_1) = soroswap.pair.get_reserves();
    let pair_reserves = if f.usdc.address < f.eurc.address {
        (pair_reserve_0, pair_reserve_1)
    } else {
        (pair_reserve_1, pair_reserve_0)
    };
    assert_eq!(
        pair_reserves,
        (RESERVE + AMOUNT_IN, RESERVE - EXPECTED_OUT_SOROSWAP)
    );
    // Stats sur la venue EFFECTIVE (frais Soroswap), un seul swap.
    assert_eq!(
        f.router.pair_stats(&f.usdc.address, &f.eurc.address),
        PairStats {
            volume_in: AMOUNT_IN,
            volume_out: EXPECTED_OUT_SOROSWAP,
            fees: FEE_SOROSWAP,
            swaps: 1,
        }
    );
}

/// Arbre d'auth contre le stack Aqua REEL. Topologie etablie sur le miroir
/// des sources (contract.rs, fn swap_chained) et prouvee par ce happy path :
/// le router Aqua fait user.require_auth() dans SA frame (couvert par l'auth
/// d'invocateur DIRECT : notre routeur l'appelle sans intermediaire,
/// contrairement a Soroswap ou l'aggregator s'interpose devant le router)
/// puis ESCROW transfer(user -> router Aqua, in_amount) dans la frame du
/// token -- exactement l'entree generique de authorize_venue_pull. Sans
/// elle, ce transfert echouerait et le swap tomberait en fallback : le happy
/// path est le fil-piege (verifie par experience controlee : pre-autorisation
/// desactivee -> AllVenuesFailed). Les transferts internes router Aqua ->
/// pool sont pre-autorises par le router Aqua lui-meme (invoker trackers,
/// invisibles d'env.auths() par construction) : seule l'auth de `from`
/// apparait, le routeur ForYield nulle part.
#[test]
fn swap_records_only_user_auth_against_real_aqua_stack() {
    let f = setup_aqua_fixture();

    f.router.swap_exact_in(
        &f.user,
        &f.usdc.address,
        &f.eurc.address,
        &AMOUNT_IN,
        &MIN_OUT,
        &Venue::AquariusRouter,
    );

    let auths = f.env.auths();
    assert!(auths.iter().all(|(addr, _)| addr != &f.router.address));
    assert_eq!(
        auths,
        std::vec![(
            f.user.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    f.router.address.clone(),
                    Symbol::new(&f.env, "swap_exact_in"),
                    (
                        f.user.clone(),
                        f.usdc.address.clone(),
                        f.eurc.address.clone(),
                        AMOUNT_IN,
                        MIN_OUT,
                        Venue::AquariusRouter,
                    )
                        .into_val(&f.env),
                )),
                sub_invocations: std::vec![AuthorizedInvocation {
                    function: AuthorizedFunction::Contract((
                        f.usdc.address.clone(),
                        Symbol::new(&f.env, "transfer"),
                        (f.user.clone(), f.router.address.clone(), AMOUNT_IN).into_val(&f.env),
                    )),
                    sub_invocations: std::vec![],
                }],
            }
        )]
    );
}

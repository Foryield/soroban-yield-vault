#![cfg(test)]
// Montants ecrits en convention Stellar 7 decimales (X_XXXXXXX, ex. 5_0000000
// = 5,0) : le groupement d'underscores suit les decimales de l'actif, pas les
// milliers, comme dans test_blend.rs du vault.
#![allow(clippy::inconsistent_digit_grouping, clippy::zero_prefixed_literal)]
//! Integration du routeur avec le stack Soroswap REEL (wasm vendorises,
//! commit epingle 84de10e0, cf. test_wasms/README.md) : factory + pair +
//! router Soroswap + AGGREGATOR (construit localement depuis les sources
//! epinglees, aucun binaire canonique publie). Socle de fixture partage avec
//! le stack Aqua : cf. test_stack_common.rs.
//!
//! Ce que ce fichier prouve : la math x*y=k avec frais 0,3 % du pair reel,
//! la convention d'appel de notre client contre l'ABI reel de l'aggregator,
//! l'arbre d'auth reel (frame du router Soroswap + transfert vers la paire,
//! pre-autorises par pull_auth_entries), le sens STRICT de la comparaison de
//! deadline du router Soroswap, et le chemin d'erreur type quand la venue
//! refuse min_out. Reste couvert par la demo testnet PR C : le comportement
//! de l'aggregator DEPLOYE (versions on-chain vs commit epingle).

extern crate std;

use super::test_stack_common::{
    self as common, pair_wasm, router_wasm, AMOUNT_IN, LEDGER_TIME, MIN_OUT, RESERVE,
    SOROSWAP_FEE_BPS,
};
use super::{PairStats, RouterError, SwapResult, SwapRouterClient, Venue};
use soroban_sdk::{
    testutils::{Address as _, AuthorizedFunction, AuthorizedInvocation},
    token::TokenClient,
    vec, Address, Env, IntoVal, Symbol,
};

/// Montant sorti attendu, DERIVE de la source epinglee (soroswap/core,
/// contracts/library/src/quotes.rs, get_amount_out ; meme math cote pair,
/// contracts/pair/src/lib.rs) et calcule a la main :
///
///   fee        = ceil(amount_in * 3 / 1000)
///              = ceil(50_000_000 * 3 / 1000) = 150_000            (0,3 %)
///   in_net     = 50_000_000 - 150_000 = 49_850_000
///   amount_out = floor(in_net * reserve_out / (reserve_in + in_net))
///              = floor(49_850_000 * 10_000_000_000 / 10_049_850_000)
///              = floor(498_500_000_000_000_000 / 10_049_850_000)
///              = 49_602_730     (reste 3_909_500_000, troncature)
///
/// Partage avec test_aqua_stack.rs : le test de fallback reel y fait servir
/// le swap par CE stack Soroswap, sur les memes reserves.
pub const EXPECTED_OUT: i128 = 4_9602730;

/// Frais COMPTABLES du routeur ForYield : amount_in x 30 bps / 10 000
/// = 150_000. Egalite numerique fortuite avec le fee LP du pair (meme taux
/// 0,3 %, mais arrondi plafond cote pair, troncature cote routeur). Partage
/// avec le test de fallback reel de test_aqua_stack.rs (venue effective =
/// Soroswap, memes frais comptables).
pub const FEE: i128 = AMOUNT_IN * SOROSWAP_FEE_BPS as i128 / 10_000;

struct StackFixture<'a> {
    env: Env,
    user: Address,
    usdc: TokenClient<'a>,
    eurc: TokenClient<'a>,
    soroswap_router: router_wasm::Client<'a>,
    pair: pair_wasm::Client<'a>,
    router: SwapRouterClient<'a>,
}

/// Socle commun + stack Soroswap reel + routeur ForYield branche sur
/// l'aggregator. La venue Aquarius est une adresse sans contrat : registre
/// vide, la venue rend false sans appel (chemin fallback des tests d'echec).
/// AQUARIUS_FEE_BPS est donc INERTE ici : exige par initialize, jamais lu
/// (aucun swap ne se conclut sur la venue Aquarius dans ce fichier).
fn setup_stack<'a>() -> StackFixture<'a> {
    let base = common::setup_base();
    let stack = common::deploy_soroswap_stack(&base);
    let router = common::init_router(&base, &stack.aggregator, &Address::generate(&base.env));

    StackFixture {
        env: base.env,
        user: base.user,
        usdc: base.usdc,
        eurc: base.eurc,
        soroswap_router: stack.router,
        pair: stack.pair,
        router,
    }
}

/// Reserves du pair reordonnees en (usdc, eurc) : le pair stocke (token_0,
/// token_1) tries par adresse, cf. common::order_usdc_eurc.
fn reserves_usdc_eurc(f: &StackFixture) -> (i128, i128) {
    let (reserve_0, reserve_1) = f.pair.get_reserves();
    common::order_usdc_eurc(&f.usdc.address, &f.eurc.address, reserve_0, reserve_1)
}

#[test]
fn swap_exact_in_serves_through_real_soroswap_stack() {
    let f = setup_stack();
    // Sanite de fixture : les reserves sont exactement celles de la
    // derivation de EXPECTED_OUT.
    assert_eq!(reserves_usdc_eurc(&f), (RESERVE, RESERVE));

    let result = f.router.swap_exact_in(
        &f.user,
        &f.usdc.address,
        &f.eurc.address,
        &AMOUNT_IN,
        &MIN_OUT,
        &Venue::SoroswapAggregator,
    );

    // Montant sorti EXACT de x*y=k avec 0,3 % (derive en tete de fichier),
    // venue effective Soroswap, frais comptables du routeur.
    assert_eq!(
        result,
        SwapResult {
            amount_out: EXPECTED_OUT,
            venue: Venue::SoroswapAggregator,
            fee: FEE,
        }
    );
    // `from` debite et credite ; invariant : solde du routeur NUL hors
    // transaction (sur les deux tokens).
    assert_eq!(f.usdc.balance(&f.user), 0);
    assert_eq!(f.eurc.balance(&f.user), EXPECTED_OUT);
    assert_eq!(f.usdc.balance(&f.router.address), 0);
    assert_eq!(f.eurc.balance(&f.router.address), 0);
    // Contrepartie dans le pair : tout amount_in y entre (fee LP comprise),
    // EXPECTED_OUT en sort.
    assert_eq!(
        reserves_usdc_eurc(&f),
        (RESERVE + AMOUNT_IN, RESERVE - EXPECTED_OUT)
    );
    // Stats de la paire ordonnee enregistrees.
    assert_eq!(
        f.router.pair_stats(&f.usdc.address, &f.eurc.address),
        PairStats {
            volume_in: AMOUNT_IN,
            volume_out: EXPECTED_OUT,
            fees: FEE,
            swaps: 1,
        }
    );
}

// Suivi de revue Task 3 : sens de comparaison du deadline reel. Source
// epinglee (soroswap/core, router, ensure_deadline) : `now >= deadline` est
// REJETE, la comparaison est stricte -- `deadline = timestamp()` echouerait
// a chaque appel, d'ou `timestamp() + 1` dans venues/soroswap.rs. Ce test
// fige le constat contre le wasm reel, dans les deux sens de la frontiere.
#[test]
fn real_router_deadline_comparison_is_strict() {
    let f = setup_stack();
    let path = vec![&f.env, f.usdc.address.clone(), f.eurc.address.clone()];

    // now == deadline : rejete (erreur contrat DeadlineExpired), fonds
    // intacts, reserves inchangees.
    let expired = f.soroswap_router.try_swap_exact_tokens_for_tokens(
        &AMOUNT_IN,
        &0,
        &path,
        &f.user,
        &LEDGER_TIME,
    );
    assert!(expired.is_err());
    assert_eq!(f.usdc.balance(&f.user), AMOUNT_IN);
    assert_eq!(reserves_usdc_eurc(&f), (RESERVE, RESERVE));

    // now + 1 : accepte, et le montant servi est celui de la derivation
    // (les reserves n'ont pas bouge entre les deux appels).
    let amounts = f.soroswap_router.swap_exact_tokens_for_tokens(
        &AMOUNT_IN,
        &0,
        &path,
        &f.user,
        &(LEDGER_TIME + 1),
    );
    assert_eq!(amounts.last().unwrap(), EXPECTED_OUT);
    assert_eq!(f.eurc.balance(&f.user), EXPECTED_OUT);
}

/// Arbre d'auth contre le stack REEL : meme exigence que le test unitaire
/// (swap_records_only_user_auth_venue_pull_preauthorized). env.auths() ne
/// restitue que les account trackers : les pre-autorisations
/// authorize_as_current_contract (frame du router Soroswap, transferts vers
/// la paire) vivent dans les invoker trackers, invisibles par construction.
/// La seule auth enregistree est celle de `from`, racine swap_exact_in, avec
/// le transfert entrant en sous-invocation ; le routeur n'apparait nulle
/// part. Le happy path lui-meme est le fil-piege : sans l'arbre reel de
/// pull_auth_entries, le require_auth du router Soroswap echouerait et le
/// swap tomberait en AllVenuesFailed.
#[test]
fn swap_records_only_user_auth_against_real_stack() {
    let f = setup_stack();

    f.router.swap_exact_in(
        &f.user,
        &f.usdc.address,
        &f.eurc.address,
        &AMOUNT_IN,
        &MIN_OUT,
        &Venue::SoroswapAggregator,
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
                        Venue::SoroswapAggregator,
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

// min_out au-dessus du realisable contre le pair reel : l'aggregator refuse
// (InsufficientOutputAmount, juge sur delta de solde de `to`), le try_ de la
// venue absorbe, fallback vers Aquarius (registre vide -> false), donc
// AllVenuesFailed et revert INTEGRAL : fonds et reserves intacts, aucune
// stat. C'est le chemin d'erreur type du routeur face a un vrai marche qui
// ne peut pas servir le prix demande.
#[test]
fn min_out_above_achievable_fails_all_venues_and_funds_intact() {
    let f = setup_stack();

    let result = f.router.try_swap_exact_in(
        &f.user,
        &f.usdc.address,
        &f.eurc.address,
        &AMOUNT_IN,
        &(EXPECTED_OUT + 1),
        &Venue::SoroswapAggregator,
    );

    assert_eq!(result, Err(Ok(RouterError::AllVenuesFailed.into())));
    assert_eq!(f.usdc.balance(&f.user), AMOUNT_IN);
    assert_eq!(f.eurc.balance(&f.user), 0);
    assert_eq!(f.usdc.balance(&f.router.address), 0);
    assert_eq!(f.eurc.balance(&f.router.address), 0);
    assert_eq!(reserves_usdc_eurc(&f), (RESERVE, RESERVE));
    assert_eq!(
        f.router.pair_stats(&f.usdc.address, &f.eurc.address),
        PairStats {
            volume_in: 0,
            volume_out: 0,
            fees: 0,
            swaps: 0,
        }
    );
}

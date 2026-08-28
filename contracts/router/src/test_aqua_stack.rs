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
//! topologie d'auth reelle de la venue (escrow transfer(routeur ForYield ->
//! router Aqua), couverte par la pre-autorisation generique
//! authorize_venue_pull), et le comportement du routeur face a un pool reel
//! VIDE. Depuis le passage mono-venue du 28/08/2026, ce dernier cas n'est
//! plus un fallback vers Soroswap mais un echec typee VenueFailed : c'est le
//! point de defaillance unique assume, ici prouve contre le stack reel.
//! Reste couvert par la demo testnet PR C : le comportement du router
//! Aquarius DEPLOYE (versions on-chain vs vendorees).

extern crate std;

use super::test_stack_common::{
    self as common, sorted_pair_vec, AMOUNT_IN, AQUARIUS_FEE_BPS, MIN_OUT, RESERVE,
};
use super::{PairStats, RouterError, SwapResult, SwapRouterClient};
use soroban_sdk::{
    testutils::{AuthorizedFunction, AuthorizedInvocation},
    token::TokenClient,
    Address, Env, IntoVal, Symbol,
};

/// Montant sorti attendu, DERIVE du miroir des sources
/// (calc1f4r/soroban-amm@f9d4a5e0, liquidity_pool/src/pool.rs,
/// get_amount_out -- fee sur la SORTIE, arrondi plafond) et calcule a la
/// main :
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
pub const EXPECTED_OUT_AQUA: i128 = 4_9601989;

/// Frais COMPTABLES du routeur ForYield sur la venue Aquarius :
/// amount_in x 10 bps / 10 000 = 50_000. Sans rapport avec le fee LP du pool
/// (0,3 % sur la sortie) : pure comptabilite du routeur.
const FEE_AQUA: i128 = AMOUNT_IN * AQUARIUS_FEE_BPS as i128 / 10_000;

struct AquaFixture<'a> {
    env: Env,
    user: Address,
    usdc: TokenClient<'a>,
    eurc: TokenClient<'a>,
    aqua: common::AquaStack<'a>,
    router: SwapRouterClient<'a>,
}

/// Socle commun + stack Aqua reel alimente RESERVE/RESERVE + routeur ForYield
/// branche sur le router Aqua, registre de pool renseigne (set_aqua_pool =
/// pool_index rendu par init_standard_pool, la cle de get_pools).
///
/// `with_liquidity` a false donne la fixture du pool reel VIDE : le pool
/// existe et est enregistre (la venue est donc bien TENTEE), mais il refuse
/// le swap (EmptyPool, spec embarque LiquidityPoolValidationError).
fn setup_aqua_fixture<'a>(with_liquidity: bool) -> AquaFixture<'a> {
    let base = common::setup_base();
    let aqua = common::deploy_aqua_stack(&base, with_liquidity);
    let router = common::init_router(&base, &aqua.router.address);
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

/// Reserves du pool Aqua reordonnees en (usdc, eurc) : get_reserves suit
/// l'ordre des tokens TRIES par adresse, cf. common::order_usdc_eurc.
fn aqua_reserves_usdc_eurc(f: &AquaFixture) -> (i128, i128) {
    let tokens = sorted_pair_vec(&f.env, &f.usdc.address, &f.eurc.address);
    let reserves = f.aqua.router.get_reserves(&tokens, &f.aqua.pool_index);
    common::order_usdc_eurc(
        &f.usdc.address,
        &f.eurc.address,
        reserves.get(0).unwrap() as i128,
        reserves.get(1).unwrap() as i128,
    )
}

#[test]
fn swap_exact_in_serves_through_real_aqua_stack() {
    let f = setup_aqua_fixture(true);
    // Sanite de fixture : les reserves sont exactement celles de la
    // derivation de EXPECTED_OUT_AQUA.
    assert_eq!(aqua_reserves_usdc_eurc(&f), (RESERVE, RESERVE));

    let result = f.router.swap_exact_in(
        &f.user,
        &f.usdc.address,
        &f.eurc.address,
        &AMOUNT_IN,
        &MIN_OUT,
    );

    // Montant sorti EXACT du constant product avec 0,3 % sur la sortie
    // (derive en tete de fichier) et frais comptables du routeur.
    assert_eq!(
        result,
        SwapResult {
            amount_out: EXPECTED_OUT_AQUA,
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

// Pool reel EXISTANT mais VIDE : le pool refuse (EmptyPool), le try_ de la
// venue absorbe, l'escrow deja tire par le router Aqua est annule par le
// revert de sa frame, et le routeur panique en VenueFailed.
//
// Sous l'architecture a deux venues, ce cas basculait sur Soroswap et le
// swap aboutissait. Depuis le 28/08/2026 il echoue : c'est le point de
// defaillance unique assume avec le retrait de la seconde venue, et ce test
// en est la preuve contre le stack reel. Le cas n'a rien de theorique, le
// pool Aquarius de testnet a ete vide de son EURC par des tiers entre
// juillet et aout 2026.
#[test]
fn real_empty_aqua_pool_fails_with_venue_failed_and_reverts_funds() {
    let f = setup_aqua_fixture(false);
    assert_eq!(aqua_reserves_usdc_eurc(&f), (0, 0));

    let result = f.router.try_swap_exact_in(
        &f.user,
        &f.usdc.address,
        &f.eurc.address,
        &AMOUNT_IN,
        &MIN_OUT,
    );

    assert_eq!(result, Err(Ok(RouterError::VenueFailed.into())));
    // Revert integral : `from` intact sur les deux tokens, routeur vide,
    // pool inchange (l'escrow tire par le router Aqua est annule avec sa
    // frame).
    assert_eq!(f.usdc.balance(&f.user), AMOUNT_IN);
    assert_eq!(f.eurc.balance(&f.user), 0);
    assert_eq!(f.usdc.balance(&f.router.address), 0);
    assert_eq!(f.eurc.balance(&f.router.address), 0);
    assert_eq!(aqua_reserves_usdc_eurc(&f), (0, 0));
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

/// Arbre d'auth contre le stack Aqua REEL. Topologie etablie sur le miroir
/// des sources (contract.rs, fn swap_chained) et prouvee par ce happy path.
/// Le `user` de swap_chained est NOTRE routeur (lib.rs lui passe sa propre
/// adresse), pas l'utilisateur final : le router Aqua fait
/// user.require_auth() dans SA frame (couvert par l'auth d'invocateur
/// DIRECT : notre routeur l'appelle sans intermediaire) puis ESCROW
/// transfer(routeur ForYield -> router Aqua, in_amount) dans la frame du
/// token -- exactement l'entree generique de authorize_venue_pull. Sans
/// elle, ce transfert echouerait et le swap entier avec lui : le happy path
/// est le fil-piege (verifie par experience controlee : pre-autorisation
/// desactivee -> echec). Les transferts internes router Aqua ->
/// pool sont pre-autorises par le router Aqua lui-meme (invoker trackers,
/// invisibles d'env.auths() par construction) : seule l'auth de `from`
/// apparait, le routeur ForYield nulle part.
#[test]
fn swap_records_only_user_auth_against_real_aqua_stack() {
    let f = setup_aqua_fixture(true);

    f.router.swap_exact_in(
        &f.user,
        &f.usdc.address,
        &f.eurc.address,
        &AMOUNT_IN,
        &MIN_OUT,
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

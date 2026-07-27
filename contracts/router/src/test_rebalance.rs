#![cfg(test)]
// Montants ecrits en convention Stellar 7 decimales (X_XXXXXXX, ex. 5_0000000
// = 5,0) : le groupement d'underscores suit les decimales de l'actif, pas les
// milliers, comme dans test_blend.rs du vault.
#![allow(clippy::inconsistent_digit_grouping, clippy::zero_prefixed_literal)]
//! Chaine de rebalance COMPLETE dans un seul env : retrait du vault USDC (D1),
//! swap par le routeur (D4) contre le stack Soroswap reel, depot dans le vault
//! EURC (D3). C'est le scenario metier du livrable D4, jusqu'ici prouve par les
//! seuls trois hashes testnet de docs/evidence/d4-dex-routing.md : aucun test
//! ne traversait la frontiere entre les deux contrats.
//!
//! Ce que ce fichier prouve, qu'aucun test cote vault ni cote routeur ne
//! couvre : la valeur qui sort du vault USDC arrive INTEGRALEMENT dans le vault
//! EURC, au prix du marche pres ; rien ne se perd entre les trois appels ; et
//! l'echec du maillon central (min_out inatteignable) laisse l'utilisateur avec
//! ses fonds et une position reconstituable, sans rien echouer dans le routeur.
//!
//! Les trois appels sont trois invocations distinctes, comme les trois
//! transactions testnet : l'atomicite du routeur protege le swap, pas la chaine
//! entiere. C'est precisement ce que le test de l'echec verifie.
//!
//! Le vault est ici une contrepartie de plus dans les fixtures du routeur (au
//! meme titre que le stack Soroswap vendorise), branche en dependance de
//! developpement : le socle common est sous #[cfg(test)], donc inaccessible
//! depuis un crate tiers, et la dependance vault -> routeur n'existe pas
//! (aucun cycle).

use super::test_soroswap_stack::EXPECTED_OUT as SPOT_OUT;
use super::test_stack_common::{self as common, AMOUNT_IN, SOROSWAP_FEE_BPS};
use super::{PairStats, RouterError, SwapResult, SwapRouterClient, Venue};
use soroban_sdk::{testutils::Address as _, token::TokenClient, Address};
use yield_vault::{YieldVault, YieldVaultClient};

/// Parts mortes verrouillees au premier depot (MINIMUM_LIQUIDITY du vault,
/// constante privee : reprise ici comme valeur ATTENDUE, pas importee -- si le
/// vault la changeait, ces tests doivent le signaler, pas suivre en silence).
const DEAD_SHARES: i128 = 1_000;

/// Montant retire du vault USDC : l'utilisateur depose AMOUNT_IN a la genese,
/// recoit AMOUNT_IN - DEAD_SHARES parts, et son retrait integral vaut autant
/// d'actif (ratio parts:actif de 1 tant que la valorisation n'a pas bouge).
const REBALANCED_IN: i128 = AMOUNT_IN - DEAD_SHARES;

/// Montant sorti attendu, meme derivation que test_soroswap_stack.rs
/// (soroswap/core, get_amount_out), sur REBALANCED_IN au lieu de AMOUNT_IN :
///
///   fee        = ceil(49_999_000 * 3 / 1000) = 149_997              (0,3 %)
///   in_net     = 49_999_000 - 149_997 = 49_849_003
///   amount_out = floor(49_849_003 * 10_000_000_000 / 10_049_849_003)
///              = 49_601_743
///
/// Strictement inferieur au SPOT_OUT du swap de 5,0 USDC de
/// test_soroswap_stack.rs : les parts mortes retiennent DEAD_SHARES d'actif
/// dans le vault de depart, et cette retenue se voit jusqu'au bout de la chaine
/// (la frontiere entre les contrats ne fabrique pas de valeur). Rapport fige a
/// la COMPILATION ci-dessous : les deux montants sont des constantes derivees
/// de la meme source, un runtime assert n'y prouverait rien.
const EXPECTED_OUT: i128 = 4_9601743;
const _: () = assert!(EXPECTED_OUT < SPOT_OUT);

/// Frais COMPTABLES du routeur : REBALANCED_IN x 30 bps / 10 000.
const FEE: i128 = REBALANCED_IN * SOROSWAP_FEE_BPS as i128 / 10_000;

/// min_out du rebalance : cotation moins 1 %, la marge exacte employee pour la
/// demonstration testnet (docs/evidence/d4-dex-routing.md). Nom distinct du
/// MIN_OUT du socle commun : ce plancher-ci porte sur REBALANCED_IN.
const REBALANCE_MIN_OUT: i128 = EXPECTED_OUT * 99 / 100;

struct RebalanceFixture<'a> {
    user: Address,
    usdc: TokenClient<'a>,
    eurc: TokenClient<'a>,
    vault_usdc: YieldVaultClient<'a>,
    vault_eurc: YieldVaultClient<'a>,
    router: SwapRouterClient<'a>,
}

/// Socle commun + stack Soroswap reel + routeur ForYield + les DEUX vaults,
/// chacun initialise sans pool (`None`) : la valorisation Blend est hors sujet
/// ici, le vault D3 EURC tourne d'ailleurs en garde pure sur testnet. La venue
/// Aquarius est une adresse sans contrat (registre vide, la venue rend false).
fn setup_rebalance<'a>() -> RebalanceFixture<'a> {
    let base = common::setup_base();
    let stack = common::deploy_soroswap_stack(&base);
    let router = common::init_router(&base, &stack.aggregator, &Address::generate(&base.env));

    let vault_usdc = YieldVaultClient::new(&base.env, &base.env.register(YieldVault, ()));
    vault_usdc.initialize(&base.admin, &base.usdc.address, &None);
    let vault_eurc = YieldVaultClient::new(&base.env, &base.env.register(YieldVault, ()));
    vault_eurc.initialize(&base.admin, &base.eurc.address, &None);

    RebalanceFixture {
        user: base.user,
        usdc: base.usdc,
        eurc: base.eurc,
        vault_usdc,
        vault_eurc,
        router,
    }
}

/// Depose la mise de l'utilisateur dans le vault USDC puis l'en retire
/// integralement : premier maillon de la chaine, commun aux deux tests. Rend
/// le montant retire, verifie egal a REBALANCED_IN.
fn deposit_then_exit_usdc_vault(f: &RebalanceFixture) -> i128 {
    let shares = f.vault_usdc.deposit(&f.user, &AMOUNT_IN);
    assert_eq!(shares, AMOUNT_IN - DEAD_SHARES);
    assert_eq!(f.vault_usdc.total_assets(), AMOUNT_IN);

    let withdrawn = f.vault_usdc.withdraw(&f.user, &shares);
    assert_eq!(withdrawn, REBALANCED_IN);
    // Le vault de depart garde exactement les parts mortes, jamais rachetables.
    assert_eq!(f.vault_usdc.total_assets(), DEAD_SHARES);
    assert_eq!(f.vault_usdc.total_shares(), DEAD_SHARES);
    assert_eq!(f.vault_usdc.shares_of(&f.user), 0);
    assert_eq!(f.usdc.balance(&f.user), withdrawn);
    withdrawn
}

/// Chaine complete : la valeur sortie du vault USDC arrive integralement dans
/// le vault EURC, au prix du marche pres. Le test-fil-piege de la frontiere
/// entre les deux contrats : toute derive d'interface (montants, sens des
/// arguments, unites) casse ici et nulle part ailleurs.
#[test]
fn rebalance_moves_the_whole_position_from_usdc_vault_to_eurc_vault() {
    let f = setup_rebalance();
    let withdrawn = deposit_then_exit_usdc_vault(&f);

    let swap = f.router.swap_exact_in(
        &f.user,
        &f.usdc.address,
        &f.eurc.address,
        &withdrawn,
        &REBALANCE_MIN_OUT,
        &Venue::SoroswapAggregator,
    );
    assert_eq!(
        swap,
        SwapResult {
            amount_out: EXPECTED_OUT,
            venue: Venue::SoroswapAggregator,
            fee: FEE,
        }
    );
    // Rien en transit : le routeur ne detient aucun des deux actifs entre deux
    // invocations.
    assert_eq!(f.usdc.balance(&f.router.address), 0);
    assert_eq!(f.eurc.balance(&f.router.address), 0);

    let shares = f.vault_eurc.deposit(&f.user, &swap.amount_out);

    // CONSERVATION : tout ce que le swap a servi est entre dans le vault EURC,
    // aux parts mortes de la nouvelle genese pres. Aucun EURC ne reste en main.
    assert_eq!(shares, EXPECTED_OUT - DEAD_SHARES);
    assert_eq!(f.vault_eurc.total_assets(), EXPECTED_OUT);
    assert_eq!(f.vault_eurc.total_shares(), EXPECTED_OUT);
    assert_eq!(f.vault_eurc.shares_of(&f.user), EXPECTED_OUT - DEAD_SHARES);
    assert_eq!(f.eurc.balance(&f.user), 0);
    assert_eq!(f.usdc.balance(&f.user), 0);
    assert_eq!(
        f.router.pair_stats(&f.usdc.address, &f.eurc.address),
        PairStats {
            volume_in: REBALANCED_IN,
            volume_out: EXPECTED_OUT,
            fees: FEE,
            swaps: 1,
        }
    );
    // Le vault de depart n'a pas bouge pendant les deux maillons suivants.
    assert_eq!(f.vault_usdc.total_assets(), DEAD_SHARES);
}

/// Maillon central en echec (min_out inatteignable) : la chaine n'etant pas
/// atomique de bout en bout, l'utilisateur se retrouve avec ses USDC en main,
/// et c'est le comportement VOULU -- rien n'est echoue dans le routeur, le
/// vault d'arrivee reste vierge, et la position se reconstitue par un simple
/// redepot dans le vault de depart. Sans ce test, une regression qui laisserait
/// les fonds dans le routeur passerait toutes les suites existantes.
#[test]
fn failed_swap_leaves_the_position_recoverable_and_the_router_empty() {
    let f = setup_rebalance();
    let withdrawn = deposit_then_exit_usdc_vault(&f);

    let failed = f.router.try_swap_exact_in(
        &f.user,
        &f.usdc.address,
        &f.eurc.address,
        &withdrawn,
        &(EXPECTED_OUT + 1),
        &Venue::SoroswapAggregator,
    );

    // Soroswap refuse le prix, Aquarius a un registre vide : les deux venues
    // echouent, tout revert.
    assert_eq!(failed, Err(Ok(RouterError::AllVenuesFailed.into())));
    assert_eq!(f.usdc.balance(&f.user), withdrawn);
    assert_eq!(f.usdc.balance(&f.router.address), 0);
    assert_eq!(f.eurc.balance(&f.router.address), 0);
    assert_eq!(f.eurc.balance(&f.user), 0);
    // Vault d'arrivee jamais touche, aucune stat enregistree.
    assert_eq!(f.vault_eurc.total_assets(), 0);
    assert_eq!(f.vault_eurc.total_shares(), 0);
    assert_eq!(
        f.router.pair_stats(&f.usdc.address, &f.eurc.address),
        PairStats {
            volume_in: 0,
            volume_out: 0,
            fees: 0,
            swaps: 0,
        }
    );

    // Reconstitution : le redepot dans le vault de depart rend a l'utilisateur
    // la totalite de la valeur retiree. Les parts restees dans le vault valent
    // toujours DEAD_SHARES d'actif, le ratio parts:actif est donc de 1 et les
    // parts emises egalent le montant redepose.
    let shares = f.vault_usdc.deposit(&f.user, &withdrawn);
    assert_eq!(shares, withdrawn);
    assert_eq!(f.usdc.balance(&f.user), 0);
    // Le vault est revenu a son etat d'avant rebalance, a l'unite pres : les
    // actifs sont de nouveau la (vault sans pool, donc solde token = actifs)
    // et le total des parts a retrouve la valeur de la genese.
    assert_eq!(f.vault_usdc.total_assets(), AMOUNT_IN);
    assert_eq!(f.usdc.balance(&f.vault_usdc.address), AMOUNT_IN);
    assert_eq!(f.vault_usdc.total_shares(), AMOUNT_IN);
}

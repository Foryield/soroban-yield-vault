#![cfg(test)]
extern crate std;

use super::test_mocks::{
    MockAggregator, MockAggregatorClient, MockAqua, MockAquaClient, MockBehavior,
};
use super::{
    venues, DataKey, PairStats, RouterError, SwapResult, SwapRouter, SwapRouterClient, Venue,
};
use soroban_sdk::{
    testutils::{Address as _, AuthorizedFunction, AuthorizedInvocation},
    token::{StellarAssetClient, TokenClient},
    Address, BytesN, Env, IntoVal, Symbol,
};

struct Fixture<'a> {
    env: Env,
    admin: Address,
    soroswap: Address,
    aquarius: Address,
    router: SwapRouterClient<'a>,
}

const SOROSWAP_FEE_BPS: u32 = 30;
const AQUARIUS_FEE_BPS: u32 = 10;

fn setup<'a>() -> Fixture<'a> {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let soroswap = Address::generate(&env);
    let aquarius = Address::generate(&env);

    let router_id = env.register(SwapRouter, ());
    let router = SwapRouterClient::new(&env, &router_id);
    router.initialize(
        &admin,
        &soroswap,
        &aquarius,
        &SOROSWAP_FEE_BPS,
        &AQUARIUS_FEE_BPS,
    );

    Fixture {
        env,
        admin,
        soroswap,
        aquarius,
        router,
    }
}

#[test]
fn initialize_stores_admin_venues_and_fees() {
    let f = setup();

    // Les getters sont prives (aucune surface publique de lecture des venues
    // en D4) : on les exerce depuis le contexte du contrat.
    f.env.as_contract(&f.router.address, || {
        assert_eq!(SwapRouter::admin(&f.env), f.admin);
        assert_eq!(
            SwapRouter::venue_addr(&f.env, Venue::SoroswapAggregator),
            f.soroswap
        );
        assert_eq!(
            SwapRouter::venue_addr(&f.env, Venue::AquariusRouter),
            f.aquarius
        );
        assert_eq!(
            SwapRouter::fee_bps(&f.env, Venue::SoroswapAggregator),
            SOROSWAP_FEE_BPS
        );
        assert_eq!(
            SwapRouter::fee_bps(&f.env, Venue::AquariusRouter),
            AQUARIUS_FEE_BPS
        );
    });
}

#[test]
fn double_initialize_fails_with_already_initialized() {
    let f = setup();

    let result = f.router.try_initialize(
        &f.admin,
        &f.soroswap,
        &f.aquarius,
        &SOROSWAP_FEE_BPS,
        &AQUARIUS_FEE_BPS,
    );

    // initialize retourne () : le client try_ type l'erreur en
    // soroban_sdk::Error, la comparaison passe par la conversion contracterror.
    assert_eq!(result, Err(Ok(RouterError::AlreadyInitialized.into())));
}

// --- Fumee des clients de venues (Task 3) : mock repond, client try_ OK.
// Le routage complet (fallback, min-out par delta de solde) arrive en
// Tasks 4-5.

const AMOUNT_IN: i128 = 5_0000000;
const SERVED_OUT: i128 = 4_9000000;
const MIN_OUT: i128 = 4_8000000;

struct VenueFixture<'a> {
    env: Env,
    user: Address,
    token_in: TokenClient<'a>,
    token_out: TokenClient<'a>,
}

/// Deux tokens de test : `user` detient AMOUNT_IN de token_in, le mock sera
/// pre-finance en token_out par `fund` (le mock sert depuis son propre solde,
/// cf. test_mocks).
fn venue_setup<'a>() -> VenueFixture<'a> {
    let env = Env::default();
    // Au niveau unitaire, le require_auth du transfert de `to`/`user` est
    // imbrique sous l'appel de venue (non rattache a la racine) : la variante
    // allowing_non_root_auth est necessaire. Dans le vrai flux (Task 4),
    // c'est le ROUTEUR qui pre-autorise son propre transfert via
    // authorize_as_current_contract, comme pool_supply du vault.
    env.mock_all_auths_allowing_non_root_auth();

    let issuer = Address::generate(&env);
    let user = Address::generate(&env);
    let token_in = TokenClient::new(
        &env,
        &env.register_stellar_asset_contract_v2(issuer.clone())
            .address(),
    );
    let token_out = TokenClient::new(
        &env,
        &env.register_stellar_asset_contract_v2(issuer).address(),
    );
    StellarAssetClient::new(&env, &token_in.address).mint(&user, &AMOUNT_IN);

    VenueFixture {
        env,
        user,
        token_in,
        token_out,
    }
}

fn fund(f: &VenueFixture, holder: &Address, amount: i128) {
    StellarAssetClient::new(&f.env, &f.token_out.address).mint(holder, &amount);
}

#[test]
fn soroswap_attempt_against_mock_moves_real_tokens() {
    let f = venue_setup();
    let mock = f.env.register(MockAggregator, ());
    MockAggregatorClient::new(&f.env, &mock).set_behavior(&MockBehavior::Serve(SERVED_OUT));
    fund(&f, &mock, SERVED_OUT);

    let ok = venues::soroswap::attempt(
        &f.env,
        &mock,
        &f.token_in.address,
        &f.token_out.address,
        AMOUNT_IN,
        MIN_OUT,
        &f.user,
    );

    assert!(ok);
    // Vrai flux de fonds : token_in tire depuis `to`, token_out servi a `to`.
    assert_eq!(f.token_in.balance(&f.user), 0);
    assert_eq!(f.token_in.balance(&mock), AMOUNT_IN);
    assert_eq!(f.token_out.balance(&f.user), SERVED_OUT);
    assert_eq!(f.token_out.balance(&mock), 0);
    // Preuve que le marqueur d'invocation est vivant (il fonde le test de
    // garde sur montants negatifs).
    assert!(MockAggregatorClient::new(&f.env, &mock).was_called());
}

#[test]
fn aqua_attempt_against_mock_moves_real_tokens() {
    let f = venue_setup();
    let mock = f.env.register(MockAqua, ());
    MockAquaClient::new(&f.env, &mock).set_behavior(&MockBehavior::Serve(SERVED_OUT));
    fund(&f, &mock, SERVED_OUT);
    let pool_hash = BytesN::from_array(&f.env, &[7u8; 32]);

    let ok = venues::aqua::attempt(
        &f.env,
        &mock,
        &f.token_in.address,
        &f.token_out.address,
        AMOUNT_IN,
        MIN_OUT,
        &f.user,
        &pool_hash,
    );

    assert!(ok);
    assert_eq!(f.token_in.balance(&f.user), 0);
    assert_eq!(f.token_in.balance(&mock), AMOUNT_IN);
    assert_eq!(f.token_out.balance(&f.user), SERVED_OUT);
    assert_eq!(f.token_out.balance(&mock), 0);
    // Meme preuve de vie du marqueur que cote aggregator.
    assert!(MockAquaClient::new(&f.env, &mock).was_called());
}

#[test]
fn attempt_returns_false_when_venue_panics_and_rolls_back() {
    let f = venue_setup();
    let mock = f.env.register(MockAggregator, ());
    MockAggregatorClient::new(&f.env, &mock).set_behavior(&MockBehavior::Panic);

    let ok = venues::soroswap::attempt(
        &f.env,
        &mock,
        &f.token_in.address,
        &f.token_out.address,
        AMOUNT_IN,
        MIN_OUT,
        &f.user,
    );

    // Le try_ absorbe la panne ET l'invocation ratee est annulee : aucun
    // token n'a bouge.
    assert!(!ok);
    assert_eq!(f.token_in.balance(&f.user), AMOUNT_IN);
    assert_eq!(f.token_in.balance(&mock), 0);
}

#[test]
fn aqua_attempt_panicking_mock_returns_false() {
    let f = venue_setup();
    let mock = f.env.register(MockAqua, ());
    MockAquaClient::new(&f.env, &mock).set_behavior(&MockBehavior::Panic);
    let pool_hash = BytesN::from_array(&f.env, &[7u8; 32]);

    let ok = venues::aqua::attempt(
        &f.env,
        &mock,
        &f.token_in.address,
        &f.token_out.address,
        AMOUNT_IN,
        MIN_OUT,
        &f.user,
        &pool_hash,
    );

    assert!(!ok);
    assert_eq!(f.token_in.balance(&f.user), AMOUNT_IN);
}

#[test]
fn aqua_attempt_returns_false_on_negative_amounts_without_calling_venue() {
    let f = venue_setup();
    let mock = f.env.register(MockAqua, ());
    MockAquaClient::new(&f.env, &mock).set_behavior(&MockBehavior::Serve(SERVED_OUT));
    fund(&f, &mock, SERVED_OUT);
    let pool_hash = BytesN::from_array(&f.env, &[7u8; 32]);

    for (amount_in, min_out) in [(-1_i128, MIN_OUT), (AMOUNT_IN, -1_i128)] {
        let ok = venues::aqua::attempt(
            &f.env,
            &mock,
            &f.token_in.address,
            &f.token_out.address,
            amount_in,
            min_out,
            &f.user,
            &pool_hash,
        );
        assert!(!ok);
    }
    // Ce test fige le contrat observable : attempt rend false, les fonds
    // sont intacts, et aucun appel de venue ABOUTI n'a eu lieu (le marqueur
    // absent est un fil-piege valide contre un appel complete). Il ne peut
    // PAS distinguer le retour anticipe d'un appel invoque puis annule par
    // rollback : l'ecriture du marqueur serait annulee avec la frame, tout
    // marqueur en storage a cette limite. La preuve directe de la garde
    // arrive en Task 6, via un helper de conversion pur teste aux bornes.
    assert!(!MockAquaClient::new(&f.env, &mock).was_called());
    assert_eq!(f.token_in.balance(&f.user), AMOUNT_IN);
}

// --- swap_exact_in (Task 4) : gardes typees, chemin nominal Soroswap,
// invariant AllVenuesFailed, garde fee_bps, observation des auths.

/// Frais comptables attendus par venue : amount_in x fee_bps / 10 000.
const SOROSWAP_FEE: i128 = AMOUNT_IN * SOROSWAP_FEE_BPS as i128 / 10_000;
const AQUARIUS_FEE: i128 = AMOUNT_IN * AQUARIUS_FEE_BPS as i128 / 10_000;

struct SwapFixture<'a> {
    env: Env,
    user: Address,
    /// Adresse du MockAggregator, branche comme venue Soroswap du routeur.
    soroswap: Address,
    /// Adresse du MockAqua, branche comme venue Aquarius du routeur.
    aquarius: Address,
    router: SwapRouterClient<'a>,
    token_in: TokenClient<'a>,
    token_out: TokenClient<'a>,
}

/// Routeur branche sur les MOCKS de venues, deux tokens reels, `user`
/// finance en token_in. Auth mockee NON permissive (mock_all_auths simple,
/// non-root interdit) : une pre-autorisation authorize_as_current_contract
/// manquante cote routeur fait ECHOUER le swap ici meme, pas seulement
/// contre la stack reelle (cf. test des auths).
fn swap_setup<'a>() -> SwapFixture<'a> {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let issuer = Address::generate(&env);
    let token_in = TokenClient::new(
        &env,
        &env.register_stellar_asset_contract_v2(issuer.clone())
            .address(),
    );
    let token_out = TokenClient::new(
        &env,
        &env.register_stellar_asset_contract_v2(issuer).address(),
    );
    StellarAssetClient::new(&env, &token_in.address).mint(&user, &AMOUNT_IN);

    let soroswap = env.register(MockAggregator, ());
    let aquarius = env.register(MockAqua, ());
    let router = SwapRouterClient::new(&env, &env.register(SwapRouter, ()));
    router.initialize(
        &admin,
        &soroswap,
        &aquarius,
        &SOROSWAP_FEE_BPS,
        &AQUARIUS_FEE_BPS,
    );

    SwapFixture {
        env,
        user,
        soroswap,
        aquarius,
        router,
        token_in,
        token_out,
    }
}

/// Ecrit l'entree de registre Aqua DIRECTEMENT en storage instance du
/// routeur, cle = paire TRIEE par adresse (meme convention que `aqua_pool`) :
/// le setter public admin arrive en Task 6, les tests de la matrice n'en
/// dependent pas.
fn set_aqua_registry(f: &SwapFixture, pool_hash: &BytesN<32>) {
    let (a, b) = if f.token_in.address < f.token_out.address {
        (f.token_in.address.clone(), f.token_out.address.clone())
    } else {
        (f.token_out.address.clone(), f.token_in.address.clone())
    };
    f.env.as_contract(&f.router.address, || {
        f.env
            .storage()
            .instance()
            .set(&DataKey::AquaPool(a, b), pool_hash);
    });
}

/// Stats zero : etat attendu de la paire apres tout swap ECHOUE (le revert
/// emporte l'ecriture de stats avec lui).
fn zero_stats() -> PairStats {
    PairStats {
        volume_in: 0,
        volume_out: 0,
        fees: 0,
        swaps: 0,
    }
}

#[test]
fn swap_exact_in_rejects_non_positive_amount_in() {
    let f = swap_setup();

    for amount_in in [0_i128, -1] {
        let result = f.router.try_swap_exact_in(
            &f.user,
            &f.token_in.address,
            &f.token_out.address,
            &amount_in,
            &MIN_OUT,
            &Venue::SoroswapAggregator,
        );
        assert_eq!(result, Err(Ok(RouterError::AmountMustBePositive.into())));
    }
}

#[test]
fn swap_exact_in_rejects_non_positive_min_out() {
    let f = swap_setup();

    for min_out in [0_i128, -1] {
        let result = f.router.try_swap_exact_in(
            &f.user,
            &f.token_in.address,
            &f.token_out.address,
            &AMOUNT_IN,
            &min_out,
            &Venue::SoroswapAggregator,
        );
        assert_eq!(result, Err(Ok(RouterError::MinOutMustBePositive.into())));
    }
}

#[test]
fn swap_exact_in_rejects_same_token() {
    let f = swap_setup();

    let result = f.router.try_swap_exact_in(
        &f.user,
        &f.token_in.address,
        &f.token_in.address,
        &AMOUNT_IN,
        &MIN_OUT,
        &Venue::SoroswapAggregator,
    );

    assert_eq!(result, Err(Ok(RouterError::SameToken.into())));
}

#[test]
fn swap_exact_in_serves_via_preferred_soroswap() {
    let f = swap_setup();
    MockAggregatorClient::new(&f.env, &f.soroswap).set_behavior(&MockBehavior::Serve(SERVED_OUT));
    StellarAssetClient::new(&f.env, &f.token_out.address).mint(&f.soroswap, &SERVED_OUT);

    let result = f.router.swap_exact_in(
        &f.user,
        &f.token_in.address,
        &f.token_out.address,
        &AMOUNT_IN,
        &MIN_OUT,
        &Venue::SoroswapAggregator,
    );

    assert_eq!(
        result,
        SwapResult {
            amount_out: SERVED_OUT,
            venue: Venue::SoroswapAggregator,
            fee: SOROSWAP_FEE,
        }
    );
    // `from` debite de amount_in, credite du produit du swap.
    assert_eq!(f.token_in.balance(&f.user), 0);
    assert_eq!(f.token_out.balance(&f.user), SERVED_OUT);
    // Invariant : solde du routeur NUL hors transaction.
    assert_eq!(f.token_in.balance(&f.router.address), 0);
    assert_eq!(f.token_out.balance(&f.router.address), 0);
    // Stats de la paire ordonnee incrementees.
    assert_eq!(
        f.router
            .pair_stats(&f.token_in.address, &f.token_out.address),
        PairStats {
            volume_in: AMOUNT_IN,
            volume_out: SERVED_OUT,
            fees: SOROSWAP_FEE,
            swaps: 1,
        }
    );
}

// INVARIANT (suivi de revue Task 3) : le chemin « toutes venues false » DOIT
// paniquer (AllVenuesFailed), jamais retourner. C'est lui qui garantit le
// revert INTEGRAL quand une venue a execute mais que `attempt` a rendu false
// (retour indecodable, conversion) : les fonds sont proteges par l'atomicite
// de la transaction, pas par le jugement local.
#[test]
fn all_venues_failing_panics_and_reverts_funds() {
    let f = swap_setup();
    MockAggregatorClient::new(&f.env, &f.soroswap).set_behavior(&MockBehavior::Panic);
    // Registre Aqua vide : la venue Aquarius rend false sans etre appelee.

    let result = f.router.try_swap_exact_in(
        &f.user,
        &f.token_in.address,
        &f.token_out.address,
        &AMOUNT_IN,
        &MIN_OUT,
        &Venue::SoroswapAggregator,
    );

    assert_eq!(result, Err(Ok(RouterError::AllVenuesFailed.into())));
    // Atomicite : le transfert entrant (from -> routeur) a eu lieu AVANT la
    // panique, il est integralement annule avec elle.
    assert_eq!(f.token_in.balance(&f.user), AMOUNT_IN);
    assert_eq!(f.token_in.balance(&f.router.address), 0);
}

// --- Matrice de fallback (Task 5) : chaque test asserte la venue EFFECTIVE
// dans SwapResult ET dans les stats, plus les soldes de `from`.

#[test]
fn fallback_preferred_soroswap_panics_aqua_serves() {
    let f = swap_setup();
    MockAggregatorClient::new(&f.env, &f.soroswap).set_behavior(&MockBehavior::Panic);
    MockAquaClient::new(&f.env, &f.aquarius).set_behavior(&MockBehavior::Serve(SERVED_OUT));
    StellarAssetClient::new(&f.env, &f.token_out.address).mint(&f.aquarius, &SERVED_OUT);
    set_aqua_registry(&f, &BytesN::from_array(&f.env, &[7u8; 32]));

    let result = f.router.swap_exact_in(
        &f.user,
        &f.token_in.address,
        &f.token_out.address,
        &AMOUNT_IN,
        &MIN_OUT,
        &Venue::SoroswapAggregator,
    );

    // Venue EFFECTIVE = secours ; frais comptes au bareme de la venue qui a
    // SERVI (aquarius_fee_bps), pas de la preferee.
    assert_eq!(
        result,
        SwapResult {
            amount_out: SERVED_OUT,
            venue: Venue::AquariusRouter,
            fee: AQUARIUS_FEE,
        }
    );
    assert_eq!(f.token_in.balance(&f.user), 0);
    assert_eq!(f.token_out.balance(&f.user), SERVED_OUT);
    assert_eq!(f.token_in.balance(&f.router.address), 0);
    assert_eq!(f.token_out.balance(&f.router.address), 0);
    assert_eq!(
        f.router
            .pair_stats(&f.token_in.address, &f.token_out.address),
        PairStats {
            volume_in: AMOUNT_IN,
            volume_out: SERVED_OUT,
            fees: AQUARIUS_FEE,
            swaps: 1,
        }
    );
}

#[test]
fn fallback_preferred_serves_under_min_aqua_serves() {
    let f = swap_setup();
    // Serve sous min_out : le mock revert (min propage), comme une venue
    // reelle ; le fallback doit traverser vers Aquarius.
    MockAggregatorClient::new(&f.env, &f.soroswap).set_behavior(&MockBehavior::Serve(MIN_OUT - 1));
    MockAquaClient::new(&f.env, &f.aquarius).set_behavior(&MockBehavior::Serve(SERVED_OUT));
    StellarAssetClient::new(&f.env, &f.token_out.address).mint(&f.aquarius, &SERVED_OUT);
    set_aqua_registry(&f, &BytesN::from_array(&f.env, &[7u8; 32]));

    let result = f.router.swap_exact_in(
        &f.user,
        &f.token_in.address,
        &f.token_out.address,
        &AMOUNT_IN,
        &MIN_OUT,
        &Venue::SoroswapAggregator,
    );

    assert_eq!(
        result,
        SwapResult {
            amount_out: SERVED_OUT,
            venue: Venue::AquariusRouter,
            fee: AQUARIUS_FEE,
        }
    );
    assert_eq!(f.token_in.balance(&f.user), 0);
    assert_eq!(f.token_out.balance(&f.user), SERVED_OUT);
    assert_eq!(
        f.router
            .pair_stats(&f.token_in.address, &f.token_out.address)
            .fees,
        AQUARIUS_FEE
    );
}

// Complement du test Task 4 (Panic + registre vide) : ici les DEUX venues
// sont presentes et executent, et les deux paniquent.
#[test]
fn both_venues_present_and_panicking_reverts_funds() {
    let f = swap_setup();
    MockAggregatorClient::new(&f.env, &f.soroswap).set_behavior(&MockBehavior::Panic);
    MockAquaClient::new(&f.env, &f.aquarius).set_behavior(&MockBehavior::Panic);
    set_aqua_registry(&f, &BytesN::from_array(&f.env, &[7u8; 32]));

    let result = f.router.try_swap_exact_in(
        &f.user,
        &f.token_in.address,
        &f.token_out.address,
        &AMOUNT_IN,
        &MIN_OUT,
        &Venue::SoroswapAggregator,
    );

    assert_eq!(result, Err(Ok(RouterError::AllVenuesFailed.into())));
    // Atomicite : `from` n'a rien perdu, sur AUCUN des deux tokens.
    assert_eq!(f.token_in.balance(&f.user), AMOUNT_IN);
    assert_eq!(f.token_out.balance(&f.user), 0);
    assert_eq!(f.token_in.balance(&f.router.address), 0);
    assert_eq!(
        f.router
            .pair_stats(&f.token_in.address, &f.token_out.address),
        zero_stats()
    );
}

#[test]
fn preferred_aqua_without_registry_falls_back_to_soroswap() {
    let f = swap_setup();
    MockAggregatorClient::new(&f.env, &f.soroswap).set_behavior(&MockBehavior::Serve(SERVED_OUT));
    StellarAssetClient::new(&f.env, &f.token_out.address).mint(&f.soroswap, &SERVED_OUT);
    // Registre Aqua VIDE : la venue preferee rend false en interne, sans
    // appel au mock (marqueur absent, cf. assertion finale).

    let result = f.router.swap_exact_in(
        &f.user,
        &f.token_in.address,
        &f.token_out.address,
        &AMOUNT_IN,
        &MIN_OUT,
        &Venue::AquariusRouter,
    );

    assert_eq!(
        result,
        SwapResult {
            amount_out: SERVED_OUT,
            venue: Venue::SoroswapAggregator,
            fee: SOROSWAP_FEE,
        }
    );
    assert_eq!(f.token_in.balance(&f.user), 0);
    assert_eq!(f.token_out.balance(&f.user), SERVED_OUT);
    assert_eq!(
        f.router
            .pair_stats(&f.token_in.address, &f.token_out.address)
            .fees,
        SOROSWAP_FEE
    );
    assert!(!MockAquaClient::new(&f.env, &f.aquarius).was_called());
}

// Chemin nominal Aquarius : valide de bout en bout, au niveau unitaire, la
// pre-autorisation du tirage par Aqua et le trajet i128 -> u128 -> i128.
#[test]
fn preferred_aqua_nominal_serves_with_registry() {
    let f = swap_setup();
    MockAquaClient::new(&f.env, &f.aquarius).set_behavior(&MockBehavior::Serve(SERVED_OUT));
    StellarAssetClient::new(&f.env, &f.token_out.address).mint(&f.aquarius, &SERVED_OUT);
    set_aqua_registry(&f, &BytesN::from_array(&f.env, &[7u8; 32]));

    let result = f.router.swap_exact_in(
        &f.user,
        &f.token_in.address,
        &f.token_out.address,
        &AMOUNT_IN,
        &MIN_OUT,
        &Venue::AquariusRouter,
    );

    assert_eq!(
        result,
        SwapResult {
            amount_out: SERVED_OUT,
            venue: Venue::AquariusRouter,
            fee: AQUARIUS_FEE,
        }
    );
    assert_eq!(f.token_in.balance(&f.user), 0);
    assert_eq!(f.token_out.balance(&f.user), SERVED_OUT);
    assert_eq!(f.token_in.balance(&f.router.address), 0);
    assert_eq!(f.token_out.balance(&f.router.address), 0);
    assert_eq!(
        f.router
            .pair_stats(&f.token_in.address, &f.token_out.address),
        PairStats {
            volume_in: AMOUNT_IN,
            volume_out: SERVED_OUT,
            fees: AQUARIUS_FEE,
            swaps: 1,
        }
    );
    // La preferee a servi : le secours n'a pas ete invoque.
    assert!(!MockAggregatorClient::new(&f.env, &f.soroswap).was_called());
}

// Suivi de revue Task 4 : la branche SlippageExceeded etait inatteignable
// avec Serve (le mock revert sous le min). ServeIgnoringMin incarne la venue
// MENTEUSE : elle annonce succes en servant sous min_out ; la defense en
// profondeur du routeur (jugement sur delta de solde) doit tout revert.
#[test]
fn lying_venue_serving_under_min_hits_slippage_exceeded() {
    let f = swap_setup();
    MockAggregatorClient::new(&f.env, &f.soroswap)
        .set_behavior(&MockBehavior::ServeIgnoringMin(MIN_OUT - 1));
    StellarAssetClient::new(&f.env, &f.token_out.address).mint(&f.soroswap, &(MIN_OUT - 1));

    let result = f.router.try_swap_exact_in(
        &f.user,
        &f.token_in.address,
        &f.token_out.address,
        &AMOUNT_IN,
        &MIN_OUT,
        &Venue::SoroswapAggregator,
    );

    assert_eq!(result, Err(Ok(RouterError::SlippageExceeded.into())));
    // Revert integral : les DEUX soldes de `from` sont intacts.
    assert_eq!(f.token_in.balance(&f.user), AMOUNT_IN);
    assert_eq!(f.token_out.balance(&f.user), 0);
    assert_eq!(f.token_in.balance(&f.router.address), 0);
    assert_eq!(
        f.router
            .pair_stats(&f.token_in.address, &f.token_out.address),
        zero_stats()
    );
}

#[test]
fn initialize_rejects_fee_bps_above_100_percent() {
    // La garde couvre chacune des deux venues independamment.
    for (soroswap_bps, aquarius_bps) in [(10_001_u32, AQUARIUS_FEE_BPS), (SOROSWAP_FEE_BPS, 10_001)]
    {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let soroswap = Address::generate(&env);
        let aquarius = Address::generate(&env);
        let router = SwapRouterClient::new(&env, &env.register(SwapRouter, ()));

        let result =
            router.try_initialize(&admin, &soroswap, &aquarius, &soroswap_bps, &aquarius_bps);

        assert_eq!(result, Err(Ok(RouterError::InvalidFeeBps.into())));
    }
}

#[test]
fn initialize_accepts_fee_bps_at_100_percent() {
    // Borne incluse : 10 000 bps = 100 %, valide.
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let soroswap = Address::generate(&env);
    let aquarius = Address::generate(&env);
    let router = SwapRouterClient::new(&env, &env.register(SwapRouter, ()));

    router.initialize(&admin, &soroswap, &aquarius, &10_000, &10_000);
}

/// Suivi de revue Task 3 : observation des auths enregistrees.
///
/// env.auths() ne restitue que les account trackers (require_auth satisfaits
/// par une entree d'auth d'adresse) : les pre-autorisations
/// authorize_as_current_contract vivent dans les invoker trackers, invisibles
/// ici PAR CONSTRUCTION (soroban-env-host 25, get_authenticated_authorizations
/// ne parcourt que account_trackers). L'observation probante est donc double :
/// 1) sous mock_all_auths simple (non-root interdit), une pre-autorisation
///    manquante fait ECHOUER le swap avec « make sure that you have called
///    authorize_as_current_contract() » (require_auth_recording, env-host) :
///    le happy path est deja un fil-piege ;
/// 2) l'arbre enregistre ne contient QUE l'auth de `from` : si l'adresse du
///    routeur y figurait, son transfert vers la venue aurait ete servi par
///    l'auth mockee (recording) et non par la pre-autorisation (les invoker
///    trackers sont verifies AVANT le mode recording, require_auth_internal).
#[test]
fn swap_records_only_user_auth_venue_pull_preauthorized() {
    let f = swap_setup();
    MockAggregatorClient::new(&f.env, &f.soroswap).set_behavior(&MockBehavior::Serve(SERVED_OUT));
    StellarAssetClient::new(&f.env, &f.token_out.address).mint(&f.soroswap, &SERVED_OUT);

    f.router.swap_exact_in(
        &f.user,
        &f.token_in.address,
        &f.token_out.address,
        &AMOUNT_IN,
        &MIN_OUT,
        &Venue::SoroswapAggregator,
    );

    let auths = f.env.auths();
    // Le routeur n'apparait dans AUCUNE auth enregistree : ses transferts
    // sortants sont couverts par la pre-autorisation, pas par le mock.
    assert!(auths.iter().all(|(addr, _)| addr != &f.router.address));
    // Seule auth enregistree : `from`, racine swap_exact_in, avec le
    // transfert entrant (from -> routeur) en sous-invocation.
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
                        f.token_in.address.clone(),
                        f.token_out.address.clone(),
                        AMOUNT_IN,
                        MIN_OUT,
                        Venue::SoroswapAggregator,
                    )
                        .into_val(&f.env),
                )),
                sub_invocations: std::vec![AuthorizedInvocation {
                    function: AuthorizedFunction::Contract((
                        f.token_in.address.clone(),
                        Symbol::new(&f.env, "transfer"),
                        (f.user.clone(), f.router.address.clone(), AMOUNT_IN).into_val(&f.env),
                    )),
                    sub_invocations: std::vec![],
                }],
            }
        )]
    );
}

#[test]
fn pair_stats_defaults_to_zeros() {
    let f = setup();
    let token_in = Address::generate(&f.env);
    let token_out = Address::generate(&f.env);

    assert_eq!(f.router.pair_stats(&token_in, &token_out), zero_stats());
}

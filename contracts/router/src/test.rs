#![cfg(test)]
extern crate std;

use super::test_mocks::{
    MockAggregator, MockAggregatorClient, MockAqua, MockAquaClient, MockBehavior,
};
use super::{
    venues, AquaPoolSetEvent, PairStats, RouterError, SwapEvent, SwapResult, SwapRouter,
    SwapRouterClient, Venue,
};
use soroban_sdk::{
    testutils::{
        Address as _, AuthorizedFunction, AuthorizedInvocation, Events as _, MockAuth,
        MockAuthInvoke,
    },
    token::{StellarAssetClient, TokenClient},
    Address, BytesN, Env, Event as _, IntoVal, Symbol,
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

/// Pool hash Aqua arbitraire des tests : la valeur n'a aucun sens on-chain,
/// seuls comptent sa presence (registre alimente) et son transport.
fn pool_hash(env: &Env) -> BytesN<32> {
    BytesN::from_array(env, &[7u8; 32])
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
    let pool_hash = pool_hash(&f.env);

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
    let pool_hash = pool_hash(&f.env);

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
    let pool_hash = pool_hash(&f.env);

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
    // marqueur en storage a cette limite. La preuve directe de la garde vit
    // dans venues::convert (helpers purs testes aux bornes, sans contrat).
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

/// Alimente le registre Aqua par le SETTER PUBLIC (auth admin mockee par la
/// fixture) : les tests de la matrice traversent la meme surface que les ops.
fn set_aqua_registry(f: &SwapFixture, pool_hash: &BytesN<32>) {
    f.router
        .set_aqua_pool(&f.token_in.address, &f.token_out.address, pool_hash);
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
    set_aqua_registry(&f, &pool_hash(&f.env));

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
    set_aqua_registry(&f, &pool_hash(&f.env));

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
            .pair_stats(&f.token_in.address, &f.token_out.address),
        PairStats {
            volume_in: AMOUNT_IN,
            volume_out: SERVED_OUT,
            fees: AQUARIUS_FEE,
            swaps: 1,
        }
    );
}

// Complement du test Task 4 (Panic + registre vide) : ici les DEUX venues
// sont presentes et executent, et les deux paniquent.
#[test]
fn both_venues_present_and_panicking_reverts_funds() {
    let f = swap_setup();
    MockAggregatorClient::new(&f.env, &f.soroswap).set_behavior(&MockBehavior::Panic);
    MockAquaClient::new(&f.env, &f.aquarius).set_behavior(&MockBehavior::Panic);
    set_aqua_registry(&f, &pool_hash(&f.env));

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
            .pair_stats(&f.token_in.address, &f.token_out.address),
        PairStats {
            volume_in: AMOUNT_IN,
            volume_out: SERVED_OUT,
            fees: SOROSWAP_FEE,
            swaps: 1,
        }
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
    set_aqua_registry(&f, &pool_hash(&f.env));

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

// Borne EXACTE du jugement min-out : servir exactement min_out est un succes
// (received >= min_out, inclusif). Fige la frontiere entre swap servi et
// SlippageExceeded : une mutation >= -> > tue ce test.
#[test]
fn swap_serving_exactly_min_out_succeeds() {
    let f = swap_setup();
    MockAggregatorClient::new(&f.env, &f.soroswap).set_behavior(&MockBehavior::Serve(MIN_OUT));
    StellarAssetClient::new(&f.env, &f.token_out.address).mint(&f.soroswap, &MIN_OUT);

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
            amount_out: MIN_OUT,
            venue: Venue::SoroswapAggregator,
            fee: SOROSWAP_FEE,
        }
    );
    assert_eq!(f.token_in.balance(&f.user), 0);
    assert_eq!(f.token_out.balance(&f.user), MIN_OUT);
    assert_eq!(
        f.router
            .pair_stats(&f.token_in.address, &f.token_out.address),
        PairStats {
            volume_in: AMOUNT_IN,
            volume_out: MIN_OUT,
            fees: SOROSWAP_FEE,
            swaps: 1,
        }
    );
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

// Temoin de l'invariant AllVenuesFailed (suivi de revue Task 5) : une venue
// qui EXECUTE reellement (tire token_in du routeur, sert token_out) puis
// retourne un montant inconvertible (> i128::MAX) rend attempt false AVANT
// tout jugement de delta (aqua.rs : Ok(Ok(out)) inconvertible -> false).
// Le solde token_in du routeur etant vide apres le tirage d'Aqua, le tirage
// de la venue de secours echoue -> AllVenuesFailed -> revert INTEGRAL : les
// fonds sont proteges par l'atomicite de la transaction, pas par le
// jugement local (lib.rs, commentaire d'invariant).
#[test]
fn venue_executing_but_returning_inconvertible_reverts_all() {
    let f = swap_setup();
    MockAquaClient::new(&f.env, &f.aquarius)
        .set_behavior(&MockBehavior::ServeReturningHuge(SERVED_OUT));
    StellarAssetClient::new(&f.env, &f.token_out.address).mint(&f.aquarius, &SERVED_OUT);
    // Soroswap PRETE a servir : si le routeur jugeait le delta d'Aqua
    // (SERVED_OUT >= MIN_OUT) ou si Soroswap pouvait tirer, le swap
    // reussirait ; AllVenuesFailed prouve donc les deux mecanismes.
    MockAggregatorClient::new(&f.env, &f.soroswap).set_behavior(&MockBehavior::Serve(SERVED_OUT));
    StellarAssetClient::new(&f.env, &f.token_out.address).mint(&f.soroswap, &SERVED_OUT);
    set_aqua_registry(&f, &pool_hash(&f.env));

    let result = f.router.try_swap_exact_in(
        &f.user,
        &f.token_in.address,
        &f.token_out.address,
        &AMOUNT_IN,
        &MIN_OUT,
        &Venue::AquariusRouter,
    );

    assert_eq!(result, Err(Ok(RouterError::AllVenuesFailed.into())));
    // Revert integral : `from` intact sur les deux tokens, routeur vide,
    // les mocks retrouvent leur pre-financement.
    assert_eq!(f.token_in.balance(&f.user), AMOUNT_IN);
    assert_eq!(f.token_out.balance(&f.user), 0);
    assert_eq!(f.token_in.balance(&f.router.address), 0);
    assert_eq!(f.token_out.balance(&f.router.address), 0);
    assert_eq!(f.token_in.balance(&f.aquarius), 0);
    assert_eq!(f.token_out.balance(&f.aquarius), SERVED_OUT);
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

// --- Registre admin des pools Aqua (Task 6) : setter admin-only, getter
// ops/demo (PR C verifie l'etat du registre par ce getter).

#[test]
fn set_aqua_pool_rejects_non_admin() {
    // Pas de mock_all_auths : seule l'auth du NON-admin est mockee, le
    // require_auth de l'admin doit donc echouer (initialize ne requiert
    // aucune auth, aucun mock necessaire avant).
    let env = Env::default();
    let admin = Address::generate(&env);
    let non_admin = Address::generate(&env);
    let token_a = Address::generate(&env);
    let token_b = Address::generate(&env);
    let router = SwapRouterClient::new(&env, &env.register(SwapRouter, ()));
    router.initialize(
        &admin,
        &Address::generate(&env),
        &Address::generate(&env),
        &SOROSWAP_FEE_BPS,
        &AQUARIUS_FEE_BPS,
    );
    let hash = pool_hash(&env);

    env.mock_auths(&[MockAuth {
        address: &non_admin,
        invoke: &MockAuthInvoke {
            contract: &router.address,
            fn_name: "set_aqua_pool",
            args: (token_a.clone(), token_b.clone(), hash.clone()).into_val(&env),
            sub_invokes: &[],
        },
    }]);
    let result = router.try_set_aqua_pool(&token_a, &token_b, &hash);

    // Echec d'auth = erreur HOTE (pas un code RouterError) : l'admin n'a
    // pas signe. Si le require_auth disparaissait, l'appel reussirait et ce
    // test echouerait.
    assert!(result.is_err());
    assert_eq!(router.aqua_pool_of(&token_a, &token_b), None);
}

#[test]
fn set_aqua_pool_stores_sorted_pair_and_getter_reads_both_orders() {
    let f = setup();
    let token_a = Address::generate(&f.env);
    let token_b = Address::generate(&f.env);
    let hash_1 = BytesN::from_array(&f.env, &[1u8; 32]);
    let hash_2 = BytesN::from_array(&f.env, &[2u8; 32]);

    assert_eq!(f.router.aqua_pool_of(&token_a, &token_b), None);

    f.router.set_aqua_pool(&token_a, &token_b, &hash_1);
    // Un pool sert les deux sens : le getter repond quel que soit l'ordre.
    assert_eq!(
        f.router.aqua_pool_of(&token_a, &token_b),
        Some(hash_1.clone())
    );
    assert_eq!(f.router.aqua_pool_of(&token_b, &token_a), Some(hash_1));

    // Ecriture dans l'ordre INVERSE : meme cle triee, l'entree est
    // remplacee, pas dupliquee.
    f.router.set_aqua_pool(&token_b, &token_a, &hash_2);
    assert_eq!(f.router.aqua_pool_of(&token_a, &token_b), Some(hash_2));
}

#[test]
fn pair_stats_defaults_to_zeros() {
    let f = setup();
    let token_in = Address::generate(&f.env);
    let token_out = Address::generate(&f.env);

    assert_eq!(f.router.pair_stats(&token_in, &token_out), zero_stats());
}

// --- Events #[contractevent] (Task 7) : schema D6a, venue EFFECTIVE.
//
// Semantique de env.events().all() VERIFIEE dans soroban-sdk 25.3.2 : le test
// Env active l'invocation metering (sdk env.rs, new_for_testutils), qui VIDE
// le buffer d'events du host a chaque invocation racine (soroban-env-host
// 25.2.2, invocation_metering.rs, push_invocation a stack_depth 0) ; all()
// filtre en outre les events des sous-appels echoues (!failed_call). Donc :
// all() = events de la DERNIERE invocation racine uniquement -> asserter
// immediatement apres l'appel sous test, comme le fait deja le vault.

#[test]
fn swap_emits_event_with_full_schema_and_effective_venue() {
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

    // Comparaison XDR complete (topics ET data) via Event::to_xdr : le seul
    // event du routeur dans l'invocation est `swap` (les transferts token
    // sont emis par les contrats token, filtres par filter_by_contract).
    assert_eq!(
        f.env.events().all().filter_by_contract(&f.router.address),
        [SwapEvent {
            from: f.user.clone(),
            token_in: f.token_in.address.clone(),
            token_out: f.token_out.address.clone(),
            amount_in: AMOUNT_IN,
            amount_out: SERVED_OUT,
            venue: Venue::SoroswapAggregator,
            fee: SOROSWAP_FEE,
            min_out: MIN_OUT,
        }
        .to_xdr(&f.env, &f.router.address)]
    );
}

// Cas fallback : la preferee panique, le secours sert -> l'event porte la
// venue EFFECTIVE (AquariusRouter) et le fee au bareme de la venue qui a
// servi, meme exigence que SwapResult et les stats.
#[test]
fn swap_event_carries_effective_venue_on_fallback() {
    let f = swap_setup();
    MockAggregatorClient::new(&f.env, &f.soroswap).set_behavior(&MockBehavior::Panic);
    MockAquaClient::new(&f.env, &f.aquarius).set_behavior(&MockBehavior::Serve(SERVED_OUT));
    StellarAssetClient::new(&f.env, &f.token_out.address).mint(&f.aquarius, &SERVED_OUT);
    set_aqua_registry(&f, &pool_hash(&f.env));

    f.router.swap_exact_in(
        &f.user,
        &f.token_in.address,
        &f.token_out.address,
        &AMOUNT_IN,
        &MIN_OUT,
        &Venue::SoroswapAggregator,
    );

    assert_eq!(
        f.env.events().all().filter_by_contract(&f.router.address),
        [SwapEvent {
            from: f.user.clone(),
            token_in: f.token_in.address.clone(),
            token_out: f.token_out.address.clone(),
            amount_in: AMOUNT_IN,
            amount_out: SERVED_OUT,
            venue: Venue::AquariusRouter,
            fee: AQUARIUS_FEE,
            min_out: MIN_OUT,
        }
        .to_xdr(&f.env, &f.router.address)]
    );
}

// Suivi de revue Task 6 : changement de config admin auditable on-chain
// (posture D6a). L'event porte la paire TRIEE (l'identite de la cle de
// registre), quelle que soit l'ordre des arguments passes au setter.
#[test]
fn set_aqua_pool_emits_event_with_sorted_pair() {
    let f = setup();
    let token_a = Address::generate(&f.env);
    let token_b = Address::generate(&f.env);
    let (lo, hi) = if token_a < token_b {
        (token_a, token_b)
    } else {
        (token_b, token_a)
    };
    let hash = pool_hash(&f.env);

    // Arguments volontairement dans l'ordre INVERSE du tri : l'event doit
    // porter la paire triee, pas la paire passee.
    f.router.set_aqua_pool(&hi, &lo, &hash);

    assert_eq!(
        f.env.events().all().filter_by_contract(&f.router.address),
        [AquaPoolSetEvent {
            token_a: lo,
            token_b: hi,
            pool_hash: hash,
        }
        .to_xdr(&f.env, &f.router.address)]
    );
}

// Suivi de revue Task 6 : paire degeneree (token_a == token_b) rejetee en
// erreur typee -- sans cette garde, l'entree n'aurait aucune voie de
// suppression (pas de deleter en D4). Aucun event sur rejet.
#[test]
fn set_aqua_pool_rejects_same_token_without_event() {
    let f = setup();
    let token = Address::generate(&f.env);
    let hash = pool_hash(&f.env);

    let result = f.router.try_set_aqua_pool(&token, &token, &hash);

    assert_eq!(result, Err(Ok(RouterError::SameToken.into())));
    // Absence d'event : l'invocation a echoue, all() ne restitue rien d'elle
    // (events des appels echoues filtres) -- et le registre est intact.
    assert!(f
        .env
        .events()
        .all()
        .filter_by_contract(&f.router.address)
        .events()
        .is_empty());
    assert_eq!(f.router.aqua_pool_of(&token, &token), None);
}

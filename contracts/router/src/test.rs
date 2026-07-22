#![cfg(test)]
use super::test_mocks::{
    MockAggregator, MockAggregatorClient, MockAqua, MockAquaClient, MockBehavior,
};
use super::{venues, PairStats, RouterError, SwapRouter, SwapRouterClient, Venue};
use soroban_sdk::{
    testutils::Address as _,
    token::{StellarAssetClient, TokenClient},
    Address, BytesN, Env,
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

#[test]
fn pair_stats_defaults_to_zeros() {
    let f = setup();
    let token_in = Address::generate(&f.env);
    let token_out = Address::generate(&f.env);

    assert_eq!(
        f.router.pair_stats(&token_in, &token_out),
        PairStats {
            volume_in: 0,
            volume_out: 0,
            fees: 0,
            swaps: 0,
        }
    );
}

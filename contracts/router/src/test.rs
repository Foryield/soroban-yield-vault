#![cfg(test)]
use super::{PairStats, RouterError, SwapRouter, SwapRouterClient, Venue};
use soroban_sdk::{testutils::Address as _, Address, Env};

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

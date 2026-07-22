#![cfg(test)]
//! Proprietes (proptest) : invariants du routeur sur des sequences de swaps
//! mockes. 256 cas generes par propriete a chaque execution (config par
//! defaut, meme reglage que le vault), sequences bornees a 5 swaps.
//!
//! Invariants verifies apres CHAQUE appel (servi ou echoue) :
//! - solde du routeur NUL dans les deux tokens (rien ne reste hors
//!   transaction, succes comme revert) ;
//! - stats de la paire = somme EXACTE des swaps SERVIS, recalculee par le
//!   modele du test (volume_in, volume_out, fees, count) ;
//! - issue de chaque appel conforme au modele : venue effective et montant
//!   sur succes, erreur TYPEE predite (slippage vs panne de venue) sur echec.

use super::test_mocks::{
    MockAggregator, MockAggregatorClient, MockAqua, MockAquaClient, MockBehavior,
};
use super::{PairStats, RouterError, SwapRouter, SwapRouterClient, Venue};
use proptest::prelude::*;
use soroban_sdk::{
    testutils::{Address as _, EnvTestConfig},
    token::{StellarAssetClient, TokenClient},
    Address, BytesN, Env,
};

const SOROSWAP_FEE_BPS: u32 = 30;
const AQUARIUS_FEE_BPS: u32 = 10;

/// Comportement d'une venue mockee, vu du modele.
#[derive(Clone, Copy, Debug)]
enum VenueMode {
    /// Sert min_out + delta : la venue peut servir le swap.
    Serve,
    /// Panique : attempt rend false, le fallback traverse.
    Panic,
    /// Sert sous le minimum : le mock revert lui-meme (venue reelle),
    /// attempt rend false, le fallback traverse.
    UnderMin,
    /// Piege : la venue EXECUTE puis fait echouer le swap entier.
    /// Soroswap -> ServeIgnoringMin (venue menteuse, SlippageExceeded) ;
    /// Aqua -> ServeReturningHuge (retour inconvertible, AllVenuesFailed).
    /// Dans les deux cas : atteinte = revert integral, stats intactes.
    Trap,
}

/// Un swap de la sequence generee.
#[derive(Clone, Debug)]
struct Op {
    amount_in: i128,
    min_out: i128,
    /// Montant servi par une venue en mode Serve : min_out + delta.
    soro_delta: i128,
    aqua_delta: i128,
    soro_mode: VenueMode,
    aqua_mode: VenueMode,
    prefer_soroswap: bool,
}

impl Op {
    fn serve_amount(&self, venue: Venue) -> i128 {
        match venue {
            Venue::SoroswapAggregator => self.min_out + self.soro_delta,
            Venue::AquariusRouter => self.min_out + self.aqua_delta,
        }
    }

    fn mode(&self, venue: Venue) -> VenueMode {
        match venue {
            Venue::SoroswapAggregator => self.soro_mode,
            Venue::AquariusRouter => self.aqua_mode,
        }
    }

    fn preferred(&self) -> Venue {
        if self.prefer_soroswap {
            Venue::SoroswapAggregator
        } else {
            Venue::AquariusRouter
        }
    }

    /// Comportement concret a configurer sur le mock de la venue.
    fn behavior(&self, venue: Venue) -> MockBehavior {
        let serve = self.serve_amount(venue);
        match self.mode(venue) {
            VenueMode::Serve => MockBehavior::Serve(serve),
            VenueMode::Panic => MockBehavior::Panic,
            VenueMode::UnderMin => MockBehavior::Serve(self.min_out - 1),
            VenueMode::Trap => match venue {
                Venue::SoroswapAggregator => MockBehavior::ServeIgnoringMin(self.min_out - 1),
                Venue::AquariusRouter => MockBehavior::ServeReturningHuge(serve),
            },
        }
    }

    /// Modele du routeur : venue effective et montant servi, ou l'erreur
    /// typee attendue si le swap entier doit echouer (revert integral).
    fn expected_outcome(&self, registry_set: bool) -> Result<(Venue, i128), RouterError> {
        let order = match self.preferred() {
            Venue::SoroswapAggregator => [Venue::SoroswapAggregator, Venue::AquariusRouter],
            Venue::AquariusRouter => [Venue::AquariusRouter, Venue::SoroswapAggregator],
        };
        for venue in order {
            // Registre Aqua vide : attempt rend false AVANT d'invoquer la
            // venue, le fallback traverse quel que soit son mode.
            if venue == Venue::AquariusRouter && !registry_set {
                continue;
            }
            match self.mode(venue) {
                VenueMode::Serve => return Ok((venue, self.serve_amount(venue))),
                VenueMode::Panic | VenueMode::UnderMin => continue,
                // Le piege arrete le swap entier, avec l'erreur typee propre
                // a son mecanisme :
                // - Soroswap (ServeIgnoringMin) : attempt rend true, le
                //   routeur juge delta = min_out - 1 < min_out et panique
                //   SlippageExceeded sans essayer le secours ;
                // - Aqua (ServeReturningHuge) : la venue EXECUTE (tire
                //   token_in du routeur) puis attempt rend false ; le
                //   secours ne peut plus tirer (solde routeur vide) ou la
                //   boucle est epuisee -> AllVenuesFailed dans les deux cas.
                VenueMode::Trap => {
                    return Err(match venue {
                        Venue::SoroswapAggregator => RouterError::SlippageExceeded,
                        Venue::AquariusRouter => RouterError::AllVenuesFailed,
                    })
                }
            }
        }
        Err(RouterError::AllVenuesFailed)
    }
}

fn venue_mode() -> impl Strategy<Value = VenueMode> {
    // Serve surpondere : les sequences doivent servir souvent pour exercer
    // l'accumulation des stats, pas seulement les reverts.
    prop_oneof![
        3 => Just(VenueMode::Serve),
        1 => Just(VenueMode::Panic),
        1 => Just(VenueMode::UnderMin),
        1 => Just(VenueMode::Trap),
    ]
}

fn op() -> impl Strategy<Value = Op> {
    (
        1i128..1_000_000_000,
        1i128..1_000_000_000,
        0i128..1_000,
        0i128..1_000,
        venue_mode(),
        venue_mode(),
        any::<bool>(),
    )
        .prop_map(
            |(
                amount_in,
                min_out,
                soro_delta,
                aqua_delta,
                soro_mode,
                aqua_mode,
                prefer_soroswap,
            )| {
                Op {
                    amount_in,
                    min_out,
                    soro_delta,
                    aqua_delta,
                    soro_mode,
                    aqua_mode,
                    prefer_soroswap,
                }
            },
        )
}

fn fee_bps(venue: Venue) -> i128 {
    match venue {
        Venue::SoroswapAggregator => i128::from(SOROSWAP_FEE_BPS),
        Venue::AquariusRouter => i128::from(AQUARIUS_FEE_BPS),
    }
}

struct Bench<'a> {
    env: Env,
    user: Address,
    soroswap: Address,
    aquarius: Address,
    router: SwapRouterClient<'a>,
    token_in: TokenClient<'a>,
    token_out: TokenClient<'a>,
}

/// Routeur branche sur les mocks de venues, deux tokens reels. Pas de
/// snapshot par cas : proptest rejouerait 256 ecritures par test.
fn bench<'a>() -> Bench<'a> {
    let env = Env::new_with_config(EnvTestConfig {
        capture_snapshot_at_drop: false,
    });
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

    Bench {
        env: env.clone(),
        user,
        soroswap,
        aquarius,
        router,
        token_in,
        token_out,
    }
}

proptest! {
    /// Pour toute sequence de swaps mockes : solde du routeur nul dans les
    /// deux tokens apres chaque appel, stats = somme exacte des swaps servis.
    #[test]
    fn prop_router_balance_zero_and_stats_exact_sum(
        registry_set in any::<bool>(),
        ops in prop::collection::vec(op(), 1..=5),
    ) {
        let b = bench();
        if registry_set {
            b.router.set_aqua_pool(
                &b.token_in.address,
                &b.token_out.address,
                &BytesN::from_array(&b.env, &[7u8; 32]),
            );
        }

        let mut expected = PairStats { volume_in: 0, volume_out: 0, fees: 0, swaps: 0 };
        for op in &ops {
            // Financement par swap : le user recoit amount_in, chaque mock
            // recoit de quoi servir son montant maximal (les reliquats des
            // swaps reverts restent chez les mocks, sans effet sur le
            // routeur ni sur les stats).
            StellarAssetClient::new(&b.env, &b.token_in.address).mint(&b.user, &op.amount_in);
            StellarAssetClient::new(&b.env, &b.token_out.address)
                .mint(&b.soroswap, &op.serve_amount(Venue::SoroswapAggregator));
            StellarAssetClient::new(&b.env, &b.token_out.address)
                .mint(&b.aquarius, &op.serve_amount(Venue::AquariusRouter));
            MockAggregatorClient::new(&b.env, &b.soroswap)
                .set_behavior(&op.behavior(Venue::SoroswapAggregator));
            MockAquaClient::new(&b.env, &b.aquarius)
                .set_behavior(&op.behavior(Venue::AquariusRouter));

            let result = b.router.try_swap_exact_in(
                &b.user,
                &b.token_in.address,
                &b.token_out.address,
                &op.amount_in,
                &op.min_out,
                &op.preferred(),
            );

            match op.expected_outcome(registry_set) {
                Ok((venue, amount_out)) => {
                    let served = result.expect("swap modele servi").expect("conversion");
                    prop_assert_eq!(served.venue, venue);
                    prop_assert_eq!(served.amount_out, amount_out);
                    expected.volume_in += op.amount_in;
                    expected.volume_out += amount_out;
                    expected.fees += op.amount_in * fee_bps(venue) / 10_000;
                    expected.swaps += 1;
                }
                // Erreur TYPEE assertee, pas un simple is_err : le modele
                // predit aussi le code d'echec (slippage vs panne de venue).
                Err(expected_err) => prop_assert_eq!(result, Err(Ok(expected_err.into()))),
            }

            // Invariant 1 : solde du routeur NUL dans les deux tokens.
            prop_assert_eq!(b.token_in.balance(&b.router.address), 0);
            prop_assert_eq!(b.token_out.balance(&b.router.address), 0);
            // Invariant 2 : stats = somme exacte des swaps servis.
            prop_assert_eq!(
                b.router.pair_stats(&b.token_in.address, &b.token_out.address),
                expected.clone()
            );
        }
    }
}

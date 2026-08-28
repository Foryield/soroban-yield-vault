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
//! - issue de chaque appel conforme au modele : montant sur succes, erreur
//!   TYPEE predite sur echec (registre vide, slippage, panne de venue).
//!
//! Le modele a perdu sa dimension de choix de venue le 28/08/2026 avec le
//! passage mono-venue ; il a gagne en contrepartie la distinction des trois
//! erreurs typees, que l'ancienne architecture confondait partiellement dans
//! le fallback.

use super::test_mocks::{MockAqua, MockAquaClient, MockBehavior};
use super::{PairStats, RouterError, SwapRouter, SwapRouterClient};
use proptest::prelude::*;
use soroban_sdk::{
    testutils::{Address as _, EnvTestConfig},
    token::{StellarAssetClient, TokenClient},
    Address, BytesN, Env,
};

const AQUARIUS_FEE_BPS: u32 = 10;

/// Comportement de la venue mockee, vu du modele.
#[derive(Clone, Copy, Debug)]
enum VenueMode {
    /// Sert min_out + delta : la venue sert le swap.
    Serve,
    /// Panique : attempt rend false -> VenueFailed.
    Panic,
    /// Sert sous le minimum : le mock revert lui-meme (venue reelle),
    /// attempt rend false -> VenueFailed.
    UnderMin,
    /// Venue MENTEUSE : attempt rend true, mais le delta de solde est sous
    /// min_out -> SlippageExceeded (defense en profondeur du routeur).
    TrapLying,
    /// Venue qui EXECUTE puis retourne un montant inconvertible : attempt
    /// rend false APRES coup -> VenueFailed, revert integral.
    TrapHuge,
}

/// Un swap de la sequence generee.
#[derive(Clone, Debug)]
struct Op {
    amount_in: i128,
    min_out: i128,
    /// Montant servi par la venue en mode Serve : min_out + delta.
    delta: i128,
    mode: VenueMode,
}

impl Op {
    fn serve_amount(&self) -> i128 {
        self.min_out + self.delta
    }

    /// Comportement concret a configurer sur le mock de la venue.
    fn behavior(&self) -> MockBehavior {
        match self.mode {
            VenueMode::Serve => MockBehavior::Serve(self.serve_amount()),
            VenueMode::Panic => MockBehavior::Panic,
            VenueMode::UnderMin => MockBehavior::Serve(self.min_out - 1),
            VenueMode::TrapLying => MockBehavior::ServeIgnoringMin(self.min_out - 1),
            VenueMode::TrapHuge => MockBehavior::ServeReturningHuge(self.serve_amount()),
        }
    }

    /// Modele du routeur : montant servi, ou l'erreur typee attendue si le
    /// swap entier doit echouer (revert integral).
    fn expected_outcome(&self, registry_set: bool) -> Result<i128, RouterError> {
        // Registre vide : le routeur panique AVANT d'invoquer la venue, quel
        // que soit son mode.
        if !registry_set {
            return Err(RouterError::AquaPoolNotSet);
        }
        match self.mode {
            VenueMode::Serve => Ok(self.serve_amount()),
            VenueMode::Panic | VenueMode::UnderMin => Err(RouterError::VenueFailed),
            // La venue annonce succes en servant sous min_out : le routeur
            // juge sur delta de solde et panique.
            VenueMode::TrapLying => Err(RouterError::SlippageExceeded),
            // La venue EXECUTE (tire token_in du routeur) puis rend un
            // montant inconvertible : attempt rend false, le routeur panique
            // sans regarder son solde, l'atomicite restitue les fonds.
            VenueMode::TrapHuge => Err(RouterError::VenueFailed),
        }
    }
}

fn venue_mode() -> impl Strategy<Value = VenueMode> {
    // Serve surpondere : les sequences doivent servir souvent pour exercer
    // l'accumulation des stats, pas seulement les reverts.
    prop_oneof![
        4 => Just(VenueMode::Serve),
        1 => Just(VenueMode::Panic),
        1 => Just(VenueMode::UnderMin),
        1 => Just(VenueMode::TrapLying),
        1 => Just(VenueMode::TrapHuge),
    ]
}

fn op() -> impl Strategy<Value = Op> {
    (
        1i128..1_000_000_000,
        1i128..1_000_000_000,
        0i128..1_000,
        venue_mode(),
    )
        .prop_map(|(amount_in, min_out, delta, mode)| Op {
            amount_in,
            min_out,
            delta,
            mode,
        })
}

struct Bench<'a> {
    env: Env,
    user: Address,
    aquarius: Address,
    router: SwapRouterClient<'a>,
    token_in: TokenClient<'a>,
    token_out: TokenClient<'a>,
}

/// Routeur branche sur le mock de venue, deux tokens reels. Pas de snapshot
/// par cas : proptest rejouerait 256 ecritures par test.
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

    let aquarius = env.register(MockAqua, ());
    let router = SwapRouterClient::new(&env, &env.register(SwapRouter, ()));
    router.initialize(&admin, &aquarius, &AQUARIUS_FEE_BPS);

    Bench {
        env: env.clone(),
        user,
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
            // Financement par swap : le user recoit amount_in, le mock recoit
            // de quoi servir son montant maximal (les reliquats des swaps
            // reverts restent chez le mock, sans effet sur le routeur ni sur
            // les stats).
            StellarAssetClient::new(&b.env, &b.token_in.address).mint(&b.user, &op.amount_in);
            StellarAssetClient::new(&b.env, &b.token_out.address)
                .mint(&b.aquarius, &op.serve_amount());
            MockAquaClient::new(&b.env, &b.aquarius).set_behavior(&op.behavior());

            let result = b.router.try_swap_exact_in(
                &b.user,
                &b.token_in.address,
                &b.token_out.address,
                &op.amount_in,
                &op.min_out,
            );

            match op.expected_outcome(registry_set) {
                Ok(amount_out) => {
                    let served = result.expect("swap modele servi").expect("conversion");
                    prop_assert_eq!(served.amount_out, amount_out);
                    expected.volume_in += op.amount_in;
                    expected.volume_out += amount_out;
                    expected.fees += op.amount_in * i128::from(AQUARIUS_FEE_BPS) / 10_000;
                    expected.swaps += 1;
                }
                // Erreur TYPEE assertee, pas un simple is_err : le modele
                // predit aussi le code d'echec (registre vide, slippage,
                // panne de venue).
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

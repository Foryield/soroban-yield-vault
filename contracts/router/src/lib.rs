#![no_std]
//! ForYield Soroban SwapRouter (Deliverable D4).
//!
//! - best-execution USDC<->EURC : la cotation et la selection de venue
//!   (`preferred`) se font off-chain ; le contrat garantit ce que seul
//!   l'on-chain garantit : min-out et fallback atomique dans la meme
//!   transaction (soit une venue sert au moins min_out, soit tout revert) ;
//! - venues (aggregator Soroswap, router Aquarius) fixees a l'initialize,
//!   immuables : changement de venue = redeploiement (meme convention que le
//!   pool du vault D1) ;
//! - registre admin des pools Aquarius (pool_hash par paire) : le hash change
//!   a chaque re-seed testnet, un setter admin evite un redeploiement pour un
//!   simple identifiant de pool ; sans entree, la venue Aquarius echoue en
//!   `AquaPoolNotSet` et le fallback la traverse ;
//! - invariant : solde du routeur nul hors transaction (le produit du swap
//!   est integralement reverse a l'appelant dans la meme invocation).
//!
//! Hors scope D4 : multi-hop (pas de `path` expose), setters de venues,
//! frais preleves par le routeur (fee_bps = comptabilite, pas prelevement).

use soroban_sdk::{
    contract, contracterror, contractimpl, contractmeta, contracttype, panic_with_error, Address,
    Env,
};

/// Erreurs typees du routeur : contractuelles pour les integrateurs (un
/// client off-chain teste un code, pas une chaine de panique). Les erreurs
/// de garde restent distinctes de `AllVenuesFailed` (le client distingue
/// slippage et panne de venue).
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum RouterError {
    AlreadyInitialized = 1,
    AmountMustBePositive = 2,
    MinOutMustBePositive = 3,
    SameToken = 4,
    AquaPoolNotSet = 5,
    AllVenuesFailed = 6,
    SlippageExceeded = 7,
    AmountConversion = 8,
    MathOverflow = 9,
}

contractmeta!(
    key = "desc",
    val = "ForYield SwapRouter - best-execution, min-out, fallback atomique"
);

/// Venue d'execution. `preferred` cote client, venue EFFECTIVE dans
/// `SwapResult` (celle qui a servi apres fallback).
#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Venue {
    SoroswapAggregator = 0,
    AquariusRouter = 1,
}

/// Resultat d'un swap servi : montant sorti, venue effective, frais
/// comptabilises (amount_in x fee_bps de la venue / 10 000).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SwapResult {
    pub amount_out: i128,
    pub venue: Venue,
    pub fee: i128,
}

/// Accumulateurs par paire ordonnee (token_in, token_out) : matiere premiere
/// du dashboard D6c, sans indexeur.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PairStats {
    pub volume_in: i128,
    pub volume_out: i128,
    pub fees: i128,
    pub swaps: u64,
}

#[contracttype]
#[derive(Clone)]
enum DataKey {
    Admin,
    SoroswapAggregator,
    AquariusRouter,
    SoroswapFeeBps,
    AquariusFeeBps,
    /// Cle = paire TRIEE par adresse (un pool Aqua sert les deux sens).
    AquaPool(Address, Address),
    /// Cle = paire ORDONNEE (token_in, token_out) telle que swappee :
    /// le sens du flux compte.
    Stats(Address, Address),
}

#[contract]
pub struct SwapRouter;

#[contractimpl]
impl SwapRouter {
    /// Initialise le routeur. Idempotence interdite : un second appel panique.
    /// Les venues et leurs fee_bps sont immuables (pas de setter en D4) :
    /// changement de venue = redeploiement.
    pub fn initialize(
        env: Env,
        admin: Address,
        soroswap_aggregator: Address,
        aquarius_router: Address,
        soroswap_fee_bps: u32,
        aquarius_fee_bps: u32,
    ) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic_with_error!(&env, RouterError::AlreadyInitialized);
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::SoroswapAggregator, &soroswap_aggregator);
        env.storage()
            .instance()
            .set(&DataKey::AquariusRouter, &aquarius_router);
        env.storage()
            .instance()
            .set(&DataKey::SoroswapFeeBps, &soroswap_fee_bps);
        env.storage()
            .instance()
            .set(&DataKey::AquariusFeeBps, &aquarius_fee_bps);
    }

    /// Statistiques cumulees de la paire ORDONNEE (token_in, token_out) telle
    /// que swappee : le sens du flux compte, un aller-retour alimente deux
    /// entrees distinctes. Zeros tant qu'aucun swap n'a ete servi.
    pub fn pair_stats(env: Env, token_in: Address, token_out: Address) -> PairStats {
        env.storage()
            .persistent()
            .get(&DataKey::Stats(token_in, token_out))
            .unwrap_or(PairStats {
                volume_in: 0,
                volume_out: 0,
                fees: 0,
                swaps: 0,
            })
    }

    // Les trois getters prives sont consommes par swap_exact_in (suite de la
    // PR A) et deja exerces par les tests ; les allow(dead_code) sautent des
    // que swap_exact_in existe.
    #[allow(dead_code)]
    fn admin(env: &Env) -> Address {
        env.storage().instance().get(&DataKey::Admin).unwrap()
    }

    #[allow(dead_code)]
    fn venue_addr(env: &Env, venue: Venue) -> Address {
        let key = match venue {
            Venue::SoroswapAggregator => DataKey::SoroswapAggregator,
            Venue::AquariusRouter => DataKey::AquariusRouter,
        };
        env.storage().instance().get(&key).unwrap()
    }

    #[allow(dead_code)]
    fn fee_bps(env: &Env, venue: Venue) -> u32 {
        let key = match venue {
            Venue::SoroswapAggregator => DataKey::SoroswapFeeBps,
            Venue::AquariusRouter => DataKey::AquariusFeeBps,
        };
        env.storage().instance().get(&key).unwrap()
    }
}

// Consomme a partir de la Task 4 (swap_exact_in) : d'ici la, seul le code de
// test exerce le module, d'ou l'allow(dead_code) sur le build non-test.
#[allow(dead_code)]
mod venues;

#[cfg(test)]
mod test;
#[cfg(test)]
mod test_mocks;

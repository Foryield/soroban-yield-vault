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

use soroban_sdk::{contract, contracterror, contractmeta, contracttype, Address};

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

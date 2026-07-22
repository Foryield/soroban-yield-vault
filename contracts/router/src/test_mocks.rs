#![cfg(test)]
// L'arite (8 arguments) est dictee par la signature externe repliquee a
// l'identique : la reduire casserait l'encodage de l'appel.
#![allow(clippy::too_many_arguments)]
//! Mocks de venues pilotables par storage, pour exercer le VRAI flux de
//! fonds (transferts token reels) et l'auth imbriquee.
//!
//! Les signatures de swap sont EXACTEMENT celles des traits de `venues`
//! (les traits `contractclient` ne servent qu'a generer les clients : les
//! mocks sont des contrats autonomes aux memes signatures ; seuls les noms
//! des parametres ignores prennent un underscore, l'encodage n'en depend
//! pas).
//!
//! Flux de fonds : le mock tire `token_in` depuis `to`/`user` puis sert
//! `token_out` DEPUIS SON PROPRE SOLDE, pre-finance dans le setup de test :
//! plus proche de la realite qu'un mint (une venue reelle n'est pas admin du
//! token) et sans droit d'admin a accorder au mock.

use crate::venues::soroswap::DexDistribution;
use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, token::TokenClient, vec, Address, BytesN,
    Env, Vec,
};

/// Comportement pilotable d'un mock de venue.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MockBehavior {
    /// Sert exactement ce montant de token_out (et revert si ce montant est
    /// sous le minimum demande, comme une venue reelle).
    Serve(i128),
    /// Panique (venue en panne) : le `try_` de l'appelant doit l'absorber.
    Panic,
}

/// Montant a servir selon le comportement configure, ou panique (venue en
/// panne, ou sortie sous le minimum demande).
fn serve_amount(env: &Env, min_required: i128) -> i128 {
    let behavior: MockBehavior = env
        .storage()
        .instance()
        .get(&symbol_short!("behavior"))
        .expect("appeler set_behavior avant le swap");
    match behavior {
        MockBehavior::Panic => panic!("venue mock en panne"),
        MockBehavior::Serve(amount) => {
            if amount < min_required {
                panic!("venue mock: sortie sous le minimum demande");
            }
            amount
        }
    }
}

/// Mock de l'aggregator Soroswap (trait `venues::soroswap::SoroswapAggregator`).
#[contract]
pub struct MockAggregator;

#[contractimpl]
impl MockAggregator {
    pub fn set_behavior(env: Env, behavior: MockBehavior) {
        env.storage()
            .instance()
            .set(&symbol_short!("behavior"), &behavior);
    }

    pub fn swap_exact_tokens_for_tokens(
        env: Env,
        token_in: Address,
        token_out: Address,
        amount_in: i128,
        amount_out_min: i128,
        _distribution: Vec<DexDistribution>,
        to: Address,
        _deadline: u64,
    ) -> Vec<Vec<i128>> {
        let served = serve_amount(&env, amount_out_min);
        let this = env.current_contract_address();
        TokenClient::new(&env, &token_in).transfer(&to, &this, &amount_in);
        TokenClient::new(&env, &token_out).transfer(&this, &to, &served);
        // Forme du retour de l'aggregator reel : montants par distribution.
        vec![&env, vec![&env, amount_in, served]]
    }
}

/// Mock du router Aquarius (trait `venues::aqua::AquaRouter`).
#[contract]
pub struct MockAqua;

#[contractimpl]
impl MockAqua {
    pub fn set_behavior(env: Env, behavior: MockBehavior) {
        env.storage()
            .instance()
            .set(&symbol_short!("behavior"), &behavior);
    }

    pub fn swap_chained(
        env: Env,
        user: Address,
        swaps_chain: Vec<(Vec<Address>, BytesN<32>, Address)>,
        token_in: Address,
        in_amount: u128,
        out_min: u128,
    ) -> u128 {
        let out_min = i128::try_from(out_min).expect("out_min de test > i128::MAX");
        let served = serve_amount(&env, out_min);
        // Comme le vrai router : le token servi est le token_out du dernier
        // maillon de la chaine.
        let (_, _, token_out) = swaps_chain.last().expect("swaps_chain vide");
        let amount_in = i128::try_from(in_amount).expect("in_amount de test > i128::MAX");
        let this = env.current_contract_address();
        TokenClient::new(&env, &token_in).transfer(&user, &this, &amount_in);
        TokenClient::new(&env, &token_out).transfer(&this, &user, &served);
        u128::try_from(served).expect("montant servi negatif")
    }
}

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
    /// Venue MENTEUSE : sert ce montant en ignorant le minimum demande.
    /// Seule maniere d'atteindre la branche SlippageExceeded du routeur
    /// (defense en profondeur : jugement sur delta de solde).
    ServeIgnoringMin(i128),
    /// Venue qui EXECUTE reellement (tire token_in, sert ce montant de
    /// token_out, minimum ignore) puis RETOURNE un montant inconvertible
    /// (> i128::MAX). Ne s'observe que chez MockAqua, seul retour u128 :
    /// temoin du chemin ou attempt rend false APRES execution reelle.
    ServeReturningHuge(i128),
}

/// Marqueur d'invocation : pose en tete de la fonction de swap. Un appel
/// panique est annule avec son ecriture de storage : le marqueur ne prouve
/// donc que les invocations ABOUTIES ; son absence apres un chemin sans appel
/// ni panique prouve que la venue n'a pas ete invoquee.
fn mark_called(env: &Env) {
    env.storage()
        .instance()
        .set(&symbol_short!("called"), &true);
}

fn was_called(env: &Env) -> bool {
    env.storage()
        .instance()
        .get(&symbol_short!("called"))
        .unwrap_or(false)
}

fn configured_behavior(env: &Env) -> MockBehavior {
    env.storage()
        .instance()
        .get(&symbol_short!("behavior"))
        .expect("appeler set_behavior avant le swap")
}

/// Montant a servir selon le comportement configure, ou panique (venue en
/// panne, ou sortie sous le minimum demande).
fn serve_amount(env: &Env, min_required: i128) -> i128 {
    match configured_behavior(env) {
        MockBehavior::Panic => panic!("venue mock en panne"),
        MockBehavior::Serve(amount) => {
            if amount < min_required {
                panic!("venue mock: sortie sous le minimum demande");
            }
            amount
        }
        MockBehavior::ServeIgnoringMin(amount) | MockBehavior::ServeReturningHuge(amount) => amount,
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

    pub fn was_called(env: Env) -> bool {
        was_called(&env)
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
        mark_called(&env);
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

    pub fn was_called(env: Env) -> bool {
        was_called(&env)
    }

    pub fn swap_chained(
        env: Env,
        user: Address,
        swaps_chain: Vec<(Vec<Address>, BytesN<32>, Address)>,
        token_in: Address,
        in_amount: u128,
        out_min: u128,
    ) -> u128 {
        mark_called(&env);
        // Convention auto-verifiee : le premier maillon doit porter la paire
        // {token_in, token_out du maillon} TRIEE en ordre d'adresse croissant
        // (comme le vrai router Aqua) ; un tri inverse dans venues::aqua ne
        // passerait pas inapercu.
        let (pair, _, first_out) = swaps_chain.first().expect("swaps_chain vide");
        assert_eq!(pair.len(), 2, "venue mock: paire de taille invalide");
        let (a, b) = (pair.get_unchecked(0), pair.get_unchecked(1));
        assert!(a < b, "venue mock: paire non triee par adresse");
        let sorted_ok = (a == token_in && b == first_out) || (a == first_out && b == token_in);
        assert!(sorted_ok, "venue mock: paire != {{token_in, token_out}}");

        let out_min = i128::try_from(out_min).expect("out_min de test > i128::MAX");
        let served = serve_amount(&env, out_min);
        // Comme le vrai router : le token servi est le token_out du dernier
        // maillon de la chaine.
        let (_, _, token_out) = swaps_chain.last().expect("swaps_chain vide");
        let amount_in = i128::try_from(in_amount).expect("in_amount de test > i128::MAX");
        let this = env.current_contract_address();
        TokenClient::new(&env, &token_in).transfer(&user, &this, &amount_in);
        TokenClient::new(&env, &token_out).transfer(&this, &user, &served);
        // ServeReturningHuge : les tokens ONT bouge, mais le retour ment
        // (inconvertible en i128) ; l'attempt de l'appelant doit rendre
        // false et laisser l'atomicite de la transaction proteger les fonds.
        match configured_behavior(&env) {
            MockBehavior::ServeReturningHuge(_) => (i128::MAX as u128) + 1,
            _ => u128::try_from(served).expect("montant servi negatif"),
        }
    }
}

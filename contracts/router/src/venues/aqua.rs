//! Router Aquarius.
//!
//! Le repo canonique `AquaToken/soroban-amm` est en 404 : l'implementation de
//! reference de `swap_chained` est l'adapter Aqua du repo
//! `soroswap/aggregator`, `contracts/aggregator/src/adapters/aqua.rs`
//! (sources verifiees le 22/07/2026). Chaque maillon de `swaps_chain` =
//! (paire de tokens TRIEE par ordre d'adresse, pool_hash, token_out) ;
//! les montants sont en u128.

// L'arite de `attempt` (8 arguments) est transitoire : `pool_hash` devient
// une resolution interne au registre en Task 6.
#![allow(clippy::too_many_arguments)]

use soroban_sdk::{contractclient, vec, Address, BytesN, Env, Vec};

// Le trait ne sert qu'a generer le client (`contractclient`) : il n'est
// jamais implemente ni appele directement, seul le client l'est.
#[allow(dead_code)]
#[contractclient(name = "AquaRouterClient")]
pub trait AquaRouter {
    fn swap_chained(
        env: Env,
        user: Address,
        swaps_chain: Vec<(Vec<Address>, BytesN<32>, Address)>,
        token_in: Address,
        in_amount: u128,
        out_min: u128,
    ) -> u128;
}

/// Tente le swap via le router Aquarius : chaine d'un seul maillon, paire
/// triee par adresse (un pool Aqua sert les deux sens), `pool_hash` fourni
/// par l'appelant (la resolution depuis le registre `DataKey::AquaPool`
/// arrive en Task 6).
///
/// Conversions i128 -> u128 sures uniquement si >= 0 : le routeur garantit
/// deja la positivite, mais le module doit etre sur par lui-meme (negatif ->
/// `false`, jamais de panique dans `attempt`). Au retour, un u128 >
/// i128::MAX est inconvertible cote routeur -> `false` ; l'erreur typee
/// `AmountConversion` sur ce chemin de retour arrive en Task 6, `attempt`
/// rendant `bool` dans cette tache, le mapping est documente ici.
pub fn attempt(
    env: &Env,
    router: &Address,
    token_in: &Address,
    token_out: &Address,
    amount_in: i128,
    min_out: i128,
    user: &Address,
    pool_hash: &BytesN<32>,
) -> bool {
    if amount_in < 0 || min_out < 0 {
        return false;
    }
    let in_amount = amount_in as u128;
    let out_min = min_out as u128;

    // Paire TRIEE par ordre d'adresse (convention du router Aqua, cf.
    // adapter de reference) ; le token_out du maillon reste le vrai token_out.
    let pair = if token_in < token_out {
        vec![env, token_in.clone(), token_out.clone()]
    } else {
        vec![env, token_out.clone(), token_in.clone()]
    };
    let swaps_chain = vec![env, (pair, pool_hash.clone(), token_out.clone())];

    let result = AquaRouterClient::new(env, router).try_swap_chained(
        user,
        &swaps_chain,
        token_in,
        &in_amount,
        &out_min,
    );
    match result {
        Ok(Ok(out)) => out <= i128::MAX as u128,
        _ => false,
    }
}

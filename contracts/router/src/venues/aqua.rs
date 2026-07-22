//! Router Aquarius.
//!
//! Le repo canonique `AquaToken/soroban-amm` est en 404 : l'implementation de
//! reference de `swap_chained` est l'adapter Aqua du repo
//! `soroswap/aggregator`, `contracts/aggregator/src/adapters/aqua.rs`
//! (sources verifiees le 22/07/2026). Chaque maillon de `swaps_chain` =
//! (paire de tokens TRIEE par ordre d'adresse, pool_hash, token_out) ;
//! les montants sont en u128.

// L'arite de `attempt` (8 arguments) reflete la frontiere explicite : le
// routeur resout `pool_hash` au registre admin et le passe tel quel, le
// module venue reste pur (aucune lecture de storage ici).
#![allow(clippy::too_many_arguments)]

use super::convert;
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
/// triee par adresse (un pool Aqua sert les deux sens), `pool_hash` resolu
/// par le routeur depuis le registre admin (`set_aqua_pool`).
///
/// Conversions aux bornes via `venues::convert` (helpers purs, testes aux
/// bornes) : le routeur garantit deja la positivite, mais le module doit
/// etre sur par lui-meme (negatif -> `false`, jamais de panique dans
/// `attempt`). Au retour, un u128 > i128::MAX est inconvertible -> `false`,
/// le fallback decide ; pas d'erreur typee dediee, l'architecture
/// attempt-bool route ce chemin vers `AllVenuesFailed` (cf. `RouterError`).
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
    let (Some(in_amount), Some(out_min)) = (
        convert::u128_from_i128(amount_in),
        convert::u128_from_i128(min_out),
    ) else {
        return false;
    };

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
        Ok(Ok(out)) => convert::i128_from_u128(out).is_some(),
        _ => false,
    }
}

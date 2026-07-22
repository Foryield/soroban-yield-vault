//! Aggregator Soroswap.
//!
//! Types et trait repliques A L'IDENTIQUE depuis le repo `soroswap/aggregator`
//! (sources verifiees le 22/07/2026) :
//! - `contracts/aggregator/src/models.rs` : `Protocol`, `DexDistribution`
//!   (4 champs, dont `bytes: Option<Vec<BytesN<32>>>` = pool hashes Aqua,
//!   `None` pour le protocole Soroswap) ;
//! - `contracts/aggregator/src/lib.rs` : `swap_exact_tokens_for_tokens`.

// L'arite (8 arguments) est dictee par la signature externe repliquee a
// l'identique : la reduire casserait l'encodage de l'appel.
#![allow(clippy::too_many_arguments)]

use soroban_sdk::{contractclient, contracttype, vec, Address, BytesN, Env, Vec};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Protocol {
    Soroswap = 0,
    Phoenix = 1,
    Aqua = 2,
    Comet = 3,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DexDistribution {
    pub protocol_id: Protocol,
    pub path: Vec<Address>,
    pub parts: u32,
    pub bytes: Option<Vec<BytesN<32>>>,
}

#[contractclient(name = "SoroswapAggregatorClient")]
pub trait SoroswapAggregator {
    fn swap_exact_tokens_for_tokens(
        env: Env,
        token_in: Address,
        token_out: Address,
        amount_in: i128,
        amount_out_min: i128,
        distribution: Vec<DexDistribution>,
        to: Address,
        deadline: u64,
    ) -> Vec<Vec<i128>>;
}

/// Tente le swap via l'aggregator Soroswap : distribution unique
/// `{Protocol::Soroswap, path=[token_in, token_out], parts=1, bytes=None}`,
/// `deadline` = timestamp du ledger courant (l'appel s'execute dans la meme
/// transaction, aucune marge necessaire).
///
/// Rend `false` sur toute `Err` du client `try_` (erreur contrat ou hote) et
/// sur un retour inconvertible : `attempt` ne panique jamais a cause de la
/// venue. Le succes est juge par le routeur sur delta de solde.
pub fn attempt(
    env: &Env,
    aggregator: &Address,
    token_in: &Address,
    token_out: &Address,
    amount_in: i128,
    min_out: i128,
    to: &Address,
) -> bool {
    let distribution = vec![
        env,
        DexDistribution {
            protocol_id: Protocol::Soroswap,
            path: vec![env, token_in.clone(), token_out.clone()],
            parts: 1,
            bytes: None,
        },
    ];
    let result = SoroswapAggregatorClient::new(env, aggregator).try_swap_exact_tokens_for_tokens(
        token_in,
        token_out,
        &amount_in,
        &min_out,
        &distribution,
        to,
        &env.ledger().timestamp(),
    );
    matches!(result, Ok(Ok(_)))
}

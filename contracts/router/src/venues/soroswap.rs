//! Aggregator Soroswap.
//!
//! Types et traits repliques A L'IDENTIQUE depuis le repo `soroswap/aggregator`
//! (sources verifiees le 22/07/2026, commit epingle 84de10e0) :
//! - `contracts/aggregator/src/models.rs` : `Protocol`, `DexDistribution`
//!   (4 champs, dont `bytes: Option<Vec<BytesN<32>>>` = pool hashes Aqua,
//!   `None` pour le protocole Soroswap), `Adapter` ;
//! - `contracts/aggregator/src/lib.rs` : `swap_exact_tokens_for_tokens`,
//!   `get_adapters` ;
//! - `protocols/soroswap` (soroswap/core, sous-module au commit epingle),
//!   `contracts/router/src/lib.rs` : `router_pair_for`.
//!
//! Topologie d'auth du stack reel (etablie a la task 10) : l'aggregator ne
//! detient jamais les fonds. Son adapter interne Soroswap appelle le ROUTER
//! Soroswap, qui fait `to.require_auth()` sur sa propre frame puis
//! `transfer(to -> pair, amount_in)`. Quand `to` est notre routeur, ces deux
//! require_auth imbriques (frame du router Soroswap, transfert vers la paire)
//! doivent etre pre-autorises par `authorize_as_current_contract` avec l'arbre
//! EXACT : c'est le role de `pull_auth_entries`, qui decouvre les adresses du
//! router Soroswap (`get_adapters`) et de la paire (`router_pair_for`) par
//! appels de lecture AVANT toute pre-autorisation (une pre-autorisation ne
//! couvre que l'invocation suivante : tout appel intercale la consommerait).

// L'arite (8 arguments) est dictee par la signature externe repliquee a
// l'identique : la reduire casserait l'encodage de l'appel.
#![allow(clippy::too_many_arguments)]

use soroban_sdk::{
    auth::{ContractContext, InvokerContractAuthEntry, SubContractInvocation},
    contractclient, contracttype, vec, Address, BytesN, Env, IntoVal, Symbol, Vec,
};

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

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Adapter {
    pub protocol_id: Protocol,
    pub router: Address,
    pub paused: bool,
}

// Les traits ne servent qu'a generer les clients (`contractclient`) : ils ne
// sont jamais implementes ni appeles directement, seuls les clients le sont.
// Convention de replication : les `Result<T, ...Error>` des signatures
// sources sont aplatis en `T` (l'erreur contrat remonte dans le `Err` du
// client `try_`, seule voie d'appel utilisee ici).
#[allow(dead_code)]
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

    fn get_adapters(env: Env) -> Vec<Adapter>;
}

#[allow(dead_code)]
#[contractclient(name = "SoroswapRouterClient")]
pub trait SoroswapRouter {
    fn router_pair_for(env: Env, token_a: Address, token_b: Address) -> Address;
}

/// Deadline STRICTEMENT future : `ensure_deadline` du router Soroswap rejette
/// `now >= deadline` (source epinglee, `contracts/router/src/lib.rs`), donc
/// `timestamp()` nu echouerait a chaque appel. `saturating_add` : jamais de
/// panique dans le chemin de venue ; a saturation (timestamp = u64::MAX,
/// irrealiste) la deadline egale `now`, la venue refuse et le fallback decide.
fn deadline(env: &Env) -> u64 {
    env.ledger().timestamp().saturating_add(1)
}

/// Entree de pre-autorisation d'un `transfer(from, to, amount)` sur `token`.
fn transfer_entry(
    env: &Env,
    token: &Address,
    from: &Address,
    to: &Address,
    amount: i128,
) -> InvokerContractAuthEntry {
    InvokerContractAuthEntry::Contract(SubContractInvocation {
        context: ContractContext {
            contract: token.clone(),
            fn_name: Symbol::new(env, "transfer"),
            args: (from.clone(), to.clone(), amount).into_val(env),
        },
        sub_invocations: Vec::new(env),
    })
}

/// Entrees de pre-autorisation du tirage de `token_in` par le stack Soroswap,
/// a passer a `env.authorize_as_current_contract` par l'appelant (le routeur)
/// juste avant `attempt` : la pre-autorisation ne couvre que l'invocation
/// suivante, les appels de decouverte ci-dessous doivent donc la PRECEDER.
///
/// Deux couches, chacune inoffensive si non consommee (les entrees meurent
/// avec la transaction) :
/// - entree generique `transfer(to -> aggregator, amount_in)` : couvre un
///   aggregator qui tirerait lui-meme les fonds (topologie des mocks de
///   test ; l'aggregator reel ne detient jamais rien) ;
/// - arbre reel : frame `swap_exact_tokens_for_tokens` du ROUTER Soroswap
///   (`amount_out_min` = 0, valeur codee en dur par l'adapter interne de
///   l'aggregator, source epinglee) avec en sous-invocation le
///   `transfer(to -> pair, amount_in)`. Construit seulement si la decouverte
///   (`get_adapters` puis `router_pair_for`, clients `try_`) aboutit ; sinon
///   l'entree generique reste seule et l'echec d'auth eventuel du stack reel
///   degrade en `attempt` false puis fallback, jamais en panique.
pub fn pull_auth_entries(
    env: &Env,
    aggregator: &Address,
    token_in: &Address,
    token_out: &Address,
    amount_in: i128,
    to: &Address,
) -> Vec<InvokerContractAuthEntry> {
    let mut entries = vec![
        env,
        transfer_entry(env, token_in, to, aggregator, amount_in),
    ];

    let Ok(Ok(adapters)) = SoroswapAggregatorClient::new(env, aggregator).try_get_adapters() else {
        return entries;
    };
    let Some(router) = adapters
        .iter()
        .find(|adapter| adapter.protocol_id == Protocol::Soroswap)
        .map(|adapter| adapter.router)
    else {
        return entries;
    };
    let Ok(Ok(pair)) =
        SoroswapRouterClient::new(env, &router).try_router_pair_for(token_in, token_out)
    else {
        return entries;
    };

    let path = vec![env, token_in.clone(), token_out.clone()];
    entries.push_back(InvokerContractAuthEntry::Contract(SubContractInvocation {
        context: ContractContext {
            contract: router,
            fn_name: Symbol::new(env, "swap_exact_tokens_for_tokens"),
            args: (amount_in, 0_i128, path, to.clone(), deadline(env)).into_val(env),
        },
        sub_invocations: vec![env, transfer_entry(env, token_in, to, &pair, amount_in)],
    }));
    entries
}

/// Tente le swap via l'aggregator Soroswap : distribution unique
/// `{Protocol::Soroswap, path=[token_in, token_out], parts=1, bytes=None}`,
/// `deadline` = timestamp du ledger courant + 1 (cf. `deadline` : la
/// comparaison du router Soroswap est stricte, meme transaction ou pas).
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
    // Pas de garde sur les negatifs ici, a dessein : les montants restent en
    // i128, la venue les rejette elle-meme et try_ absorbe l'erreur (la
    // pre-garde d'aqua n'existe que pour le cast i128 -> u128 non sur).
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
        &deadline(env),
    );
    matches!(result, Ok(Ok(_)))
}

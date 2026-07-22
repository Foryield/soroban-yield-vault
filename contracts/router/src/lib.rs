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
//! - registre admin des pools Aquarius (pool_hash par paire TRIEE) : le hash
//!   change a chaque re-seed testnet, `set_aqua_pool` (admin) evite un
//!   redeploiement pour un simple identifiant de pool ; registre VIDE = la
//!   venue Aquarius rend false et le fallback la traverse (pas d'erreur
//!   typee : pool non configure est un etat d'ops, observable via
//!   `aqua_pool_of`). Registre en storage instance : cardinalite petite et
//!   bornee (ecritures admin uniquement, univers cure de paires stablecoin) ;
//!   si l'espace de paires s'ouvrait, migrer en persistent avec extend_ttl
//!   en lecture et ecriture ;
//! - invariant : solde du routeur nul hors transaction (le produit du swap
//!   est integralement reverse a l'appelant dans la meme invocation) ;
//! - modele de confiance des tokens : SAC/SEP-41 supposes sans frais de
//!   transfert ni hooks (le montant transfere est le montant recu, le
//!   jugement par delta de solde y suffit) ; un token menteur ne nuit qu'a
//!   son propre appelant, le routeur ne detenant rien entre transactions.
//!
//! Hors scope D4 : multi-hop (pas de `path` expose), setters de venues,
//! frais preleves par le routeur (fee_bps = comptabilite, pas prelevement).

use soroban_sdk::{
    auth::{ContractContext, InvokerContractAuthEntry, SubContractInvocation},
    contract, contracterror, contractevent, contractimpl, contractmeta, contracttype,
    panic_with_error,
    token::TokenClient,
    vec, Address, BytesN, Env, IntoVal, Symbol, Vec,
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
    // Codes 5 (AquaPoolNotSet) et 8 (AmountConversion) RETIRES, decision de
    // design validee : l'architecture attempt-bool route registre vide et
    // debordement de conversion vers false -> fallback -> AllVenuesFailed ;
    // le client distingue deja slippage (SlippageExceeded) et panne de venue
    // (AllVenuesFailed) ; pool non configure est un etat d'ops observable
    // via aqua_pool_of. Les codes des variantes restantes sont FIGES, trous
    // admis : un code publie ne change jamais de sens.
    AllVenuesFailed = 6,
    SlippageExceeded = 7,
    MathOverflow = 9,
    InvalidFeeBps = 10,
}

/// Borne haute des fee_bps a l'initialize : 10 000 bps = 100 %.
const MAX_FEE_BPS: u32 = 10_000;

/// Denominateur du calcul de frais : fee = amount_in x fee_bps / 10 000.
const BPS_DENOMINATOR: i128 = 10_000;

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

/// Event `swap` : #[contractevent] (style cible D6a) des le premier commit,
/// le routeur etant un contrat neuf sans format herite (le vault garde ses
/// events env.events().publish deprecies jusqu'a la migration D6a planifiee).
///
/// Convention D6a minimale : acteur (from), instruments (token_in,
/// token_out), montants (amount_in, amount_out, fee, min_out), venue
/// preferee vs effective = decision d'execution (venue = EFFECTIVE, celle
/// qui a servi apres fallback ; preferred = demandee par le client : un
/// consommateur du seul flux d'events voit qu'un fallback a eu lieu,
/// preferred != venue, sans recouper les arguments d'invocation).
///
/// Choix topics/data : nom d'event fixe `swap` + acteur + instruments en
/// topics (un consommateur filtre par compte ou par paire sans decoder la
/// data) ; montants et venue en data (payload non filtrable). Le derive du
/// sdk supporte #[topic] par champ ; 4 topics au total, le plafond d'usage
/// des events Soroban (le transfer SAC en emploie 4). Pas d'event d'echec :
/// l'echec des deux venues revert tout, events compris.
#[contractevent(topics = ["swap"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SwapEvent {
    #[topic]
    pub from: Address,
    #[topic]
    pub token_in: Address,
    #[topic]
    pub token_out: Address,
    pub amount_in: i128,
    pub amount_out: i128,
    pub venue: Venue,
    pub preferred: Venue,
    pub fee: i128,
    pub min_out: i128,
}

/// Event `aqua_pool_set` : changement de config admin auditable on-chain
/// (posture D6a, suivi de revue Task 6). Porte la paire TRIEE par adresse,
/// l'identite de la cle de registre, quel que soit l'ordre des arguments du
/// setter. Tokens en topics (filtrage par paire), pool_hash en data.
#[contractevent(topics = ["aqua_pool_set"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AquaPoolSetEvent {
    #[topic]
    pub token_a: Address,
    #[topic]
    pub token_b: Address,
    pub pool_hash: BytesN<32>,
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
        // Garde bps (suivi de revue Task 2) : au-dela de 100 %, le fee
        // comptabilise depasserait le montant swappe, non-sens.
        if soroswap_fee_bps > MAX_FEE_BPS || aquarius_fee_bps > MAX_FEE_BPS {
            panic_with_error!(&env, RouterError::InvalidFeeBps);
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

    /// Echange `amount_in` de `token_in` contre au moins `min_out` de
    /// `token_out`, servi a `from`. `preferred` fixe l'ordre d'essai des
    /// venues ; la venue de secours prend le relais dans la MEME transaction.
    /// Panique en `AllVenuesFailed` si aucune venue ne sert : le revert
    /// integral protege les fonds.
    pub fn swap_exact_in(
        env: Env,
        from: Address,
        token_in: Address,
        token_out: Address,
        amount_in: i128,
        min_out: i128,
        preferred: Venue,
    ) -> SwapResult {
        from.require_auth();
        if amount_in <= 0 {
            panic_with_error!(&env, RouterError::AmountMustBePositive);
        }
        if min_out <= 0 {
            panic_with_error!(&env, RouterError::MinOutMustBePositive);
        }
        if token_in == token_out {
            panic_with_error!(&env, RouterError::SameToken);
        }

        let this = env.current_contract_address();
        TokenClient::new(&env, &token_in).transfer(&from, &this, &amount_in);
        let out_token = TokenClient::new(&env, &token_out);
        let before = out_token.balance(&this);

        let order = match preferred {
            Venue::SoroswapAggregator => [Venue::SoroswapAggregator, Venue::AquariusRouter],
            Venue::AquariusRouter => [Venue::AquariusRouter, Venue::SoroswapAggregator],
        };
        let mut venue_used = None;
        for venue in order {
            if Self::attempt_venue(&env, venue, &token_in, &token_out, amount_in, min_out) {
                // Succes juge sur delta de solde, jamais sur le retour de la
                // venue (cf. venues.rs).
                let received = out_token
                    .balance(&this)
                    .checked_sub(before)
                    .unwrap_or_else(|| panic_with_error!(&env, RouterError::MathOverflow));
                if received >= min_out {
                    venue_used = Some((venue, received));
                    break;
                }
                // La venue a « reussi » en servant moins que min_out :
                // defense en profondeur, tout revert plutot que d'arbitrer.
                panic_with_error!(&env, RouterError::SlippageExceeded);
            }
        }
        // INVARIANT : le chemin « toutes venues false » DOIT paniquer, jamais
        // retourner. C'est lui qui garantit le revert INTEGRAL quand une
        // venue a execute mais que `attempt` a rendu false (retour
        // indecodable, conversion) : les fonds sont proteges par l'atomicite
        // de la transaction, pas par le jugement local.
        let (venue, amount_out) =
            venue_used.unwrap_or_else(|| panic_with_error!(&env, RouterError::AllVenuesFailed));

        // Frais COMPTABLES uniquement : rien n'est preleve sur amount_out,
        // la commission de la venue est deja incorporee au prix servi.
        // `fee` alimente le SwapResult et les stats (dashboard D6c).
        let fee = amount_in
            .checked_mul(i128::from(Self::fee_bps(&env, venue)))
            .unwrap_or_else(|| panic_with_error!(&env, RouterError::MathOverflow))
            / BPS_DENOMINATOR;

        // Convention maison (vault D1) : ETAT D'ABORD, TRANSFERT ENSUITE.
        // amount_out est deja juge : les stats s'ecrivent avant le transfert
        // sortant, aucun appel externe ne s'intercale entre le jugement et
        // l'ecriture d'etat (CEI).
        Self::record_swap(&env, &token_in, &token_out, amount_in, amount_out, fee);

        // Event juste apres l'ecriture des stats : il decrit un swap DEJA
        // comptabilise, et le bloc etat+event reste groupe avant le transfert
        // sortant (CEI ; un event n'est pas de l'etat, mais la lecture y
        // gagne). Venue EFFECTIVE (celle qui a servi apres fallback) ET
        // preferee : preferred != venue signale un fallback au consommateur
        // du seul flux d'events.
        SwapEvent {
            from: from.clone(),
            token_in,
            token_out,
            amount_in,
            amount_out,
            venue,
            preferred,
            fee,
            min_out,
        }
        .publish(&env);

        out_token.transfer(&this, &from, &amount_out);

        SwapResult {
            amount_out,
            venue,
            fee,
        }
    }

    /// Enregistre (ou remplace) le pool Aquarius de la paire, sous cle TRIEE
    /// par adresse (un pool sert les deux sens). Admin uniquement. Le hash
    /// change a chaque re-seed testnet : ce setter evite un redeploiement
    /// pour un simple identifiant de pool.
    pub fn set_aqua_pool(env: Env, token_a: Address, token_b: Address, pool_hash: BytesN<32>) {
        Self::admin(&env).require_auth();
        // Paire degeneree rejetee (suivi de revue Task 6) : sans deleter en
        // D4, une entree (t, t) n'aurait aucune voie de suppression.
        if token_a == token_b {
            panic_with_error!(&env, RouterError::SameToken);
        }
        let (token_a, token_b) = Self::sorted_pair(token_a, token_b);
        env.storage().instance().set(
            &DataKey::AquaPool(token_a.clone(), token_b.clone()),
            &pool_hash,
        );
        AquaPoolSetEvent {
            token_a,
            token_b,
            pool_hash,
        }
        .publish(&env);
    }

    /// Pool Aquarius enregistre pour la paire (ordre des tokens indifferent),
    /// `None` si le registre est vide. Surface ops/demo : verifier l'etat du
    /// registre sans redeployer (PR C s'en sert apres chaque re-seed).
    pub fn aqua_pool_of(env: Env, token_a: Address, token_b: Address) -> Option<BytesN<32>> {
        Self::aqua_pool(&env, &token_a, &token_b)
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

    /// Tente `venue` : pre-autorise le tirage de `token_in` par la venue,
    /// puis delegue au client du sous-module. Rend `false` si la venue
    /// echoue ou, pour Aquarius, si le registre de pool est vide.
    fn attempt_venue(
        env: &Env,
        venue: Venue,
        token_in: &Address,
        token_out: &Address,
        amount_in: i128,
        min_out: i128,
    ) -> bool {
        let venue_addr = Self::venue_addr(env, venue);
        let this = env.current_contract_address();
        match venue {
            Venue::SoroswapAggregator => {
                // La topologie d'auth du stack Soroswap reel est propre a la
                // venue (frame du router Soroswap + transfert vers la paire,
                // decouverts par appels de lecture) : le module venue
                // construit les entrees, le routeur les endosse. Les appels
                // de decouverte precedent OBLIGATOIREMENT l'endossement (une
                // pre-autorisation ne couvre que l'invocation suivante).
                let entries = venues::soroswap::pull_auth_entries(
                    env,
                    &venue_addr,
                    token_in,
                    token_out,
                    amount_in,
                    &this,
                );
                env.authorize_as_current_contract(entries);
                venues::soroswap::attempt(
                    env,
                    &venue_addr,
                    token_in,
                    token_out,
                    amount_in,
                    min_out,
                    &this,
                )
            }
            Venue::AquariusRouter => {
                // Registre vide -> false, le fallback traverse la venue :
                // etat d'ops observable via aqua_pool_of, pas d'erreur typee
                // dediee (cf. RouterError).
                let Some(pool_hash) = Self::aqua_pool(env, token_in, token_out) else {
                    return false;
                };
                Self::authorize_venue_pull(env, &venue_addr, token_in, amount_in);
                venues::aqua::attempt(
                    env,
                    &venue_addr,
                    token_in,
                    token_out,
                    amount_in,
                    min_out,
                    &this,
                    &pool_hash,
                )
            }
        }
    }

    /// La venue Aquarius tire `token_in` du routeur via un token.transfer
    /// imbrique : l'auth d'invocateur ne couvrant que l'appel direct, ce
    /// transfert est pre-autorise explicitement (meme motif que pool_supply
    /// du vault D1). La pre-autorisation est etroite (token, venue et montant
    /// exacts) et meurt avec la transaction : une tentative echouee ne laisse
    /// rien d'exploitable.
    ///
    /// Topologie REELLE verifiee (task 11, stack Aqua depuis les wasm
    /// vendorises + miroir des sources, cf. test_aqua_stack.rs) : c'est bien
    /// l'arbre exact. swap_chained fait user.require_auth() dans SA frame
    /// (couvert par l'auth d'invocateur direct, notre routeur l'appelant sans
    /// intermediaire) puis un ESCROW transfer(user -> router Aqua, in_amount)
    /// -- precisement cette entree ; les transferts internes vers les pools
    /// sont pre-autorises par le router Aqua lui-meme. Contrairement a
    /// Soroswap, aucune construction dediee n'est requise : la venue Soroswap
    /// a la sienne (cf. venues::soroswap::pull_auth_entries : l'arbre reel
    /// passe par le router Soroswap et la paire, pas par l'aggregator).
    fn authorize_venue_pull(env: &Env, venue_addr: &Address, token_in: &Address, amount_in: i128) {
        let this = env.current_contract_address();
        env.authorize_as_current_contract(vec![
            env,
            InvokerContractAuthEntry::Contract(SubContractInvocation {
                context: ContractContext {
                    contract: token_in.clone(),
                    fn_name: Symbol::new(env, "transfer"),
                    args: (this, venue_addr.clone(), amount_in).into_val(env),
                },
                sub_invocations: Vec::new(env),
            }),
        ]);
    }

    /// Paire TRIEE par adresse : setter (cle ET event) et lecteurs partagent
    /// la meme construction, aucun desaccord d'identite possible.
    fn sorted_pair(a: Address, b: Address) -> (Address, Address) {
        if a < b {
            (a, b)
        } else {
            (b, a)
        }
    }

    /// Cle de registre de la paire, TRIEE par adresse.
    fn aqua_pool_key(a: &Address, b: &Address) -> DataKey {
        let (a, b) = Self::sorted_pair(a.clone(), b.clone());
        DataKey::AquaPool(a, b)
    }

    /// Pool Aqua de la paire. `None` tant que `set_aqua_pool` n'a pas
    /// alimente le registre.
    fn aqua_pool(env: &Env, a: &Address, b: &Address) -> Option<BytesN<32>> {
        env.storage().instance().get(&Self::aqua_pool_key(a, b))
    }

    /// Accumule les stats de la paire ORDONNEE (token_in, token_out) en
    /// storage persistent, arithmetique verifiee.
    fn record_swap(
        env: &Env,
        token_in: &Address,
        token_out: &Address,
        amount_in: i128,
        amount_out: i128,
        fee: i128,
    ) {
        let prev = Self::pair_stats(env.clone(), token_in.clone(), token_out.clone());
        let overflow = || panic_with_error!(env, RouterError::MathOverflow);
        let stats = PairStats {
            volume_in: prev
                .volume_in
                .checked_add(amount_in)
                .unwrap_or_else(overflow),
            volume_out: prev
                .volume_out
                .checked_add(amount_out)
                .unwrap_or_else(overflow),
            fees: prev.fees.checked_add(fee).unwrap_or_else(overflow),
            swaps: prev
                .swaps
                .checked_add(1)
                .unwrap_or_else(|| panic_with_error!(env, RouterError::MathOverflow)),
        };
        env.storage()
            .persistent()
            .set(&DataKey::Stats(token_in.clone(), token_out.clone()), &stats);
    }

    fn admin(env: &Env) -> Address {
        env.storage().instance().get(&DataKey::Admin).unwrap()
    }

    fn venue_addr(env: &Env, venue: Venue) -> Address {
        let key = match venue {
            Venue::SoroswapAggregator => DataKey::SoroswapAggregator,
            Venue::AquariusRouter => DataKey::AquariusRouter,
        };
        env.storage().instance().get(&key).unwrap()
    }

    fn fee_bps(env: &Env, venue: Venue) -> u32 {
        let key = match venue {
            Venue::SoroswapAggregator => DataKey::SoroswapFeeBps,
            Venue::AquariusRouter => DataKey::AquariusFeeBps,
        };
        env.storage().instance().get(&key).unwrap()
    }
}

mod venues;

#[cfg(test)]
mod test;
#[cfg(test)]
mod test_aqua_stack;
#[cfg(test)]
mod test_mocks;
#[cfg(test)]
mod test_props;
#[cfg(test)]
mod test_soroswap_stack;
#[cfg(test)]
mod test_stack_common;

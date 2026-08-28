#![no_std]
//! ForYield Soroban SwapRouter (Deliverable D4).
//!
//! MONO-VENUE depuis le 28/08/2026 : la venue Soroswap a ete retiree du
//! projet suite a sa compromission (cf.
//! docs/plans/2026-08-28-retrait-soroswap-aquarius-seul.md). Aucune venue de
//! remplacement n'a ete retenue : Phoenix impose un whitelisting du factory
//! pour creer un pool, notre paire ne peut donc pas y etre semee. Le secours
//! atomique disparait avec la seconde venue, et Aquarius devient un point de
//! defaillance unique : c'est un arbitrage assume, pas un oubli.
//!
//! - le contrat garantit ce que seul l'on-chain garantit : min-out juge sur
//!   delta de solde (soit la venue sert au moins min_out, soit tout revert) ;
//!   la cotation reste off-chain (scripts/quote_aqua.sh), elle calibre min_out ;
//! - venue (router Aquarius) fixee a l'initialize, immuable : changement de
//!   venue = redeploiement (meme convention que le pool du vault D1) ;
//! - registre admin des pools Aquarius (pool_hash par paire TRIEE) : le hash
//!   change a chaque re-seed testnet, `set_aqua_pool` (admin) evite un
//!   redeploiement pour un simple identifiant de pool ; registre VIDE =
//!   erreur typee `AquaPoolNotSet` (sans fallback a traverser, un registre
//!   vide est une condition d'ops nette, le client merite le code qui la
//!   nomme). Registre en storage instance : cardinalite petite et bornee
//!   (ecritures admin uniquement, univers cure de paires stablecoin) ; si
//!   l'espace de paires s'ouvrait, migrer en persistent avec extend_ttl en
//!   lecture et ecriture ;
//! - invariant : solde du routeur nul hors transaction (le produit du swap
//!   est integralement reverse a l'appelant dans la meme invocation) ;
//! - modele de confiance des tokens : SAC/SEP-41 supposes sans frais de
//!   transfert ni hooks (le montant transfere est le montant recu, le
//!   jugement par delta de solde y suffit) ; un token menteur ne nuit qu'a
//!   son propre appelant, le routeur ne detenant rien entre transactions.
//!
//! Hors scope D4 : multi-hop (pas de `path` expose), setter de venue,
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
/// de garde restent distinctes de `VenueFailed` (le client distingue
/// slippage, registre vide et panne de venue).
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum RouterError {
    AlreadyInitialized = 1,
    AmountMustBePositive = 2,
    MinOutMustBePositive = 3,
    SameToken = 4,
    /// REINTRODUIT le 28/08/2026 sous son sens d'origine. Le code 5 avait ete
    /// retire quand l'architecture a deux venues routait un registre vide vers
    /// false -> fallback -> AllVenuesFailed. Sans seconde venue, il n'y a plus
    /// de fallback a traverser : le registre vide redevient une condition d'ops
    /// nette, que le client doit pouvoir distinguer d'une panne de venue.
    AquaPoolNotSet = 5,
    /// Anciennement `AllVenuesFailed`. Meme code, MEME SENS (la venue n'a pas
    /// servi), nom au singulier depuis le passage mono-venue : un code publie
    /// ne change jamais de sens, seule sa denomination suit l'architecture.
    VenueFailed = 6,
    SlippageExceeded = 7,
    // Code 8 (AmountConversion) RETIRE : un debordement de conversion aux
    // bornes Aquarius reste route vers false -> VenueFailed. Trou admis, un
    // code publie ne change jamais de sens.
    MathOverflow = 9,
    InvalidFeeBps = 10,
}

/// Borne haute des fee_bps a l'initialize : 10 000 bps = 100 %.
const MAX_FEE_BPS: u32 = 10_000;

/// Denominateur du calcul de frais : fee = amount_in x fee_bps / 10 000.
const BPS_DENOMINATOR: i128 = 10_000;

contractmeta!(
    key = "desc",
    val = "ForYield SwapRouter - venue Aquarius, min-out, comptabilite de frais"
);

/// Resultat d'un swap servi : montant sorti et frais comptabilises
/// (amount_in x fee_bps de la venue / 10 000).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SwapResult {
    pub amount_out: i128,
    pub fee: i128,
}

/// Event `swap` : #[contractevent] (style cible D6a) des le premier commit,
/// le routeur etant un contrat neuf sans format herite (le vault garde ses
/// events env.events().publish deprecies jusqu'a la migration D6a planifiee).
///
/// Convention D6a minimale : acteur (from), instruments (token_in,
/// token_out), montants (amount_in, amount_out, fee, min_out). Les champs
/// `venue` et `preferred` ont ete RETIRES avec la seconde venue le
/// 28/08/2026 : ils encodaient une decision d'execution (fallback ou non)
/// qui n'existe plus, et un champ constant n'apprend rien a un consommateur.
///
/// Choix topics/data : nom d'event fixe `swap` + acteur + instruments en
/// topics (un consommateur filtre par compte ou par paire sans decoder la
/// data) ; montants en data (payload non filtrable). Le derive du sdk
/// supporte #[topic] par champ ; 4 topics au total, le plafond d'usage des
/// events Soroban (le transfer SAC en emploie 4). Pas d'event d'echec :
/// l'echec de la venue revert tout, events compris.
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
    AquariusRouter,
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
    /// La venue et son fee_bps sont immuables (pas de setter en D4) :
    /// changement de venue = redeploiement.
    pub fn initialize(env: Env, admin: Address, aquarius_router: Address, aquarius_fee_bps: u32) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic_with_error!(&env, RouterError::AlreadyInitialized);
        }
        // Garde bps (suivi de revue Task 2) : au-dela de 100 %, le fee
        // comptabilise depasserait le montant swappe, non-sens.
        if aquarius_fee_bps > MAX_FEE_BPS {
            panic_with_error!(&env, RouterError::InvalidFeeBps);
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::AquariusRouter, &aquarius_router);
        env.storage()
            .instance()
            .set(&DataKey::AquariusFeeBps, &aquarius_fee_bps);
    }

    /// Echange `amount_in` de `token_in` contre au moins `min_out` de
    /// `token_out`, servi a `from`, via la venue Aquarius. Panique en
    /// `AquaPoolNotSet` si la paire n'est pas au registre, en `VenueFailed`
    /// si la venue ne sert pas : le revert integral protege les fonds.
    pub fn swap_exact_in(
        env: Env,
        from: Address,
        token_in: Address,
        token_out: Address,
        amount_in: i128,
        min_out: i128,
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

        // Registre lu AVANT le transfert entrant serait plus econome, mais le
        // transfert precede la lecture depuis l'origine et l'ordre n'a pas
        // d'incidence : les deux chemins revert integralement.
        let Some(pool_hash) = Self::aqua_pool(&env, &token_in, &token_out) else {
            panic_with_error!(&env, RouterError::AquaPoolNotSet);
        };

        let this = env.current_contract_address();
        TokenClient::new(&env, &token_in).transfer(&from, &this, &amount_in);
        let out_token = TokenClient::new(&env, &token_out);
        let before = out_token.balance(&this);

        let venue_addr = Self::venue_addr(&env);
        Self::authorize_venue_pull(&env, &venue_addr, &token_in, amount_in);
        // INVARIANT : `attempt` rendant false DOIT paniquer, jamais retourner.
        // C'est lui qui garantit le revert INTEGRAL quand la venue a execute
        // mais que `attempt` a rendu false (retour indecodable, conversion) :
        // les fonds sont proteges par l'atomicite de la transaction, pas par
        // le jugement local. Le passage mono-venue simplifie ce chemin, il ne
        // l'affaiblit pas.
        if !venues::aqua::attempt(
            &env,
            &venue_addr,
            &token_in,
            &token_out,
            amount_in,
            min_out,
            &this,
            &pool_hash,
        ) {
            panic_with_error!(&env, RouterError::VenueFailed);
        }

        // Succes juge sur delta de solde, jamais sur le retour de la venue
        // (cf. venues.rs).
        let amount_out = out_token
            .balance(&this)
            .checked_sub(before)
            .unwrap_or_else(|| panic_with_error!(&env, RouterError::MathOverflow));
        if amount_out < min_out {
            // La venue a « reussi » en servant moins que min_out : defense en
            // profondeur, tout revert plutot que d'arbitrer.
            panic_with_error!(&env, RouterError::SlippageExceeded);
        }

        // Frais COMPTABLES uniquement : rien n'est preleve sur amount_out,
        // la commission de la venue est deja incorporee au prix servi.
        // `fee` alimente le SwapResult et les stats (dashboard D6c).
        let fee = amount_in
            .checked_mul(i128::from(Self::fee_bps(&env)))
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
        // gagne).
        SwapEvent {
            from: from.clone(),
            token_in,
            token_out,
            amount_in,
            amount_out,
            fee,
            min_out,
        }
        .publish(&env);

        out_token.transfer(&this, &from, &amount_out);

        SwapResult { amount_out, fee }
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

    /// La venue Aquarius tire `token_in` du routeur via un token.transfer
    /// imbrique : l'auth d'invocateur ne couvrant que l'appel direct, ce
    /// transfert est pre-autorise explicitement (meme motif que pool_supply
    /// du vault D1). La pre-autorisation est etroite (token, venue et montant
    /// exacts) et meurt avec la transaction : une tentative echouee ne laisse
    /// rien d'exploitable.
    ///
    /// Topologie REELLE verifiee (task 11, stack Aqua depuis les wasm
    /// vendorises + miroir des sources, cf. test_aqua_stack.rs) : c'est bien
    /// l'arbre exact. Le `user` de swap_chained est NOTRE routeur, pas
    /// l'utilisateur final : swap_chained fait user.require_auth() dans SA
    /// frame (couvert par l'auth d'invocateur direct, notre routeur
    /// l'appelant sans intermediaire) puis un ESCROW transfer(routeur
    /// ForYield -> router Aqua, in_amount) -- precisement cette entree ;
    /// les transferts internes vers les pools sont pre-autorises par le
    /// router Aqua lui-meme. Une seule entree suffit donc, contrairement a
    /// l'arbre a deux niveaux qu'exigeait la venue Soroswap retiree.
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

    fn venue_addr(env: &Env) -> Address {
        env.storage()
            .instance()
            .get(&DataKey::AquariusRouter)
            .unwrap()
    }

    fn fee_bps(env: &Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::AquariusFeeBps)
            .unwrap()
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
mod test_rebalance;
#[cfg(test)]
mod test_stack_common;

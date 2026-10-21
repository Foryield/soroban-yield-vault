#![no_std]
//! ForYield Soroban YieldVault (Tranche 1 / Deliverable 1, allocateur D5).
//!
//! - depot d'un actif (USDC, via son StellarAssetContract) et emission de parts
//!   proportionnelles : parts = montant x total_parts / actifs_avant, tronque ;
//! - retrait pro-rata : montant = parts x actifs / total_parts, tronque ;
//! - les arrondis de parts et de montants sont en faveur du vault ; seule la
//!   valorisation tronquee de la position Blend peut sous-estimer les actifs
//!   d'au plus une unite brute (poussiere au benefice de l'entrant, du meme
//!   ordre que la tolerance MINIMUM_LIQUIDITY) ;
//! - MINIMUM_LIQUIDITY parts mortes au premier depot (anti-inflation) ;
//! - allocation D5 : 0 a MAX_STRATEGIES strategies au trait DeFindex
//!   (`defindex-strategy-core`), fixees au deploiement avec cible et plafond.
//!   Un depot reste oisif ; un retrait est servi par l'oisif puis par les
//!   strategies dans l'ordre de la liste (ordre de liquidite). total_assets =
//!   oisif + somme des `balance` des strategies : le rendement d'une strategie
//!   fait monter le prix de la part sans aucune action du vault ;
//! - pause d'urgence (admin) ;
//! - prolongation de la duree de vie a chaque depot et retrait : l'instance,
//!   le code et l'entree de parts du porteur, sans quoi ils s'archivent au bout
//!   de la duree par defaut du reseau, environ sept jours.
//!
//! - reequilibrage par lot (`rebalance`, keeper ou admin) : chaque poche,
//!   oisif compris, revient a sa cible des qu'une d'elles s'en ecarte de plus
//!   de `drift_bps` ; plafonds par strategie, perte bornee par lot, sortie
//!   d'urgence par strategie (`emergency_exit`, admin, meme en pause).
//!
//! - events au schema de conformite D6a (`#[contractevent]`, numero `seq`
//!   sans trou) et valeur liquidative quotidienne publiee par `checkpoint`,
//!   sans authentification.
//!
//! Plan de conception : docs/plans/2026-10-20-d5-allocateur-conception.md.
//!
//! Hors scope : frais high-water mark (D6b), parts SEP-41 transferables.

use defindex_strategy_core::DeFindexStrategyClient;
use soroban_sdk::{
    auth::{ContractContext, InvokerContractAuthEntry, SubContractInvocation},
    contract, contracterror, contractevent, contractimpl, contractmeta, contracttype,
    panic_with_error, symbol_short,
    token::TokenClient,
    vec, Address, Env, IntoVal, Symbol, Vec,
};

/// Erreurs typees du vault : contractuelles pour les integrateurs (un client
/// off-chain teste un code, pas une chaine de panique).
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum VaultError {
    // 1 (AlreadyInitialized) retire avec initialize : le constructeur ne
    // s'execute qu'une fois, au deploiement. Jamais reutilise.
    AmountMustBePositive = 2,
    DepositTooSmall = 3,
    SharesMustBePositive = 4,
    InsufficientShares = 5,
    WithdrawTooSmall = 6,
    VaultInsolvent = 7,
    ContractPaused = 8,
    MathOverflow = 9,
    // 10 (PoolReserveMissing, pool Blend direct de D1) retire : jamais reutilise.
    TooManyStrategies = 11,
    StrategyAssetMismatch = 12,
    InvalidAllocation = 13,
    DuplicateStrategy = 14,
    StrategyShortfall = 15,
    InvalidStrategyBalance = 16,
    NotAuthorized = 17,
    NothingToRebalance = 18,
    CapExceeded = 19,
    RebalanceLoss = 20,
    StrategyNotFound = 21,
    NavAlreadyPublished = 22,
}

/// Une strategie au trait DeFindex et sa place dans l'allocation, en points de
/// base de total_assets. Le plafond est fige au deploiement ; la cible se
/// reglera ensuite, toujours sous le plafond.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StrategyConfig {
    pub address: Address,
    /// Protocole sous-jacent (`blend`, `aquarius`), repris par les events
    /// `allocate` et `deallocate` : l'execution via une entite hors MiCA est
    /// enregistree avec son identite (art. 12 du reglement 2025/1140).
    pub protocol: Symbol,
    pub target_bps: u32,
    pub cap_bps: u32,
    pub active: bool,
}

// Events du schema de conformite D6a (docs/plans/2026-10-20-d6a-schema-
// events-conformite.md, sections 3, 4 et 8). Regles communes :
// - `seq` : numero d'enregistrement monotone et sans trou, propre au vault ;
//   un trou dans la suite indexee signale un event manque ;
// - `total_assets` (et `total_shares` quand l'operation les change) : valeurs
//   a la fin de l'appel, identiques sur tous les events d'un meme appel ;
// - un changement incompatible change le nom de l'event, jamais sa forme.

#[contractevent(topics = ["deposit"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DepositEvent {
    #[topic]
    pub owner: Address,
    pub assets: i128,
    pub shares: i128,
    pub seq: u64,
    pub total_assets: i128,
    pub total_shares: i128,
}

/// `assets` : ce que le client a effectivement recu, cout de sortie deduit.
#[contractevent(topics = ["withdraw"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WithdrawEvent {
    #[topic]
    pub owner: Address,
    pub shares: i128,
    pub assets: i128,
    pub seq: u64,
    pub total_assets: i128,
    pub total_shares: i128,
}

/// Versement a une strategie, decide par l'allocateur (`reason = drift`,
/// `algo = fy_alloc_v1`).
#[contractevent(topics = ["allocate"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AllocateEvent {
    #[topic]
    pub strategy: Address,
    pub protocol: Symbol,
    pub assets: i128,
    pub position_after: i128,
    pub decided_by: Address,
    pub algo: Option<Symbol>,
    pub reason: Symbol,
    pub seq: u64,
    pub total_assets: i128,
}

/// Rapatriement depuis une strategie : par l'allocateur (`reason = drift`)
/// ou pour servir un retrait que l'oisif ne couvre pas (`reason = redeem`,
/// decide par le client, sans algorithme).
#[contractevent(topics = ["deallocate"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeallocateEvent {
    #[topic]
    pub strategy: Address,
    pub protocol: Symbol,
    pub assets: i128,
    pub position_after: i128,
    pub decided_by: Address,
    pub algo: Option<Symbol>,
    pub reason: Symbol,
    pub seq: u64,
    pub total_assets: i128,
}

/// `write_off` : passage en perte decide par l'admin, a tracer comme tel.
#[contractevent(topics = ["emergency_exit"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EmergencyExitEvent {
    #[topic]
    pub strategy: Address,
    pub assets_recovered: i128,
    pub write_off: bool,
    pub decided_by: Address,
    pub seq: u64,
    pub total_assets: i128,
}

#[contractevent(topics = ["paused"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PausedEvent {
    #[topic]
    pub admin: Address,
    pub seq: u64,
}

#[contractevent(topics = ["unpaused"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnpausedEvent {
    #[topic]
    pub admin: Address,
    pub seq: u64,
}

/// `config_set` du schema, scinde par type de valeur : un event Soroban a des
/// champs de type fixe.
#[contractevent(topics = ["keeper_set"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KeeperSetEvent {
    pub old: Address,
    pub new: Address,
    pub admin: Address,
    pub seq: u64,
}

/// Cibles en points de base, dans l'ordre de `strategies()`.
#[contractevent(topics = ["targets_set"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TargetsSetEvent {
    pub old: Vec<u32>,
    pub new: Vec<u32>,
    pub admin: Address,
    pub seq: u64,
}

/// Valeur liquidative quotidienne, emise par `checkpoint` meme sans
/// operation : le releve periodique se reconstitue a partir de ces events.
#[contractevent(topics = ["nav"])]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NavEvent {
    pub total_assets: i128,
    pub total_shares: i128,
    pub seq: u64,
}

contractmeta!(
    key = "desc",
    val = "ForYield YieldVault - depot/retrait, parts proportionnelles"
);

#[contracttype]
#[derive(Clone)]
enum DataKey {
    Admin,
    Asset,
    Keeper,
    Strategies,
    DriftBps,
    Paused,
    TotalShares,
    Shares(Address),
    Seq,
    LastNavDay,
}

/// Parts mortes verrouillées au premier dépôt (jamais rachetables) : borne le
/// coût d'une attaque par inflation du prix de la première part (modèle
/// Uniswap V2 / DeFindex). En unités de 7 décimales, 1000 = 0,0001 actif.
const MINIMUM_LIQUIDITY: i128 = 1_000;

/// total_assets appelle `balance` sur chaque strategie : borner leur nombre
/// borne le cout de chaque depot et de chaque retrait.
pub(crate) const MAX_STRATEGIES: u32 = 4;

/// 100 % en points de base.
const BPS: u32 = 10_000;

/// Code de l'algorithme d'allocation (champ 41 du reglement 2025/1140) : il
/// change quand la logique de decision change, jamais pour un parametre.
const ALGO: &str = "fy_alloc_v1";

/// Secondes par jour UTC : `checkpoint` publie au plus un `nav` par jour.
const SECONDS_PER_DAY: u64 = 86_400;

/// Perte toleree, en points de base du volume deplace : la marge de
/// glissement de D4. Elle borne un reequilibrage, le manque d'un retrait
/// servi par les strategies (a la charge du client sortant) et une sortie
/// d'urgence sans passage en perte. Au-dela, tout revert.
pub(crate) const MAX_REBALANCE_LOSS_BPS: i128 = 100;

/// Ledgers par jour, fermeture nominale a 5 secondes.
const LEDGERS_PER_DAY: u32 = 17_280;

/// Plafond reseau d'une duree de vie d'entree (`max_entry_ttl`), soit environ
/// 180 jours, identique sur testnet et sur mainnet.
const MAX_ENTRY_TTL: u32 = 3_110_400;

/// En dessous de 30 jours restants, on prolonge.
pub(crate) const TTL_THRESHOLD: u32 = 30 * LEDGERS_PER_DAY;

/// Cible de prolongation : 120 jours, largement sous le plafond reseau.
pub(crate) const TTL_EXTEND_TO: u32 = 120 * LEDGERS_PER_DAY;

// Viser au-dessus du plafond reseau fait echouer la transaction entiere, et une
// cible sous le seuil ne prolongerait jamais rien. Verifie en conditions
// reelles : une prolongation posee pile sur la borne est rejetee, d'ou la
// comparaison stricte. La garde est ici plutot que dans un test, pour qu'un
// relevement des constantes casse la compilation et jamais le reseau.
const _: () = assert!(TTL_THRESHOLD < TTL_EXTEND_TO && TTL_EXTEND_TO < MAX_ENTRY_TTL);

/// Strategie sollicitee par un retrait : adresse, protocole, montant rendu,
/// position ensuite.
type Pulled = (Address, Symbol, i128, i128);

#[contract]
pub struct YieldVault;

#[contractimpl]
impl YieldVault {
    /// Constructeur : s'execute une seule fois, dans la transaction meme du
    /// deploiement. Aucune fenetre ne reste ouverte entre le deploiement et la
    /// configuration, qu'un tiers pourrait occuper avec ses propres strategies
    /// et son propre keeper (ce que permettait l'initialize de D1).
    /// `keeper` : seule cle, avec l'admin, admise a reequilibrer.
    /// `strategies` (liste immuable, ordre = ordre de liquidite des retraits) :
    /// vide = vault de garde pure, utilise par les instances sans strategie
    /// sur leur actif (ex. EURC). `drift_bps` : ecart a la cible qui declenche
    /// le reequilibrage.
    pub fn __constructor(
        env: Env,
        admin: Address,
        keeper: Address,
        asset: Address,
        strategies: Vec<StrategyConfig>,
        drift_bps: u32,
    ) {
        // La liste et les plafonds sont immuables et le constructeur a un coup :
        // une configuration fausse rendrait le vault inutilisable sans recours.
        // Echouer ici est la seule occasion de le dire.
        Self::check_strategies(&env, &asset, &strategies);
        // Le declencheur doit depasser la perte toleree par lot : sinon l'ecart
        // laisse par une perte suffirait a relancer un lot, et les pertes se
        // composeraient de lot en lot.
        if (drift_bps as i128) <= MAX_REBALANCE_LOSS_BPS || drift_bps > BPS {
            panic_with_error!(&env, VaultError::InvalidAllocation);
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Keeper, &keeper);
        env.storage().instance().set(&DataKey::Asset, &asset);
        env.storage()
            .instance()
            .set(&DataKey::Strategies, &strategies);
        env.storage().instance().set(&DataKey::DriftBps, &drift_bps);
        env.storage().instance().set(&DataKey::Paused, &false);
        env.storage().instance().set(&DataKey::TotalShares, &0i128);
    }

    /// Depose `amount` de l'actif et emet des parts au pro-rata des actifs
    /// detenus. Le transfert de tokens exige l'autorisation de `from`.
    pub fn deposit(env: Env, from: Address, amount: i128) -> i128 {
        from.require_auth();
        Self::require_not_paused(&env);
        if amount <= 0 {
            panic_with_error!(&env, VaultError::AmountMustBePositive);
        }

        let token = TokenClient::new(&env, &Self::asset(&env));
        // Actifs AVANT le transfert entrant (solde oisif + strategies) :
        // le ratio parts:actif ne doit pas inclure le montant en cours de depot.
        let assets_before = token
            .balance(&env.current_contract_address())
            .checked_add(Self::strategy_assets(&env))
            .unwrap_or_else(|| panic_with_error!(&env, VaultError::MathOverflow));

        let total_before = Self::total_shares(env.clone());
        let (shares, total) = if total_before == 0 {
            // Genese : tout l'actif deja detenu (donation comprise) entre dans
            // le total, pour que l'invariant total_parts == actifs vaille des
            // l'origine. MINIMUM_LIQUIDITY parts mortes, comptees dans le
            // total mais attribuees a personne (jamais rachetables).
            let genesis = assets_before
                .checked_add(amount)
                .unwrap_or_else(|| panic_with_error!(&env, VaultError::MathOverflow));
            if genesis <= MINIMUM_LIQUIDITY {
                panic_with_error!(&env, VaultError::DepositTooSmall);
            }
            (genesis - MINIMUM_LIQUIDITY, genesis)
        } else {
            // Des parts existent mais plus aucun actif (perte totale de
            // strategie) : refuser le depot plutot que diviser par zero.
            if assets_before == 0 {
                panic_with_error!(&env, VaultError::VaultInsolvent);
            }
            // parts = montant x total_parts / actifs_avant, tronque : l'arrondi
            // est toujours en faveur du vault (les parts existantes).
            let shares = amount
                .checked_mul(total_before)
                .unwrap_or_else(|| panic_with_error!(&env, VaultError::MathOverflow))
                / assets_before;
            if shares == 0 {
                panic_with_error!(&env, VaultError::DepositTooSmall);
            }
            (shares, total_before + shares)
        };

        // Etat d'abord, transfert ensuite (checks-effects-interactions),
        // meme convention que withdraw : aucune ecriture apres l'appel externe.
        let key = DataKey::Shares(from.clone());
        let prev: i128 = env.storage().persistent().get(&key).unwrap_or(0);
        env.storage().persistent().set(&key, &(prev + shares));
        env.storage().instance().set(&DataKey::TotalShares, &total);
        Self::extend_ttls(&env, &key);

        // Le depot reste oisif : il rejoint les strategies au prochain
        // reequilibrage, par lot.
        token.transfer(&from, env.current_contract_address(), &amount);

        DepositEvent {
            owner: from,
            assets: amount,
            shares,
            seq: Self::next_seq(&env),
            total_assets: assets_before + amount,
            total_shares: total,
        }
        .publish(&env);
        shares
    }

    /// Retire `shares` parts : burn et restitution de l'actif au pro-rata.
    pub fn withdraw(env: Env, from: Address, shares: i128) -> i128 {
        from.require_auth();
        Self::require_not_paused(&env);
        if shares <= 0 {
            panic_with_error!(&env, VaultError::SharesMustBePositive);
        }

        let key = DataKey::Shares(from.clone());
        let balance: i128 = env.storage().persistent().get(&key).unwrap_or(0);
        if balance < shares {
            panic_with_error!(&env, VaultError::InsufficientShares);
        }

        // montant = parts x actifs / total_parts, sur l'etat AVANT burn,
        // tronque : l'arrondi est toujours en faveur du vault.
        let token = TokenClient::new(&env, &Self::asset(&env));
        let idle = token.balance(&env.current_contract_address());
        let assets = idle
            .checked_add(Self::strategy_assets(&env))
            .unwrap_or_else(|| panic_with_error!(&env, VaultError::MathOverflow));
        let total_before = Self::total_shares(env.clone());
        let amount = shares
            .checked_mul(assets)
            .unwrap_or_else(|| panic_with_error!(&env, VaultError::MathOverflow))
            / total_before;
        // Un retrait qui tronque a 0 unite brulerait des parts pour rien.
        if amount == 0 {
            panic_with_error!(&env, VaultError::WithdrawTooSmall);
        }

        env.storage().persistent().set(&key, &(balance - shares));
        env.storage()
            .instance()
            .set(&DataKey::TotalShares, &(total_before - shares));
        Self::extend_ttls(&env, &key);

        // Desallocation : la part du retrait que le solde oisif ne couvre pas
        // est retiree des strategies avant de servir le client. Le cout de
        // sortie d'une strategie (frais et glissement d'un pool LP) est a la
        // charge du client sortant, dans la marge de MAX_REBALANCE_LOSS_BPS :
        // il recoit ce qui est effectivement rentre (decision du 20/10).
        let (amount, pulled, value_change) = if amount > idle {
            let (shortfall, pulled, value_change) =
                Self::pull_from_strategies(&env, &token, amount - idle);
            (amount - shortfall, pulled, value_change)
        } else {
            (amount, Vec::new(&env), 0)
        };

        token.transfer(&env.current_contract_address(), &from, &amount);

        // L'actif baisse du montant servi et, si les strategies ont ete
        // sollicitees, de leur cout de sortie.
        let total_assets = assets - amount + value_change;
        let redeem = symbol_short!("redeem");
        for (strategy, protocol, got, position_after) in pulled.iter() {
            DeallocateEvent {
                position_after,
                strategy,
                protocol,
                assets: got,
                decided_by: from.clone(),
                algo: None,
                reason: redeem.clone(),
                seq: Self::next_seq(&env),
                total_assets,
            }
            .publish(&env);
        }
        WithdrawEvent {
            owner: from,
            shares,
            assets: amount,
            seq: Self::next_seq(&env),
            total_assets,
            total_shares: total_before - shares,
        }
        .publish(&env);
        amount
    }

    /// Actif total gere : solde token oisif + valeur des strategies.
    pub fn total_assets(env: Env) -> i128 {
        TokenClient::new(&env, &Self::asset(&env))
            .balance(&env.current_contract_address())
            .checked_add(Self::strategy_assets(&env))
            .unwrap_or_else(|| panic_with_error!(&env, VaultError::MathOverflow))
    }

    /// Parts detenues par `owner`.
    pub fn shares_of(env: Env, owner: Address) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::Shares(owner))
            .unwrap_or(0)
    }

    /// Total des parts emises.
    pub fn total_shares(env: Env) -> i128 {
        env.storage()
            .instance()
            .get(&DataKey::TotalShares)
            .unwrap_or(0)
    }

    /// Strategies et leur configuration, dans l'ordre de liquidite.
    pub fn strategies(env: Env) -> Vec<StrategyConfig> {
        Self::strategy_list(&env)
    }

    /// Repartition courante : valeur de chaque strategie, en unites d'actif
    /// (0 pour une strategie sortie en urgence, hors perimetre). L'oisif se lit
    /// par le solde token du vault.
    pub fn allocation(env: Env) -> Vec<(Address, i128)> {
        let this = env.current_contract_address();
        let mut out = Vec::new(&env);
        for s in Self::strategy_list(&env).iter() {
            let value = if s.active {
                Self::strategy_balance(&env, &s.address, &this)
            } else {
                0
            };
            out.push_back((s.address, value));
        }
        out
    }

    pub fn keeper(env: Env) -> Address {
        env.storage().instance().get(&DataKey::Keeper).unwrap()
    }

    pub fn drift_bps(env: Env) -> u32 {
        env.storage().instance().get(&DataKey::DriftBps).unwrap()
    }

    /// Ramene chaque poche a sa cible, en une transaction : retraits des
    /// strategies au-dessus de leur cible d'abord, depots ensuite, dans
    /// l'ordre de la liste. Reserve au keeper et a l'admin : ils choisissent
    /// le moment, jamais la repartition, que fixent les cibles. Refuse si
    /// aucune poche ne s'ecarte de sa cible de plus de `drift_bps` points de
    /// base de total_assets. Rend les mouvements effectues, mesures au delta
    /// du solde du vault : positif = verse a la strategie, negatif = rapatrie.
    pub fn rebalance(env: Env, caller: Address) -> Vec<(Address, i128)> {
        caller.require_auth();
        if caller != Self::admin(&env) && caller != Self::keeper(env.clone()) {
            panic_with_error!(&env, VaultError::NotAuthorized);
        }
        Self::require_not_paused(&env);

        let this = env.current_contract_address();
        let token = TokenClient::new(&env, &Self::asset(&env));
        let strategies = Self::strategy_list(&env);
        let total_before = Self::total_assets(env.clone());

        // Cible et valeur courante de chaque strategie active ; l'oisif vise
        // ce qui reste.
        let mut targets: Vec<i128> = Vec::new(&env);
        let mut current: Vec<i128> = Vec::new(&env);
        let mut idle_target = total_before;
        for s in strategies.iter() {
            let target = if s.active {
                Self::bps_of(&env, total_before, s.target_bps)
            } else {
                0
            };
            idle_target -= target;
            targets.push_back(target);
            current.push_back(if s.active {
                Self::strategy_balance(&env, &s.address, &this)
            } else {
                0
            });
        }
        let idle = token.balance(&this);

        let threshold = Self::bps_of(&env, total_before, Self::drift_bps(env.clone()));
        let drifted = (idle - idle_target).abs() > threshold
            || (0..strategies.len())
                .any(|i| (current.get(i).unwrap() - targets.get(i).unwrap()).abs() > threshold);
        if total_before == 0 || !drifted {
            panic_with_error!(&env, VaultError::NothingToRebalance);
        }

        let mut moves: Vec<(Address, i128)> = Vec::new(&env);
        let mut withdrawn: i128 = 0;
        let mut deposited: i128 = 0;
        // Retraits d'abord : ils alimentent l'oisif que les depots consomment.
        for (i, s) in strategies.iter().enumerate() {
            let excess = current.get(i as u32).unwrap() - targets.get(i as u32).unwrap();
            if excess > 0 {
                let before = token.balance(&this);
                DeFindexStrategyClient::new(&env, &s.address).withdraw(&excess, &this, &this);
                let got = token.balance(&this) - before;
                withdrawn += got;
                moves.push_back((s.address.clone(), -got));
            }
        }
        for (i, s) in strategies.iter().enumerate() {
            let missing = targets.get(i as u32).unwrap() - current.get(i as u32).unwrap();
            let available = token.balance(&this);
            let amount = if missing < available {
                missing
            } else {
                available
            };
            if amount > 0 {
                // Mesure au delta du solde du vault, comme les retraits.
                let before = token.balance(&this);
                Self::strategy_deposit(&env, &token, &s.address, amount);
                let paid = before - token.balance(&this);
                deposited += paid;
                moves.push_back((s.address.clone(), paid));
            }
        }

        // Un ecart d'une unite sur un petit total franchit un seuil nul sans
        // rien laisser a deplacer : pas de lot vide.
        if moves.is_empty() {
            panic_with_error!(&env, VaultError::NothingToRebalance);
        }

        // Perte bornee : une strategie qui valorise moins qu'elle n'a recu
        // (frais d'entree, glissement, defaillance) ne passe que dans la marge.
        // Le volume deplace est le plus grand des deux flux, pas leur somme :
        // 100 retires de A puis verses a B ont deplace 100, pas 200.
        let moved = if withdrawn > deposited {
            withdrawn
        } else {
            deposited
        };
        let total_after = Self::total_assets(env.clone());
        let loss = total_before - total_after;
        if loss > 0 && loss * (BPS as i128) > MAX_REBALANCE_LOSS_BPS * moved {
            panic_with_error!(&env, VaultError::RebalanceLoss);
        }
        // Plafonds, verifies sur l'etat final, rapportes au plus grand des deux
        // totaux : une perte d'arrondi toleree ailleurs ne doit pas faire
        // franchir son plafond a une strategie dont la cible l'egale.
        let cap_base = if total_after > total_before {
            total_after
        } else {
            total_before
        };
        let mut positions: Vec<i128> = Vec::new(&env);
        for s in strategies.iter() {
            let value = if s.active {
                Self::strategy_balance(&env, &s.address, &this)
            } else {
                0
            };
            if value > Self::bps_of(&env, cap_base, s.cap_bps) {
                panic_with_error!(&env, VaultError::CapExceeded);
            }
            positions.push_back(value);
        }

        // Une strategie bouge au plus une fois par lot (au-dessus ou
        // au-dessous de sa cible, jamais les deux) : un event par mouvement,
        // dans l'ordre d'execution, avec la position finale.
        let algo = Some(Symbol::new(&env, ALGO));
        let drift = symbol_short!("drift");
        for (address, amount) in moves.iter() {
            let i = strategies
                .iter()
                .position(|s| s.address == address)
                .unwrap() as u32;
            let protocol = strategies.get(i).unwrap().protocol;
            let position_after = positions.get(i).unwrap();
            let seq = Self::next_seq(&env);
            if amount > 0 {
                AllocateEvent {
                    strategy: address,
                    protocol,
                    assets: amount,
                    position_after,
                    decided_by: caller.clone(),
                    algo: algo.clone(),
                    reason: drift.clone(),
                    seq,
                    total_assets: total_after,
                }
                .publish(&env);
            } else {
                DeallocateEvent {
                    strategy: address,
                    protocol,
                    assets: -amount,
                    position_after,
                    decided_by: caller.clone(),
                    algo: algo.clone(),
                    reason: drift.clone(),
                    seq,
                    total_assets: total_after,
                }
                .publish(&env);
            }
        }
        moves
    }

    /// Remplace la cle keeper (admin) : une cle chaude compromise ne reste pas
    /// active pour toujours.
    pub fn set_keeper(env: Env, keeper: Address) {
        let admin = Self::admin(&env);
        admin.require_auth();
        let old = Self::keeper(env.clone());
        env.storage().instance().set(&DataKey::Keeper, &keeper);
        KeeperSetEvent {
            old,
            new: keeper,
            admin,
            seq: Self::next_seq(&env),
        }
        .publish(&env);
    }

    /// Nouvelles cibles, dans l'ordre de la liste (admin). Chaque cible reste
    /// sous le plafond fixe au deploiement, leur somme sous 100 % ; une
    /// strategie sortie en urgence garde une cible nulle.
    pub fn set_targets(env: Env, targets: Vec<u32>) {
        let admin = Self::admin(&env);
        admin.require_auth();
        let mut strategies = Self::strategy_list(&env);
        if targets.len() != strategies.len() {
            panic_with_error!(&env, VaultError::InvalidAllocation);
        }
        let mut old: Vec<u32> = Vec::new(&env);
        let mut sum: u32 = 0;
        for i in 0..strategies.len() {
            let mut s = strategies.get(i).unwrap();
            old.push_back(s.target_bps);
            let target = targets.get(i).unwrap();
            if target > s.cap_bps || (!s.active && target != 0) {
                panic_with_error!(&env, VaultError::InvalidAllocation);
            }
            sum += target;
            s.target_bps = target;
            strategies.set(i, s);
        }
        if sum > BPS {
            panic_with_error!(&env, VaultError::InvalidAllocation);
        }
        env.storage()
            .instance()
            .set(&DataKey::Strategies, &strategies);
        TargetsSetEvent {
            old,
            new: targets,
            admin,
            seq: Self::next_seq(&env),
        }
        .publish(&env);
    }

    /// Sortie d'urgence (admin) : rapatrie tout ce que la strategie rend et la
    /// desactive pour de bon. Une strategie inactive sort du perimetre : plus
    /// comptee dans total_assets, plus sollicitee aux retraits, plus jamais
    /// alimentee. Fonctionne en pause : c'est quand tout est gele qu'on en a
    /// besoin.
    ///
    /// `write_off = false` : la strategie doit tout rendre, sinon rien ne
    /// change (cas d'un pool gele, dont les fonds ne sont qu'immobilises).
    /// `write_off = true` : decision explicite de passer en perte ce que la
    /// strategie ne rend pas (lecture ou retrait en echec) pour que le vault
    /// fonctionne a nouveau ; le prix de la part baisse d'autant.
    pub fn emergency_exit(env: Env, strategy: Address, write_off: bool) -> i128 {
        let admin = Self::admin(&env);
        admin.require_auth();
        let this = env.current_contract_address();
        let token = TokenClient::new(&env, &Self::asset(&env));
        let mut strategies = Self::strategy_list(&env);
        let index = strategies
            .iter()
            .position(|s| s.address == strategy && s.active)
            .unwrap_or_else(|| panic_with_error!(&env, VaultError::StrategyNotFound))
            as u32;

        let mut config = strategies.get(index).unwrap();
        config.active = false;
        config.target_bps = 0;
        strategies.set(index, config);
        env.storage()
            .instance()
            .set(&DataKey::Strategies, &strategies);

        let client = DeFindexStrategyClient::new(&env, &strategy);
        let before = token.balance(&this);
        if write_off {
            if let Ok(Ok(value)) = client.try_balance(&this) {
                if value > 0 {
                    let _ = client.try_withdraw(&value, &this, &this);
                }
            }
        } else {
            let due = Self::strategy_balance(&env, &strategy, &this);
            if due > 0 {
                client.withdraw(&due, &this, &this);
            }
            // Sans passage en perte, tout doit rentrer, au cout de sortie
            // pres (MAX_REBALANCE_LOSS_BPS) : une strategie qui retient une
            // part de ce qu'elle annonce ne la fait pas perdre en silence.
            let got = token.balance(&this) - before;
            if got * (BPS as i128) < due * (BPS as i128 - MAX_REBALANCE_LOSS_BPS) {
                panic_with_error!(&env, VaultError::StrategyShortfall);
            }
        }
        let recovered = token.balance(&this) - before;
        EmergencyExitEvent {
            strategy,
            assets_recovered: recovered,
            write_off,
            decided_by: admin,
            seq: Self::next_seq(&env),
            total_assets: Self::total_assets(env.clone()),
        }
        .publish(&env);
        recovered
    }

    /// Pause d'urgence (admin uniquement).
    pub fn pause(env: Env) {
        let admin = Self::admin(&env);
        admin.require_auth();
        env.storage().instance().set(&DataKey::Paused, &true);
        PausedEvent {
            admin,
            seq: Self::next_seq(&env),
        }
        .publish(&env);
    }

    /// Leve la pause (admin uniquement).
    pub fn unpause(env: Env) {
        let admin = Self::admin(&env);
        admin.require_auth();
        env.storage().instance().set(&DataKey::Paused, &false);
        UnpausedEvent {
            admin,
            seq: Self::next_seq(&env),
        }
        .publish(&env);
    }

    /// Publie la valeur liquidative du jour (event `nav`), au plus une fois
    /// par jour UTC de l'horodatage du ledger. Sans authentification : une
    /// tache planifiee l'appelle chaque jour et, si elle en manque un,
    /// n'importe qui peut rattraper. Le jour calendaire, plutot qu'un delai
    /// de 24 h depuis le dernier appel, evite qu'un appel en avance sur la
    /// veille fasse sauter un jour. Fonctionne en pause : la valeur reste a
    /// publier quand tout est gele. Prolonge aussi la duree de vie de
    /// l'instance.
    pub fn checkpoint(env: Env) {
        let day = env.ledger().timestamp() / SECONDS_PER_DAY;
        let last: Option<u64> = env.storage().instance().get(&DataKey::LastNavDay);
        if last.is_some_and(|last| day <= last) {
            panic_with_error!(&env, VaultError::NavAlreadyPublished);
        }
        env.storage().instance().set(&DataKey::LastNavDay, &day);
        env.storage()
            .instance()
            .extend_ttl(TTL_THRESHOLD, TTL_EXTEND_TO);
        NavEvent {
            total_assets: Self::total_assets(env.clone()),
            total_shares: Self::total_shares(env.clone()),
            seq: Self::next_seq(&env),
        }
        .publish(&env);
    }

    /// Numero d'enregistrement du dernier event emis (0 avant le premier).
    pub fn last_seq(env: Env) -> u64 {
        env.storage().instance().get(&DataKey::Seq).unwrap_or(0)
    }

    pub fn is_paused(env: Env) -> bool {
        env.storage()
            .instance()
            .get(&DataKey::Paused)
            .unwrap_or(false)
    }

    /// Reserve le numero d'enregistrement suivant : 1, 2, 3... sans trou,
    /// un par event. Un appel qui revert annule aussi son increment.
    fn next_seq(env: &Env) -> u64 {
        let seq = Self::last_seq(env.clone()) + 1;
        env.storage().instance().set(&DataKey::Seq, &seq);
        seq
    }

    fn admin(env: &Env) -> Address {
        env.storage().instance().get(&DataKey::Admin).unwrap()
    }

    fn asset(env: &Env) -> Address {
        env.storage().instance().get(&DataKey::Asset).unwrap()
    }

    fn strategy_list(env: &Env) -> Vec<StrategyConfig> {
        env.storage()
            .instance()
            .get(&DataKey::Strategies)
            .unwrap_or(Vec::new(env))
    }

    /// Valeur des strategies actives en unites d'actif : somme des `balance`
    /// rendues pour le vault. Le trait exige une valeur en actif sous-jacent, jamais
    /// en parts ; chaque strategie tronque en faveur de ses deposants.
    fn strategy_assets(env: &Env) -> i128 {
        let this = env.current_contract_address();
        let mut total: i128 = 0;
        for s in Self::strategy_list(env).iter().filter(|s| s.active) {
            let value = Self::strategy_balance(env, &s.address, &this);
            total = total
                .checked_add(value)
                .unwrap_or_else(|| panic_with_error!(env, VaultError::MathOverflow));
        }
        total
    }

    /// `bps` points de base de `amount`, tronque.
    fn bps_of(env: &Env, amount: i128, bps: u32) -> i128 {
        amount
            .checked_mul(bps as i128)
            .unwrap_or_else(|| panic_with_error!(env, VaultError::MathOverflow))
            / BPS as i128
    }

    /// Verse `amount` d'oisif a une strategie. Son `deposit` declenche un
    /// token.transfer(vault -> strategie) imbrique : l'auth d'invocateur ne
    /// couvrant que l'appel direct, ce transfert exact est pre-autorise, meme
    /// convention que `invest_in_strategy` du vault DeFindex. Le vault juge au
    /// delta de son solde : la strategie ne peut pas prendre plus.
    fn strategy_deposit(env: &Env, token: &TokenClient, strategy: &Address, amount: i128) {
        let this = env.current_contract_address();
        env.authorize_as_current_contract(vec![
            env,
            InvokerContractAuthEntry::Contract(SubContractInvocation {
                context: ContractContext {
                    contract: token.address.clone(),
                    fn_name: Symbol::new(env, "transfer"),
                    args: (this.clone(), strategy.clone(), amount).into_val(env),
                },
                sub_invocations: Vec::new(env),
            }),
        ]);
        DeFindexStrategyClient::new(env, strategy).deposit(&amount, &this);
    }

    /// `balance` d'une strategie pour le vault. Une valeur negative ferait
    /// baisser total_assets, donc emettre trop de parts a l'entrant : elle est
    /// refusee plutot que comptee.
    fn strategy_balance(env: &Env, strategy: &Address, vault: &Address) -> i128 {
        let value = DeFindexStrategyClient::new(env, strategy).balance(vault);
        if value < 0 {
            panic_with_error!(env, VaultError::InvalidStrategyBalance);
        }
        value
    }

    /// Ramene `need` unites d'actif sur le vault, strategie par strategie dans
    /// l'ordre de la liste, et rend le manque restant. Le vault juge sur le
    /// delta de son propre solde, jamais sur la valeur de retour de la
    /// strategie : une strategie qui ment sur ce qu'elle a rendu ne trompe
    /// pas le vault. Un manque est tolere jusqu'a MAX_REBALANCE_LOSS_BPS de ce
    /// qui a ete demande aux strategies (cout de sortie, a la charge du client
    /// sortant) ; au-dela, tout le retrait revert (aucune perte de parts),
    /// comme en D1 quand Blend gelait sa reserve. Rend aussi, pour les events,
    /// ce que chaque strategie sollicitee a rendu et sa position ensuite, et
    /// la variation de total_assets due au seul passage par les strategies
    /// (cout de sortie, au signe pres) : les strategies non sollicitees n'ont
    /// pas bouge, inutile de les relire.
    fn pull_from_strategies(
        env: &Env,
        token: &TokenClient,
        need: i128,
    ) -> (i128, Vec<Pulled>, i128) {
        let this = env.current_contract_address();
        let requested = need;
        let mut need = need;
        let mut pulled = Vec::new(env);
        let mut value_change: i128 = 0;
        for s in Self::strategy_list(env).iter().filter(|s| s.active) {
            if need <= 0 {
                break;
            }
            let available = Self::strategy_balance(env, &s.address, &this);
            if available == 0 {
                continue;
            }
            let client = DeFindexStrategyClient::new(env, &s.address);
            let ask = if available < need { available } else { need };
            let before = token.balance(&this);
            client.withdraw(&ask, &this, &this);
            let got = token.balance(&this) - before;
            need -= got;
            let position_after = Self::strategy_balance(env, &s.address, &this);
            value_change += got + position_after - available;
            pulled.push_back((s.address, s.protocol, got, position_after));
        }
        if need <= 0 {
            return (0, pulled, value_change);
        }
        if need * (BPS as i128) > MAX_REBALANCE_LOSS_BPS * requested {
            panic_with_error!(env, VaultError::StrategyShortfall);
        }
        (need, pulled, value_change)
    }

    /// Gardes de configuration : nombre borne, strategies actives, actif
    /// commun, `balance` lisible, pas de doublon, cible sous plafond sous
    /// 100 %, somme des cibles sous 100 % (le reste est la poche oisive).
    fn check_strategies(env: &Env, asset: &Address, strategies: &Vec<StrategyConfig>) {
        if strategies.len() > MAX_STRATEGIES {
            panic_with_error!(env, VaultError::TooManyStrategies);
        }
        let mut target_sum: u32 = 0;
        for (i, s) in strategies.iter().enumerate() {
            // Une strategie ne devient inactive que par la sortie d'urgence.
            if !s.active || s.target_bps > s.cap_bps || s.cap_bps > BPS {
                panic_with_error!(env, VaultError::InvalidAllocation);
            }
            target_sum += s.target_bps;
            for other in strategies.iter().skip(i + 1) {
                if other.address == s.address {
                    panic_with_error!(env, VaultError::DuplicateStrategy);
                }
            }
            // Une strategie sur un autre actif fausserait total_assets sans
            // bruit : son `balance` serait compte dans la mauvaise unite.
            if DeFindexStrategyClient::new(env, &s.address).asset() != *asset {
                panic_with_error!(env, VaultError::StrategyAssetMismatch);
            }
            // Sonde : une strategie dont `balance` panique ou rend une valeur
            // negative bloquerait total_assets, donc tout depot et tout
            // retrait, pour toujours.
            Self::strategy_balance(env, &s.address, &env.current_contract_address());
        }
        if target_sum > BPS {
            panic_with_error!(env, VaultError::InvalidAllocation);
        }
    }

    /// Repousse l'archivage de tout ce que l'operation vient de toucher :
    /// l'instance et le code (un seul appel couvre les deux), et l'entree de
    /// parts du porteur. Sans cela, la duree par defaut du reseau est d'environ
    /// sept jours et les entrees s'archivent en silence : la restauration est
    /// permissionless donc rien n'est perdu, mais elle est payante et elle
    /// surprend celui qui la declenche.
    fn extend_ttls(env: &Env, holder: &DataKey) {
        env.storage()
            .instance()
            .extend_ttl(TTL_THRESHOLD, TTL_EXTEND_TO);
        env.storage()
            .persistent()
            .extend_ttl(holder, TTL_THRESHOLD, TTL_EXTEND_TO);
    }

    fn require_not_paused(env: &Env) {
        if env
            .storage()
            .instance()
            .get(&DataKey::Paused)
            .unwrap_or(false)
        {
            panic_with_error!(env, VaultError::ContractPaused);
        }
    }
}

#[cfg(test)]
mod test;
#[cfg(test)]
mod test_blend;
#[cfg(test)]
mod test_invariants;
#[cfg(test)]
mod test_matrix;
#[cfg(test)]
mod test_mock_strategy;
#[cfg(test)]
mod test_props;
#[cfg(test)]
mod test_strategies;

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
//! Plan de conception : docs/plans/2026-10-20-d5-allocateur-conception.md.
//! Le reequilibrage par lot, les plafonds et la sortie d'urgence arrivent aux
//! etapes suivantes du plan ; d'ici la, aucun depot n'est alloue.
//!
//! Hors scope : frais high-water mark (D6b), parts SEP-41 transferables.

use defindex_strategy_core::DeFindexStrategyClient;
use soroban_sdk::{
    contract, contracterror, contractimpl, contractmeta, contracttype, panic_with_error,
    symbol_short, token::TokenClient, Address, Env, Vec,
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
}

/// Une strategie au trait DeFindex et sa place dans l'allocation, en points de
/// base de total_assets. Le plafond est fige au deploiement ; la cible se
/// reglera ensuite, toujours sous le plafond.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StrategyConfig {
    pub address: Address,
    pub target_bps: u32,
    pub cap_bps: u32,
    pub active: bool,
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
        if drift_bps == 0 || drift_bps > BPS {
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

        // Migration vers #[contractevent] prevue avec le schema d'events de
        // conformite (D6a) : ne pas changer la forme des events avant.
        #[allow(deprecated)]
        env.events()
            .publish((symbol_short!("deposit"), from), (amount, shares));
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
        // est retiree des strategies avant de servir le client.
        if amount > idle {
            Self::pull_from_strategies(&env, &token, amount - idle);
        }

        token.transfer(&env.current_contract_address(), &from, &amount);

        // Meme reserve que deposit : forme des events figee jusqu'a D6a.
        #[allow(deprecated)]
        env.events()
            .publish((symbol_short!("withdraw"), from), (shares, amount));
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

    /// Repartition courante : valeur de chaque strategie, en unites d'actif.
    /// L'oisif se lit par le solde token du vault.
    pub fn allocation(env: Env) -> Vec<(Address, i128)> {
        let this = env.current_contract_address();
        let mut out = Vec::new(&env);
        for s in Self::strategy_list(&env).iter() {
            let value = Self::strategy_balance(&env, &s.address, &this);
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

    /// Pause d'urgence (admin uniquement).
    pub fn pause(env: Env) {
        Self::admin(&env).require_auth();
        env.storage().instance().set(&DataKey::Paused, &true);
    }

    /// Leve la pause (admin uniquement).
    pub fn unpause(env: Env) {
        Self::admin(&env).require_auth();
        env.storage().instance().set(&DataKey::Paused, &false);
    }

    pub fn is_paused(env: Env) -> bool {
        env.storage()
            .instance()
            .get(&DataKey::Paused)
            .unwrap_or(false)
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

    /// Valeur des strategies en unites d'actif : somme des `balance` rendues
    /// pour le vault. Le trait exige une valeur en actif sous-jacent, jamais
    /// en parts ; chaque strategie tronque en faveur de ses deposants.
    fn strategy_assets(env: &Env) -> i128 {
        let this = env.current_contract_address();
        let mut total: i128 = 0;
        for s in Self::strategy_list(env).iter() {
            let value = Self::strategy_balance(env, &s.address, &this);
            total = total
                .checked_add(value)
                .unwrap_or_else(|| panic_with_error!(env, VaultError::MathOverflow));
        }
        total
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

    /// Ramene au moins `need` unites d'actif sur le vault, strategie par
    /// strategie dans l'ordre de la liste. Le vault juge sur le delta de son
    /// propre solde, jamais sur la valeur de retour de la strategie : une
    /// strategie qui ment sur ce qu'elle a rendu ne trompe pas le vault. Si
    /// les strategies ne rendent pas assez, tout le retrait revert (aucune
    /// perte de parts), comme en D1 quand Blend gelait sa reserve.
    fn pull_from_strategies(env: &Env, token: &TokenClient, need: i128) {
        let this = env.current_contract_address();
        let mut need = need;
        for s in Self::strategy_list(env).iter() {
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
            need -= token.balance(&this) - before;
        }
        if need > 0 {
            panic_with_error!(env, VaultError::StrategyShortfall);
        }
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
mod test_matrix;
#[cfg(test)]
mod test_props;
#[cfg(test)]
mod test_strategies;

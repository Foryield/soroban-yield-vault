#![no_std]
//! ForYield Soroban YieldVault (Tranche 1 / Deliverable 1).
//!
//! - depot d'un actif (USDC, via son StellarAssetContract) et emission de parts
//!   proportionnelles : parts = montant x total_parts / actifs_avant, tronque ;
//! - retrait pro-rata : montant = parts x actifs / total_parts, tronque ;
//! - les arrondis de parts et de montants sont en faveur du vault ; seule la
//!   valorisation tronquee de la position Blend peut sous-estimer les actifs
//!   d'au plus une unite brute (poussiere au benefice de l'entrant, du meme
//!   ordre que la tolerance MINIMUM_LIQUIDITY) ;
//! - MINIMUM_LIQUIDITY parts mortes au premier depot (anti-inflation) ;
//! - allocation Blend v2 (optionnelle, fixee a l'initialize) : tout depot est
//!   fourni au pool de lending, tout retrait en est servi, et total_assets
//!   valorise la position (bTokens x b_rate) - l'interet accru fait monter le
//!   prix de la part sans aucune action du vault ;
//! - pause d'urgence (admin).
//!
//! RISQUE ACCEPTE (perimetre D1) : le pool est immuable et il n'existe aucune
//! fonction de desallocation d'urgence. `pause()` bloque les nouvelles
//! operations mais ne rapatrie PAS les fonds deja fournis a Blend ; si Blend
//! gele la reserve, les retraits echouent atomiquement (aucune perte de parts)
//! jusqu'au degel. Chemin de migration/divest : Tranche 2.
//!
//! Hors scope (Tranches 2-3) : routing Soroswap/Aquarius, DeFindex,
//! frais high-water mark, parts SEP-41 transferables.

use blend_contract_sdk::pool as blend;
use soroban_sdk::{
    auth::{ContractContext, InvokerContractAuthEntry, SubContractInvocation},
    contract, contracterror, contractimpl, contractmeta, contracttype, panic_with_error,
    symbol_short,
    token::TokenClient,
    vec, Address, Env, IntoVal, Symbol, Vec,
};

/// Erreurs typees du vault : contractuelles pour les integrateurs (un client
/// off-chain teste un code, pas une chaine de panique).
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum VaultError {
    AlreadyInitialized = 1,
    AmountMustBePositive = 2,
    DepositTooSmall = 3,
    SharesMustBePositive = 4,
    InsufficientShares = 5,
    WithdrawTooSmall = 6,
    VaultInsolvent = 7,
    ContractPaused = 8,
    MathOverflow = 9,
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
    Pool,
    Paused,
    TotalShares,
    Shares(Address),
}

/// Parts mortes verrouillées au premier dépôt (jamais rachetables) : borne le
/// coût d'une attaque par inflation du prix de la première part (modèle
/// Uniswap V2 / DeFindex). En unités de 7 décimales, 1000 = 0,0001 actif.
const MINIMUM_LIQUIDITY: i128 = 1_000;

/// Types de requete Blend v2 (pool `submit`). Supply/Withdraw simples : la
/// position d'un vault qui n'emprunte jamais reste hors de la matrice de
/// liquidation (pas de SupplyCollateral).
const BLEND_REQUEST_SUPPLY: u32 = 0;
const BLEND_REQUEST_WITHDRAW: u32 = 1;

/// Le b_rate Blend est un fixed-point 12 decimales.
const SCALAR_12: i128 = 1_000_000_000_000;

#[contract]
pub struct YieldVault;

#[contractimpl]
impl YieldVault {
    /// Initialise le vault. Idempotence interdite : un second appel panique.
    /// `pool` (optionnel, immuable) : pool de lending Blend v2 vers lequel les
    /// depots sont alloues. `None` = vault de garde pure (aucune strategie),
    /// utilise par les instances sans pool sur leur actif (ex. EURC).
    pub fn initialize(env: Env, admin: Address, asset: Address, pool: Option<Address>) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic_with_error!(&env, VaultError::AlreadyInitialized);
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Asset, &asset);
        if let Some(pool) = pool {
            env.storage().instance().set(&DataKey::Pool, &pool);
        }
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
        // Actifs AVANT le transfert entrant (solde oisif + position Blend) :
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

        token.transfer(&from, env.current_contract_address(), &amount);

        // Allocation : tout depot part immediatement vers le pool Blend
        // (aucun actif oisif tant qu'une strategie est branchee).
        if let Some(pool) = Self::pool_addr(&env) {
            Self::pool_supply(&env, &pool, amount);
        }

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

        // Desallocation : la part du retrait que le solde oisif ne couvre pas
        // est retiree du pool Blend avant de servir le client.
        if amount > idle {
            if let Some(pool) = Self::pool_addr(&env) {
                Self::pool_withdraw(&env, &pool, amount - idle);
            }
        }

        token.transfer(&env.current_contract_address(), &from, &amount);

        // Meme reserve que deposit : forme des events figee jusqu'a D6a.
        #[allow(deprecated)]
        env.events()
            .publish((symbol_short!("withdraw"), from), (shares, amount));
        amount
    }

    /// Actif total gere : solde token oisif + valeur de la position Blend.
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

    fn pool_addr(env: &Env) -> Option<Address> {
        env.storage().instance().get(&DataKey::Pool)
    }

    /// Valeur de la position Blend en unites d'actif : bTokens x b_rate / 1e12,
    /// tronquee (valorisation conservatrice, l'interet non accru reste au vault).
    fn strategy_assets(env: &Env) -> i128 {
        match Self::pool_addr(env) {
            None => 0,
            Some(pool) => {
                let client = blend::Client::new(env, &pool);
                let asset = Self::asset(env);
                let reserve = client.get_reserve(&asset);
                let positions = client.get_positions(&env.current_contract_address());
                let b_tokens = positions.supply.get(reserve.config.index).unwrap_or(0);
                b_tokens
                    .checked_mul(reserve.data.b_rate)
                    .unwrap_or_else(|| panic_with_error!(&env, VaultError::MathOverflow))
                    / SCALAR_12
            }
        }
    }

    /// Fournit `amount` d'actif au pool Blend. Le `submit` du pool declenche un
    /// token.transfer(vault -> pool) imbrique : l'auth d'invocateur ne couvrant
    /// que l'appel direct, ce transfert est pre-autorise explicitement.
    fn pool_supply(env: &Env, pool: &Address, amount: i128) {
        let this = env.current_contract_address();
        let asset = Self::asset(env);
        env.authorize_as_current_contract(vec![
            env,
            InvokerContractAuthEntry::Contract(SubContractInvocation {
                context: ContractContext {
                    contract: asset.clone(),
                    fn_name: Symbol::new(env, "transfer"),
                    args: (this.clone(), pool.clone(), amount).into_val(env),
                },
                sub_invocations: Vec::new(env),
            }),
        ]);
        Self::pool_submit(env, pool, &asset, BLEND_REQUEST_SUPPLY, amount);
    }

    /// Retire `amount` d'actif du pool Blend vers le vault (transfert entrant :
    /// aucune pre-autorisation necessaire).
    fn pool_withdraw(env: &Env, pool: &Address, amount: i128) {
        let asset = Self::asset(env);
        Self::pool_submit(env, pool, &asset, BLEND_REQUEST_WITHDRAW, amount);
    }

    fn pool_submit(env: &Env, pool: &Address, asset: &Address, request_type: u32, amount: i128) {
        let this = env.current_contract_address();
        blend::Client::new(env, pool).submit(
            &this,
            &this,
            &this,
            &vec![
                env,
                blend::Request {
                    address: asset.clone(),
                    amount,
                    request_type,
                },
            ],
        );
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

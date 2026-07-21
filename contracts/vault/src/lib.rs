#![no_std]
//! ForYield Soroban YieldVault (Tranche 1 / Deliverable 1).
//!
//! - depot d'un actif (USDC, via son StellarAssetContract) et emission de parts
//!   proportionnelles : parts = montant x total_parts / actifs_avant, tronque ;
//! - retrait pro-rata : montant = parts x actifs / total_parts, tronque ;
//! - les deux arrondis sont en faveur du vault (des parts existantes) ;
//! - MINIMUM_LIQUIDITY parts mortes au premier depot (anti-inflation) ;
//! - pause d'urgence (admin).
//!
//! Tant qu'aucune strategie ne rapporte, le ratio effectif reste 1 part = 1 unite.
//! Hors scope de cet increment (suite D1 + Tranche 2) : allocation Blend v2,
//! routing Soroswap/Aquarius, DeFindex, frais high-water mark, parts SEP-41.

use soroban_sdk::{
    contract, contractimpl, contractmeta, contracttype, symbol_short, token::TokenClient, Address,
    Env,
};

contractmeta!(
    key = "desc",
    val = "ForYield YieldVault - depot/retrait, parts proportionnelles"
);

#[contracttype]
#[derive(Clone)]
enum DataKey {
    Admin,
    Asset,
    Paused,
    TotalShares,
    Shares(Address),
}

/// Parts mortes verrouillées au premier dépôt (jamais rachetables) : borne le
/// coût d'une attaque par inflation du prix de la première part (modèle
/// Uniswap V2 / DeFindex). En unités de 7 décimales, 1000 = 0,0001 actif.
const MINIMUM_LIQUIDITY: i128 = 1_000;

#[contract]
pub struct YieldVault;

#[contractimpl]
impl YieldVault {
    /// Initialise le vault. Idempotence interdite : un second appel panique.
    pub fn initialize(env: Env, admin: Address, asset: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Asset, &asset);
        env.storage().instance().set(&DataKey::Paused, &false);
        env.storage().instance().set(&DataKey::TotalShares, &0i128);
    }

    /// Depose `amount` de l'actif et emet des parts au pro-rata des actifs
    /// detenus. Le transfert de tokens exige l'autorisation de `from`.
    pub fn deposit(env: Env, from: Address, amount: i128) -> i128 {
        from.require_auth();
        Self::require_not_paused(&env);
        if amount <= 0 {
            panic!("amount must be positive");
        }

        let token = TokenClient::new(&env, &Self::asset(&env));
        // Actifs AVANT le transfert entrant : le ratio parts:actif du calcul
        // ne doit pas inclure le montant en train d'etre depose.
        let assets_before = token.balance(&env.current_contract_address());
        token.transfer(&from, &env.current_contract_address(), &amount);

        let total_before = Self::total_shares(env.clone());
        let (shares, total) = if total_before == 0 {
            // Premier depot : MINIMUM_LIQUIDITY parts mortes, comptees dans le
            // total mais attribuees a personne (jamais rachetables).
            if amount <= MINIMUM_LIQUIDITY {
                panic!("deposit too small");
            }
            (amount - MINIMUM_LIQUIDITY, amount)
        } else {
            // parts = montant x total_parts / actifs_avant, tronque : l'arrondi
            // est toujours en faveur du vault (les parts existantes).
            let shares = amount
                .checked_mul(total_before)
                .expect("share math overflow")
                / assets_before;
            if shares == 0 {
                panic!("deposit too small");
            }
            (shares, total_before + shares)
        };

        let key = DataKey::Shares(from.clone());
        let prev: i128 = env.storage().persistent().get(&key).unwrap_or(0);
        env.storage().persistent().set(&key, &(prev + shares));
        env.storage().instance().set(&DataKey::TotalShares, &total);

        env.events()
            .publish((symbol_short!("deposit"), from), (amount, shares));
        shares
    }

    /// Retire `shares` parts : burn et restitution de l'actif au pro-rata.
    pub fn withdraw(env: Env, from: Address, shares: i128) -> i128 {
        from.require_auth();
        Self::require_not_paused(&env);
        if shares <= 0 {
            panic!("shares must be positive");
        }

        let key = DataKey::Shares(from.clone());
        let balance: i128 = env.storage().persistent().get(&key).unwrap_or(0);
        if balance < shares {
            panic!("insufficient shares");
        }

        // montant = parts x actifs / total_parts, sur l'etat AVANT burn,
        // tronque : l'arrondi est toujours en faveur du vault.
        let token = TokenClient::new(&env, &Self::asset(&env));
        let assets = token.balance(&env.current_contract_address());
        let total_before = Self::total_shares(env.clone());
        let amount = shares.checked_mul(assets).expect("share math overflow") / total_before;

        env.storage().persistent().set(&key, &(balance - shares));
        env.storage()
            .instance()
            .set(&DataKey::TotalShares, &(total_before - shares));

        token.transfer(&env.current_contract_address(), &from, &amount);

        env.events()
            .publish((symbol_short!("withdraw"), from), (shares, amount));
        amount
    }

    /// Actif reellement detenu par le vault (lecture on-chain du solde token).
    pub fn total_assets(env: Env) -> i128 {
        TokenClient::new(&env, &Self::asset(&env)).balance(&env.current_contract_address())
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

    fn require_not_paused(env: &Env) {
        if env
            .storage()
            .instance()
            .get(&DataKey::Paused)
            .unwrap_or(false)
        {
            panic!("contract is paused");
        }
    }
}

#[cfg(test)]
mod test;

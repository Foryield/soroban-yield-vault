#![no_std]
//! ForYield Soroban YieldVault - MVP (Tranche 1 / Deliverable 1).
//!
//! Scope volontairement minimal pour la demo SCF Build :
//! - depot d'un actif (USDC, via son StellarAssetContract) et emission de parts 1:1 ;
//! - retrait : burn des parts et restitution de l'actif ;
//! - pause d'urgence (admin).
//!
//! Hors scope MVP (Tranches 2-3) : allocation Blend/Aquarius, routing DeFindex,
//! frais avec high-water mark, parts SEP-41 transferables, wrapper SAC EURC.
//! Le ratio parts:actif est 1:1 tant qu'aucune strategie n'est branchee.

use soroban_sdk::{
    contract, contractimpl, contractmeta, contracttype, symbol_short, token::TokenClient, Address,
    Env,
};

contractmeta!(
    key = "desc",
    val = "ForYield YieldVault MVP - deposit/withdraw, parts 1:1"
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

    /// Depose `amount` de l'actif et emet des parts (1:1 au MVP).
    /// Le transfert de tokens exige l'autorisation de `from`.
    pub fn deposit(env: Env, from: Address, amount: i128) -> i128 {
        from.require_auth();
        Self::require_not_paused(&env);
        if amount <= 0 {
            panic!("amount must be positive");
        }

        let token = TokenClient::new(&env, &Self::asset(&env));
        token.transfer(&from, &env.current_contract_address(), &amount);

        let shares = amount; // 1:1 tant qu'aucune strategie n'est branchee
        let key = DataKey::Shares(from.clone());
        let prev: i128 = env.storage().persistent().get(&key).unwrap_or(0);
        env.storage().persistent().set(&key, &(prev + shares));

        let total = Self::total_shares(env.clone()) + shares;
        env.storage().instance().set(&DataKey::TotalShares, &total);

        env.events()
            .publish((symbol_short!("deposit"), from), (amount, shares));
        shares
    }

    /// Retire `shares` parts : burn et restitution de l'actif (1:1 au MVP).
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
        env.storage().persistent().set(&key, &(balance - shares));

        let total = Self::total_shares(env.clone()) - shares;
        env.storage().instance().set(&DataKey::TotalShares, &total);

        let amount = shares; // 1:1
        let token = TokenClient::new(&env, &Self::asset(&env));
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

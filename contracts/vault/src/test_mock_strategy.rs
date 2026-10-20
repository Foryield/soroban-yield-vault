#![cfg(test)]
//! Strategie factice au trait DeFindex, partagee par les tests d'allocation
//! et d'invariants du vault. Elle garde ses fonds (modele `hodl` de DeFindex)
//! et rend pour `balance` son propre solde, quel que soit `from` : un don
//! direct sur elle simule le rendement. Ses reglages simulent une strategie
//! fautive (retenue ou surplus au retrait, `balance` force, en panne ou
//! gonfle, retrait gele, frais d'entree).

use defindex_strategy_core::{DeFindexStrategyTrait, StrategyError};
use soroban_sdk::{
    contract, contractimpl, contracttype, token::TokenClient, Address, Bytes, Env, Val, Vec,
};

#[contracttype]
enum Key {
    Asset,
    SkewBps,
    BalanceOverride,
    Broken,
    BrokenWithdraw,
    DepositFeeBps,
    InflateBps,
}

#[contract]
pub struct MockStrategy;

#[contractimpl]
impl DeFindexStrategyTrait for MockStrategy {
    fn __constructor(env: Env, asset: Address, _init_args: Vec<Val>) {
        env.storage().instance().set(&Key::Asset, &asset);
    }

    fn asset(env: Env) -> Result<Address, StrategyError> {
        Ok(env.storage().instance().get(&Key::Asset).unwrap())
    }

    fn deposit(env: Env, amount: i128, from: Address) -> Result<i128, StrategyError> {
        from.require_auth();
        let token = TokenClient::new(&env, &Self::asset(env.clone())?);
        token.transfer(&from, env.current_contract_address(), &amount);
        // Frais d'entree : une part du depot est detruite, comme une perte
        // de glissement a l'entree d'une position.
        let fee_bps: i128 = env
            .storage()
            .instance()
            .get(&Key::DepositFeeBps)
            .unwrap_or(0);
        let fee = amount * fee_bps / 10_000;
        if fee > 0 {
            token.burn(&env.current_contract_address(), &fee);
        }
        Self::balance(env, from)
    }

    fn harvest(_env: Env, _from: Address, _data: Option<Bytes>) -> Result<(), StrategyError> {
        Ok(())
    }

    fn balance(env: Env, _from: Address) -> Result<i128, StrategyError> {
        if env.storage().instance().has(&Key::Broken) {
            panic!("strategie en panne");
        }
        if let Some(value) = env.storage().instance().get(&Key::BalanceOverride) {
            return Ok(value);
        }
        let token = TokenClient::new(&env, &Self::asset(env.clone())?);
        let held = token.balance(&env.current_contract_address());
        // Valorisation gonflee : annonce plus que ce qu'elle detient.
        let inflate: i128 = env.storage().instance().get(&Key::InflateBps).unwrap_or(0);
        Ok(held + held * inflate / 10_000)
    }

    fn withdraw(env: Env, amount: i128, from: Address, to: Address) -> Result<i128, StrategyError> {
        from.require_auth();
        if env.storage().instance().has(&Key::BrokenWithdraw) {
            panic!("retrait gele");
        }
        let skew: i32 = env.storage().instance().get(&Key::SkewBps).unwrap_or(0);
        let sent = amount * (10_000 + skew as i128) / 10_000;
        let token = TokenClient::new(&env, &Self::asset(env.clone())?);
        token.transfer(&env.current_contract_address(), &to, &sent);
        if skew != 0 {
            // Ment : annonce le montant demande, pas ce qui a ete rendu.
            return Ok(amount);
        }
        Self::balance(env, from)
    }
}

#[contractimpl]
impl MockStrategy {
    /// Ecart applique a chaque retrait : negatif = retenue, positif =
    /// surplus. Le montant demande est annonce dans les deux cas.
    pub fn set_skew_bps(env: Env, bps: i32) {
        env.storage().instance().set(&Key::SkewBps, &bps);
    }

    pub fn set_balance_override(env: Env, value: i128) {
        env.storage().instance().set(&Key::BalanceOverride, &value);
    }

    pub fn break_balance(env: Env) {
        env.storage().instance().set(&Key::Broken, &true);
    }

    /// Retrait en echec, lecture intacte : un pool gele.
    pub fn break_withdraw(env: Env) {
        env.storage().instance().set(&Key::BrokenWithdraw, &true);
    }

    pub fn set_deposit_fee_bps(env: Env, bps: i128) {
        env.storage().instance().set(&Key::DepositFeeBps, &bps);
    }

    pub fn set_inflate_bps(env: Env, bps: i128) {
        env.storage().instance().set(&Key::InflateBps, &bps);
    }
}

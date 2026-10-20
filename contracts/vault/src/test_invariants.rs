#![cfg(test)]
// Montants en convention Stellar 7 decimales (X_XXXXXXX, ex. 0_1000000 = 0,1).
#![allow(clippy::inconsistent_digit_grouping, clippy::zero_prefixed_literal)]
//! Invariants D5 (proptest) : sequences aleatoires de depots, retraits,
//! rendements, reequilibrages, nouvelles cibles et sorties d'urgence sur un
//! vault a deux strategies factices, l'une qui retient une part de ses
//! retraits, l'autre a frais d'entree.
//!
//! Avant chaque operation, un modele ecrit depuis la specification predit son
//! issue exacte a partir de l'etat lu sur la chaine : montant rendu, parts
//! emises, mouvements du lot, ou refus precis. Le contrat doit la reproduire
//! a l'unite. Un refus laisse tout l'etat intact. S'y ajoutent des
//! invariants economiques, que le modele ne fait que constater : conservation
//! des tokens, prix de la part, plafonds, sortie complete de chaque porteur.

extern crate std;

use super::{
    StrategyConfig, VaultError, YieldVault, YieldVaultClient, BPS, MAX_REBALANCE_LOSS_BPS,
    MINIMUM_LIQUIDITY,
};
use crate::test_mock_strategy::{MockStrategy, MockStrategyClient};
use proptest::prelude::*;
use soroban_sdk::{
    testutils::{Address as _, EnvTestConfig},
    token::{StellarAssetClient, TokenClient},
    vec, Address, Env, Error, InvokeError, Val, Vec,
};
use std::{fmt::Debug, vec::Vec as StdVec};

const USERS: usize = 3;
const FUNDS: i128 = 10_000_0000000;
/// Plafonds figes au deploiement, dans l'ordre de la liste.
const CAPS: [u32; 2] = [9_000, 3_000];
const DRIFT_BPS: u32 = 500;
const B: i128 = BPS as i128;

#[derive(Clone, Copy, Debug)]
enum Size {
    Pct(i128),
    Units(i128),
}

#[derive(Clone, Copy, Debug)]
enum Caller {
    Admin,
    Keeper,
    Intruder,
}

#[derive(Clone, Debug)]
enum Op {
    Deposit { user: usize, amount: i128 },
    Withdraw { user: usize, size: Size },
    Yield { strategy: usize, amount: i128 },
    Rebalance { by: Caller },
    SetTargets { t0: u32, t1: u32 },
    EmergencyExit { strategy: usize, write_off: bool },
}

/// Montants d'une sequence. Un vault minuscule (quelques milliers d'unites)
/// est tire a part : c'est la que vivent les refus par arrondi (depot sans
/// part, manque au-dela de la marge sur un petit retrait ou une petite
/// sortie d'urgence), qu'un seul gros depot rendrait inatteignables.
fn amount(tiny: bool) -> BoxedStrategy<i128> {
    if tiny {
        (1i128..3_000).boxed()
    } else {
        prop_oneof![1i128..2_000, 1i128..100_0000000].boxed()
    }
}

fn op(tiny: bool) -> impl Strategy<Value = Op> {
    let size = prop_oneof![
        (1i128..=100).prop_map(Size::Pct),
        (1i128..2_000).prop_map(Size::Units)
    ];
    let caller = prop_oneof![
        3 => Just(Caller::Keeper),
        1 => Just(Caller::Admin),
        1 => Just(Caller::Intruder)
    ];
    // Des cibles quelconques, et des cibles qui allouent tout (plus d'oisif :
    // les retraits passent par les strategies), qui portent la strategie 0 a
    // son plafond (une retenue au rapatriement le fait alors franchir), ou
    // qui depassent 100 % a deux.
    let targets = prop_oneof![
        (0u32..=9_500, 0u32..=3_500),
        Just((7_000, 3_000)),
        Just((9_000, 1_000)),
        Just((9_000, 0)),
        // Chacune sous son plafond, mais leur somme au-dessus de 100 %.
        Just((8_000, 3_000)),
    ];
    prop_oneof![
        4 => (0..USERS, amount(tiny)).prop_map(|(user, amount)| Op::Deposit { user, amount }),
        3 => (0..USERS, size).prop_map(|(user, size)| Op::Withdraw { user, size }),
        2 => (0..2usize, amount(tiny))
            .prop_map(|(strategy, amount)| Op::Yield { strategy, amount }),
        3 => caller.prop_map(|by| Op::Rebalance { by }),
        1 => targets.prop_map(|(t0, t1)| Op::SetTargets { t0, t1 }),
        1 => (0..2usize, any::<bool>())
            .prop_map(|(strategy, write_off)| Op::EmergencyExit { strategy, write_off }),
    ]
}

struct World<'a> {
    env: Env,
    admin: Address,
    keeper: Address,
    intruder: Address,
    users: StdVec<Address>,
    token: TokenClient<'a>,
    vault: YieldVaultClient<'a>,
    strategies: [MockStrategyClient<'a>; 2],
    /// Ecart au retrait de chaque strategie, en points de base : la 0
    /// retient (<= 0), la 1 rend juste.
    skew: [i128; 2],
    /// Frais d'entree de chaque strategie, en points de base : seule la 1 en
    /// preleve.
    fee: [i128; 2],
}

fn world<'a>(skew_bps: i32, fee_bps: i128) -> World<'a> {
    // Pas de snapshot par cas : proptest en ecrirait un par sequence.
    let env = Env::new_with_config(EnvTestConfig {
        capture_snapshot_at_drop: false,
    });
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let keeper = Address::generate(&env);
    let intruder = Address::generate(&env);
    let asset = env
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let users: StdVec<Address> = (0..USERS).map(|_| Address::generate(&env)).collect();
    for user in &users {
        StellarAssetClient::new(&env, &asset).mint(user, &FUNDS);
    }
    let register = || {
        let args: Vec<Val> = Vec::new(&env);
        MockStrategyClient::new(&env, &env.register(MockStrategy, (asset.clone(), args)))
    };
    let strategies = [register(), register()];
    strategies[0].set_skew_bps(&skew_bps);
    strategies[1].set_deposit_fee_bps(&fee_bps);
    let config = |s: &MockStrategyClient, target_bps, cap_bps| StrategyConfig {
        address: s.address.clone(),
        target_bps,
        cap_bps,
        active: true,
    };
    let list = vec![
        &env,
        config(&strategies[0], 7_000, CAPS[0]),
        config(&strategies[1], 2_000, CAPS[1]),
    ];
    let vault_id = env.register(
        YieldVault,
        (
            admin.clone(),
            keeper.clone(),
            asset.clone(),
            list,
            DRIFT_BPS,
        ),
    );
    World {
        token: TokenClient::new(&env, &asset),
        vault: YieldVaultClient::new(&env, &vault_id),
        env,
        admin,
        keeper,
        intruder,
        users,
        strategies,
        skew: [skew_bps as i128, 0],
        fee: [0, fee_bps],
    }
}

/// Resultat d'un appel `try_` : la valeur, ou l'erreur typee du vault. Tout
/// autre echec (panique d'hote, transfert impossible, conversion) est un
/// defaut et fait echouer la propriete.
fn outcome<T, C: Debug>(r: Result<Result<T, C>, Result<Error, InvokeError>>) -> Result<T, Error> {
    match r {
        Ok(Ok(value)) => Ok(value),
        Err(Ok(error)) => Err(error),
        Ok(Err(conversion)) => panic!("conversion impossible : {conversion:?}"),
        Err(Err(invoke)) => panic!("echec hors du vault : {invoke:?}"),
    }
}

/// Compare l'issue observee a l'issue predite, refus compris.
fn assert_outcome<T: Debug + PartialEq>(
    what: &str,
    got: Result<T, Error>,
    expected: Result<T, VaultError>,
) {
    let expected = expected.map_err(Error::from);
    assert_eq!(got, expected, "{what}");
}

/// Prix de la part non decroissant, en produits croises.
fn price_did_not_decrease(before: (i128, i128), after: (i128, i128)) -> bool {
    before.1 == 0 || after.0 * before.1 >= before.0 * after.1
}

/// Tout l'etat observable : un refus doit le laisser identique.
#[derive(Debug, PartialEq)]
struct Snapshot {
    configs: StdVec<StrategyConfig>,
    idle: i128,
    holdings: [i128; 2],
    total_shares: i128,
    shares: StdVec<i128>,
    wallets: StdVec<i128>,
}

/// Ce que le modele lit de la chaine avant une operation.
struct Chain {
    configs: StdVec<StrategyConfig>,
    idle: i128,
    holdings: [i128; 2],
    total_shares: i128,
}

impl Chain {
    /// Valeur d'une strategie pour le vault : ses fonds si elle est active.
    fn value(&self, i: usize) -> i128 {
        if self.configs[i].active {
            self.holdings[i]
        } else {
            0
        }
    }

    fn assets(&self) -> i128 {
        self.idle + self.value(0) + self.value(1)
    }
}

#[derive(Default)]
struct Ledger {
    /// Tout ce qui a ete frappe hors des porteurs (rendement), et detruit
    /// (frais d'entree) : la conservation des tokens en depend.
    minted: i128,
    burned: i128,
    /// Depots, sorties, et rendement verse a une strategie encore active :
    /// seul celui-la revient aux porteurs.
    deposited: i128,
    withdrawn: i128,
    yielded: i128,
}

impl World<'_> {
    fn chain(&self) -> Chain {
        let configs: StdVec<StrategyConfig> = self.vault.strategies().iter().collect();
        Chain {
            idle: self.token.balance(&self.vault.address),
            holdings: [0, 1].map(|i| self.token.balance(&self.strategies[i].address)),
            total_shares: self.vault.total_shares(),
            configs,
        }
    }

    fn snapshot(&self) -> Snapshot {
        let chain = self.chain();
        Snapshot {
            configs: chain.configs,
            idle: chain.idle,
            holdings: chain.holdings,
            total_shares: chain.total_shares,
            shares: self.users.iter().map(|u| self.vault.shares_of(u)).collect(),
            wallets: self.users.iter().map(|u| self.token.balance(u)).collect(),
        }
    }

    fn state(&self) -> (i128, i128) {
        (self.vault.total_assets(), self.vault.total_shares())
    }

    /// Fonds rendus par la strategie i sur une demande de `ask` : le mock
    /// applique son ecart, tronque.
    fn sent(&self, i: usize, ask: i128) -> i128 {
        ask * (B + self.skew[i]) / B
    }

    /// Invariants vrais apres toute operation, acceptee ou refusee.
    fn check(&self, ledger: &Ledger) {
        let chain = self.chain();

        // total_assets = oisif + valeur des strategies actives ; une strategie
        // sortie en urgence ne compte plus, meme si elle garde des fonds.
        let allocation: StdVec<(Address, i128)> = self.vault.allocation().iter().collect();
        for (i, (address, value)) in allocation.iter().enumerate() {
            assert_eq!(*address, chain.configs[i].address);
            assert_eq!(*value, chain.value(i));
        }
        assert_eq!(self.vault.total_assets(), chain.assets());

        // Cibles sous les plafonds, plafonds inchanges, somme sous 100 %,
        // strategie inactive a cible nulle.
        let mut targets = 0;
        for (i, config) in chain.configs.iter().enumerate() {
            assert_eq!(config.cap_bps, CAPS[i]);
            assert!(config.target_bps <= config.cap_bps);
            assert!(config.active || config.target_bps == 0);
            targets += config.target_bps;
        }
        assert!(targets <= BPS);

        // Les parts emises sont celles des porteurs plus les parts mortes.
        let held: i128 = self.users.iter().map(|u| self.vault.shares_of(u)).sum();
        assert!(chain.total_shares == 0 || chain.total_shares == held + MINIMUM_LIQUIDITY);

        // Conservation : rien ne se cree ni ne se perd hors du rendement
        // frappe et des frais detruits.
        let wallets: i128 = self.users.iter().map(|u| self.token.balance(u)).sum();
        assert_eq!(
            wallets + chain.idle + chain.holdings[0] + chain.holdings[1],
            FUNDS * USERS as i128 + ledger.minted - ledger.burned
        );
    }

    fn apply(&self, op: &Op, ledger: &mut Ledger) {
        let before = self.state();
        let snapshot = self.snapshot();
        let chain = self.chain();
        let refused = match *op {
            Op::Deposit { user, amount } => self.deposit(&chain, user, amount, ledger),
            Op::Withdraw { user, size } => self.withdraw(&chain, user, size, ledger),
            Op::Yield { strategy, amount } => {
                StellarAssetClient::new(&self.env, &self.token.address)
                    .mint(&self.strategies[strategy].address, &amount);
                ledger.minted += amount;
                if chain.configs[strategy].active {
                    ledger.yielded += amount;
                }
                false
            }
            Op::Rebalance { by } => self.rebalance(&chain, by, ledger),
            Op::SetTargets { t0, t1 } => self.set_targets(&chain, t0, t1),
            Op::EmergencyExit {
                strategy,
                write_off,
            } => self.emergency_exit(&chain, strategy, write_off),
        };
        if refused {
            assert_eq!(self.snapshot(), snapshot, "un refus a laisse une trace");
        }
        // Le prix de la part ne baisse que par un lot a frais d'entree ou
        // par la retenue d'une strategie sortie en urgence.
        let may_lower = matches!(op, Op::Rebalance { .. } | Op::EmergencyExit { .. });
        assert!(may_lower || price_did_not_decrease(before, self.state()));
    }

    fn deposit(&self, chain: &Chain, user: usize, amount: i128, ledger: &mut Ledger) -> bool {
        let assets = chain.assets();
        let expected = if chain.total_shares == 0 {
            let genesis = assets + amount;
            if genesis <= MINIMUM_LIQUIDITY {
                Err(VaultError::DepositTooSmall)
            } else {
                Ok(genesis - MINIMUM_LIQUIDITY)
            }
        } else if assets == 0 {
            Err(VaultError::VaultInsolvent)
        } else {
            match amount * chain.total_shares / assets {
                0 => Err(VaultError::DepositTooSmall),
                shares => Ok(shares),
            }
        };
        let got = outcome(self.vault.try_deposit(&self.users[user], &amount));
        assert_outcome("depot", got, expected);
        if expected.is_ok() {
            ledger.deposited += amount;
        }
        expected.is_err()
    }

    /// Modele du retrait : quote-part tronquee, servie par l'oisif puis par
    /// les strategies actives dans l'ordre ; le manque reste au client dans la
    /// marge, au-dela tout revert.
    fn expected_withdrawal(&self, chain: &Chain, shares: i128) -> Result<i128, VaultError> {
        let due = shares * chain.assets() / chain.total_shares;
        if due == 0 {
            return Err(VaultError::WithdrawTooSmall);
        }
        if due <= chain.idle {
            return Ok(due);
        }
        let requested = due - chain.idle;
        let mut need = requested;
        for i in 0..2 {
            if need <= 0 {
                break;
            }
            let available = chain.value(i);
            if available == 0 {
                continue;
            }
            need -= self.sent(i, available.min(need));
        }
        if need > 0 && need * B > MAX_REBALANCE_LOSS_BPS * requested {
            return Err(VaultError::StrategyShortfall);
        }
        Ok(due - need.max(0))
    }

    fn withdraw(&self, chain: &Chain, user: usize, size: Size, ledger: &mut Ledger) -> bool {
        let user = &self.users[user];
        let held = self.vault.shares_of(user);
        let shares = match size {
            Size::Pct(pct) => held * pct / 100,
            Size::Units(units) => units.min(held),
        };
        if shares == 0 {
            return false;
        }
        let expected = self.expected_withdrawal(chain, shares);
        let wallet = self.token.balance(user);
        let got = outcome(self.vault.try_withdraw(user, &shares));
        assert_outcome("retrait", got, expected);
        if let Ok(paid) = expected {
            assert_eq!(self.token.balance(user), wallet + paid);
            ledger.withdrawn += paid;
        }
        expected.is_err()
    }

    /// Modele du lot : cibles sur le total avant, retraits des poches en
    /// exces, puis depots dans l'ordre limites a l'oisif, perte bornee par le
    /// volume deplace, plafonds sur le plus grand des deux totaux.
    fn expected_rebalance(
        &self,
        chain: &Chain,
        by: Caller,
    ) -> (Result<StdVec<(Address, i128)>, VaultError>, i128) {
        if let Caller::Intruder = by {
            return (Err(VaultError::NotAuthorized), 0);
        }
        let total = chain.assets();
        let target = |i: usize| {
            if chain.configs[i].active {
                total * chain.configs[i].target_bps as i128 / B
            } else {
                0
            }
        };
        let current = [chain.value(0), chain.value(1)];
        let targets = [target(0), target(1)];
        let threshold = total * DRIFT_BPS as i128 / B;
        let idle_target = total - targets[0] - targets[1];
        let drifted = (chain.idle - idle_target).abs() > threshold
            || (0..2).any(|i| (current[i] - targets[i]).abs() > threshold);
        if total == 0 || !drifted {
            return (Err(VaultError::NothingToRebalance), 0);
        }

        let mut idle = chain.idle;
        let mut holdings = chain.holdings;
        let (mut moves, mut withdrawn, mut deposited, mut burned) = (StdVec::new(), 0, 0, 0);
        for i in 0..2 {
            let excess = current[i] - targets[i];
            if excess > 0 {
                let got = self.sent(i, excess);
                idle += got;
                holdings[i] -= got;
                withdrawn += got;
                moves.push((chain.configs[i].address.clone(), -got));
            }
        }
        for i in 0..2 {
            let amount = (targets[i] - current[i]).min(idle);
            if amount > 0 {
                let fee = amount * self.fee[i] / B;
                idle -= amount;
                holdings[i] += amount - fee;
                deposited += amount;
                burned += fee;
                moves.push((chain.configs[i].address.clone(), amount));
            }
        }
        if moves.is_empty() {
            return (Err(VaultError::NothingToRebalance), 0);
        }
        let after = Chain {
            configs: chain.configs.clone(),
            idle,
            holdings,
            total_shares: chain.total_shares,
        };
        let loss = total - after.assets();
        if loss > 0 && loss * B > MAX_REBALANCE_LOSS_BPS * withdrawn.max(deposited) {
            return (Err(VaultError::RebalanceLoss), 0);
        }
        let base = total.max(after.assets());
        for i in 0..2 {
            if after.value(i) > base * chain.configs[i].cap_bps as i128 / B {
                return (Err(VaultError::CapExceeded), 0);
            }
        }
        (Ok(moves), burned)
    }

    fn rebalance(&self, chain: &Chain, by: Caller, ledger: &mut Ledger) -> bool {
        let caller = match by {
            Caller::Admin => &self.admin,
            Caller::Keeper => &self.keeper,
            Caller::Intruder => &self.intruder,
        };
        let (expected, burned) = self.expected_rebalance(chain, by);
        let got = outcome(self.vault.try_rebalance(caller)).map(|m| m.iter().collect());
        assert_outcome("reequilibrage", got, expected.clone());
        if expected.is_err() {
            return true;
        }
        ledger.burned += burned;

        // Constats economiques du lot accepte : aucune part emise, perte
        // egale aux frais detruits, et sans frais ni retenue chaque poche
        // exactement a sa cible, plafond respecte sur le total d'apres.
        let after = self.chain();
        assert_eq!(after.total_shares, chain.total_shares);
        assert_eq!(chain.assets() - after.assets(), burned);
        if self.fee[1] == 0 && self.skew[0] == 0 {
            for i in 0..2 {
                let config = &chain.configs[i];
                let target = if config.active {
                    chain.assets() * config.target_bps as i128 / B
                } else {
                    0
                };
                assert_eq!(after.value(i), target);
                assert!(after.value(i) <= after.assets() * config.cap_bps as i128 / B);
            }
        }
        false
    }

    fn set_targets(&self, chain: &Chain, t0: u32, t1: u32) -> bool {
        let valid = t0 <= CAPS[0]
            && t1 <= CAPS[1]
            && t0 + t1 <= BPS
            && (chain.configs[0].active || t0 == 0)
            && (chain.configs[1].active || t1 == 0);
        let expected = if valid {
            Ok(())
        } else {
            Err(VaultError::InvalidAllocation)
        };
        let got = outcome(self.vault.try_set_targets(&vec![&self.env, t0, t1]));
        assert_outcome("cibles", got, expected);
        if valid {
            let configs = self.vault.strategies();
            assert_eq!(configs.get(0).unwrap().target_bps, t0);
            assert_eq!(configs.get(1).unwrap().target_bps, t1);
        }
        !valid
    }

    fn emergency_exit(&self, chain: &Chain, strategy: usize, write_off: bool) -> bool {
        let address = &self.strategies[strategy].address;
        let expected = if !chain.configs[strategy].active {
            Err(VaultError::StrategyNotFound)
        } else {
            let due = chain.holdings[strategy];
            let got = if due > 0 { self.sent(strategy, due) } else { 0 };
            if !write_off && got * B < due * (B - MAX_REBALANCE_LOSS_BPS) {
                Err(VaultError::StrategyShortfall)
            } else {
                Ok(got)
            }
        };
        let got = outcome(self.vault.try_emergency_exit(address, &write_off));
        assert_outcome("sortie d'urgence", got, expected);
        if expected.is_ok() {
            let config = self.vault.strategies().get(strategy as u32).unwrap();
            assert!(!config.active && config.target_bps == 0);
        }
        expected.is_err()
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    /// Sur toute sequence, chaque issue est celle du modele et les invariants
    /// tiennent a chaque pas ; a la fin, la retenue levee, chaque porteur sort
    /// en entier (un reliquat ne reste que s'il vaut moins d'une unite), et
    /// l'ensemble des sorties ne depasse jamais les depots plus le rendement
    /// verse aux strategies actives.
    #[test]
    fn invariants_hold_over_any_sequence(
        skew_bps in prop_oneof![Just(0i32), -90i32..0],
        fee_bps in prop_oneof![Just(0i128), 1i128..=150],
        ops in any::<bool>().prop_flat_map(|tiny| proptest::collection::vec(op(tiny), 1..30)),
    ) {
        let mut w = world(skew_bps, fee_bps);
        let mut ledger = Ledger::default();
        for op in &ops {
            w.apply(op, &mut ledger);
            w.check(&ledger);
        }

        w.strategies[0].set_skew_bps(&0);
        w.skew[0] = 0;
        for user in 0..USERS {
            let shares = w.vault.shares_of(&w.users[user]);
            if shares > 0 {
                w.withdraw(&w.chain(), user, Size::Pct(100), &mut ledger);
                let left = w.vault.shares_of(&w.users[user]);
                let (assets, total) = w.state();
                prop_assert!(left == 0 || left * assets / total == 0);
            }
            w.check(&ledger);
        }
        prop_assert!(ledger.withdrawn <= ledger.deposited + ledger.yielded);
    }
}

#![cfg(test)]
//! Frais D6b (plan docs/plans/2026-10-23-d6b-frais.md) : frais de gestion au
//! prorata du temps, commission de performance au-dessus du high-water mark,
//! parts emises a la tresorerie, events `fee_minted`, `config_set` et
//! `treasury_set`. Les sequences aleatoires sont dans test_invariants.rs.

use super::{
    ConfigSetEvent, FeeMintedEvent, NavEvent, StrategyConfig, TreasurySetEvent, VaultError,
    YieldVault, YieldVaultClient, PRICE_SCALE, SECONDS_PER_YEAR,
};
use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Events, IssuerFlags, Ledger as _},
    token::{StellarAssetClient, TokenClient},
    Address, Env, Event as _, Vec,
};

const DAY: u64 = 86_400;

struct Fixture<'a> {
    env: Env,
    admin: Address,
    user: Address,
    treasury: Address,
    token: TokenClient<'a>,
    vault: YieldVaultClient<'a>,
}

fn setup<'a>() -> Fixture<'a> {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let treasury = Address::generate(&env);
    let sac = env.register_stellar_asset_contract_v2(admin.clone());
    sac.issuer().set_flag(IssuerFlags::ClawbackEnabledFlag);
    let token = TokenClient::new(&env, &sac.address());
    StellarAssetClient::new(&env, &sac.address()).mint(&user, &10_000_000);
    let vault_id = env.register(
        YieldVault,
        (
            admin.clone(),
            admin.clone(),
            sac.address(),
            Vec::<StrategyConfig>::new(&env),
            500u32,
        ),
    );
    Fixture {
        vault: YieldVaultClient::new(&env, &vault_id),
        env,
        admin,
        user,
        treasury,
        token,
    }
}

/// Rendement simule : l'actif arrive sur le vault sans emission de parts.
fn donate(f: &Fixture, amount: i128) {
    StellarAssetClient::new(&f.env, &f.token.address).mint(&f.vault.address, &amount);
}

fn lose(f: &Fixture, amount: i128) {
    StellarAssetClient::new(&f.env, &f.token.address).clawback(&f.vault.address, &amount);
}

fn at(f: &Fixture, timestamp: u64) {
    f.env.ledger().set_timestamp(timestamp);
}

/// Noms des events du vault emis par le dernier appel, dans l'ordre.
fn names(f: &Fixture) -> std::vec::Vec<std::string::String> {
    use soroban_sdk::xdr::{ContractEventBody, ScVal};
    f.env
        .events()
        .all()
        .filter_by_contract(&f.vault.address)
        .events()
        .iter()
        .map(|e| {
            let ContractEventBody::V0(body) = &e.body;
            match &body.topics[0] {
                ScVal::Symbol(name) => name.to_utf8_string_lossy(),
                other => panic!("nom d'event inattendu : {other:?}"),
            }
        })
        .collect()
}

extern crate std;

#[test]
fn set_fees_records_old_and_new_values() {
    let f = setup();

    f.vault.set_fees(&f.treasury, &2_000, &200);

    assert_eq!(
        f.env.events().all().filter_by_contract(&f.vault.address),
        [
            ConfigSetEvent {
                key: symbol_short!("perf_bps"),
                old: 0,
                new: 2_000,
                admin: f.admin.clone(),
                seq: 1,
            }
            .to_xdr(&f.env, &f.vault.address),
            ConfigSetEvent {
                key: symbol_short!("mgmt_bps"),
                old: 0,
                new: 200,
                admin: f.admin.clone(),
                seq: 2,
            }
            .to_xdr(&f.env, &f.vault.address),
            TreasurySetEvent {
                old: None,
                new: f.treasury.clone(),
                admin: f.admin.clone(),
                seq: 3,
            }
            .to_xdr(&f.env, &f.vault.address),
        ]
    );

    // Seul ce qui change est enregistre.
    let next = Address::generate(&f.env);
    f.vault.set_fees(&next, &2_000, &100);
    assert_eq!(
        f.env.events().all().filter_by_contract(&f.vault.address),
        [
            ConfigSetEvent {
                key: symbol_short!("mgmt_bps"),
                old: 200,
                new: 100,
                admin: f.admin.clone(),
                seq: 4,
            }
            .to_xdr(&f.env, &f.vault.address),
            TreasurySetEvent {
                old: Some(f.treasury.clone()),
                new: next,
                admin: f.admin.clone(),
                seq: 5,
            }
            .to_xdr(&f.env, &f.vault.address),
        ]
    );
}

#[test]
fn fee_config_is_bounded() {
    let f = setup();
    for (treasury, perf, mgmt) in [
        (f.treasury.clone(), 3_001, 0),
        (f.treasury.clone(), 0, 501),
        (f.vault.address.clone(), 2_000, 200),
    ] {
        let err = f
            .vault
            .try_set_fees(&treasury, &perf, &mgmt)
            .unwrap_err()
            .unwrap();
        assert_eq!(err, VaultError::InvalidFeeConfig.into());
    }
    assert_eq!(f.vault.fees(), None);
    // Les plafonds eux-memes passent.
    f.vault.set_fees(&f.treasury, &3_000, &500);
}

#[test]
fn set_fees_requires_the_admin() {
    let f = setup();
    f.env.set_auths(&[]);
    assert!(f.vault.try_set_fees(&f.treasury, &2_000, &200).is_err());
}

#[test]
fn management_fee_accrues_pro_rata_temporis() {
    let f = setup();
    f.vault.deposit(&f.user, &1_000_000);
    f.vault.set_fees(&f.treasury, &0, &200);
    at(&f, SECONDS_PER_YEAR);

    f.vault.checkpoint();

    // 2 % d'un an sur 1 000 000 : 20 000, soit 20 000 x 1 000 000 / 980 000
    // parts, tronque.
    assert_eq!(
        f.env.events().all().filter_by_contract(&f.vault.address),
        [
            FeeMintedEvent {
                kind: symbol_short!("mgmt"),
                fee_shares: 20_408,
                fee_assets: 20_000,
                recipient: f.treasury.clone(),
                rate_bps: 200,
                hwm_before: PRICE_SCALE,
                hwm_after: PRICE_SCALE,
                period_start: 0,
                period_end: SECONDS_PER_YEAR,
                seq: 4,
                total_assets: 1_000_000,
                total_shares: 1_020_408,
            }
            .to_xdr(&f.env, &f.vault.address),
            NavEvent {
                total_assets: 1_000_000,
                total_shares: 1_020_408,
                seq: 5,
            }
            .to_xdr(&f.env, &f.vault.address),
        ]
    );
    // La tresorerie rachete ses parts : elles valent les frais, a la
    // troncature pres, jamais plus.
    assert_eq!(f.vault.withdraw(&f.treasury, &20_408), 19_999);
    assert_eq!(f.token.balance(&f.treasury), 19_999);
}

#[test]
fn performance_fee_is_charged_above_the_high_water_mark_only() {
    let f = setup();
    f.vault.deposit(&f.user, &1_000_000);
    f.vault.set_fees(&f.treasury, &2_000, &0);
    donate(&f, 100_000);
    at(&f, DAY);

    f.vault.checkpoint();

    // Gain de 100 000, commission de 20 000 ; HWM au prix apres prelevement.
    let fee = FeeMintedEvent {
        kind: symbol_short!("perf"),
        fee_shares: 18_518,
        fee_assets: 20_000,
        recipient: f.treasury.clone(),
        rate_bps: 2_000,
        hwm_before: PRICE_SCALE,
        hwm_after: 1_080_000_549,
        period_start: 0,
        period_end: DAY,
        seq: 4,
        total_assets: 1_100_000,
        total_shares: 1_018_518,
    };
    assert_eq!(
        f.env.events().all().filter_by_contract(&f.vault.address),
        [
            fee.clone().to_xdr(&f.env, &f.vault.address),
            NavEvent {
                total_assets: 1_100_000,
                total_shares: 1_018_518,
                seq: 5,
            }
            .to_xdr(&f.env, &f.vault.address),
        ]
    );
    assert_eq!(f.vault.fees().unwrap().hwm, 1_080_000_549);

    // Meme gain, deja facture : rien.
    at(&f, 2 * DAY);
    f.vault.checkpoint();
    assert_eq!(names(&f), ["nav"]);

    // Perte, puis remontee jusqu'au HWM : rien. Le HWM ne baisse pas.
    lose(&f, 100_000);
    at(&f, 3 * DAY);
    f.vault.checkpoint();
    assert_eq!(names(&f), ["nav"]);
    donate(&f, 100_000);
    at(&f, 4 * DAY);
    f.vault.checkpoint();
    assert_eq!(names(&f), ["nav"]);
    assert_eq!(f.vault.fees().unwrap().hwm, 1_080_000_549);

    // Au-dela, seul l'excedent paie.
    donate(&f, 50_000);
    at(&f, 5 * DAY);
    f.vault.checkpoint();
    // 20 % des 50 000 au-dessus du HWM, a la troncature pres, sur une
    // periode qui part du prelevement precedent.
    assert_eq!(
        f.env.events().all().filter_by_contract(&f.vault.address),
        [
            FeeMintedEvent {
                fee_shares: 8_933,
                fee_assets: 9_999,
                hwm_before: 1_080_000_549,
                hwm_after: 1_119_274_787,
                period_start: DAY,
                period_end: 5 * DAY,
                seq: 9,
                total_assets: 1_150_000,
                total_shares: 1_027_451,
                ..fee
            }
            .to_xdr(&f.env, &f.vault.address),
            NavEvent {
                total_assets: 1_150_000,
                total_shares: 1_027_451,
                seq: 10,
            }
            .to_xdr(&f.env, &f.vault.address),
        ]
    );
}

#[test]
fn both_fees_are_split_in_one_transaction() {
    let f = setup();
    f.vault.deposit(&f.user, &1_000_000);
    f.vault.set_fees(&f.treasury, &2_000, &200);
    donate(&f, 100_000);
    at(&f, SECONDS_PER_YEAR);

    f.vault.checkpoint();

    // Gestion d'abord, performance ensuite sur le prix apres gestion.
    let events = f.env.events().all().filter_by_contract(&f.vault.address);
    assert_eq!(names(&f), ["fee_minted", "fee_minted", "nav"]);
    let total_shares = 1_000_000 + 20_408 + 14_983;
    assert_eq!(
        events,
        [
            FeeMintedEvent {
                kind: symbol_short!("mgmt"),
                fee_shares: 20_408,
                fee_assets: 22_000,
                recipient: f.treasury.clone(),
                rate_bps: 200,
                hwm_before: PRICE_SCALE,
                hwm_after: PRICE_SCALE,
                period_start: 0,
                period_end: SECONDS_PER_YEAR,
                seq: 5,
                total_assets: 1_100_000,
                total_shares,
            }
            .to_xdr(&f.env, &f.vault.address),
            FeeMintedEvent {
                kind: symbol_short!("perf"),
                fee_shares: 14_983,
                fee_assets: 15_918,
                recipient: f.treasury.clone(),
                rate_bps: 2_000,
                hwm_before: PRICE_SCALE,
                hwm_after: 1_062_400_581,
                period_start: 0,
                period_end: SECONDS_PER_YEAR,
                seq: 6,
                total_assets: 1_100_000,
                total_shares,
            }
            .to_xdr(&f.env, &f.vault.address),
            NavEvent {
                total_assets: 1_100_000,
                total_shares,
                seq: 7,
            }
            .to_xdr(&f.env, &f.vault.address),
        ]
    );
}

#[test]
fn an_entrant_pays_nothing_on_a_gain_made_before_it_joined() {
    let f = setup();
    f.vault.deposit(&f.user, &1_000_000);
    f.vault.set_fees(&f.treasury, &2_000, &0);
    donate(&f, 100_000);
    let entrant = Address::generate(&f.env);
    StellarAssetClient::new(&f.env, &f.token.address).mint(&entrant, &110_000);

    // Le gain des porteurs presents est facture avant d'emettre ses parts.
    let shares = f.vault.deposit(&entrant, &110_000);
    assert_eq!(names(&f), ["fee_minted", "deposit"]);
    assert_eq!(shares, 101_851);

    // Il ressort aussitot avec son depot, a la troncature pres.
    let back = f.vault.withdraw(&entrant, &shares);
    assert!(back >= 110_000 - 2, "{back}");
}

#[test]
fn a_leaver_pays_its_share_before_leaving() {
    let f = setup();
    f.vault.deposit(&f.user, &1_000_000);
    f.vault.set_fees(&f.treasury, &2_000, &0);
    donate(&f, 100_000);

    // 999 000 parts sur 1 000 000 : 1 100 000 x 999 000 / 1 018 518.
    let paid = f.vault.withdraw(&f.user, &999_000);

    assert_eq!(names(&f), ["fee_minted", "withdraw"]);
    assert_eq!(paid, 1_078_920);
    assert_eq!(f.vault.shares_of(&f.treasury), 18_518);
}

#[test]
fn the_treasury_keeps_fresh_fee_shares_when_it_withdraws() {
    let f = setup();
    StellarAssetClient::new(&f.env, &f.token.address).mint(&f.treasury, &1_000_000);
    f.vault.deposit(&f.treasury, &1_000_000);
    f.vault.set_fees(&f.treasury, &2_000, &0);
    donate(&f, 100_000);

    f.vault.withdraw(&f.treasury, &500_000);

    // Ses 999 000 parts, moins 500 000 rachetees, plus 18 518 de frais.
    assert_eq!(f.vault.shares_of(&f.treasury), 517_518);
    assert_eq!(f.vault.total_shares(), 518_518);
}

#[test]
fn changes_never_apply_to_the_past() {
    let f = setup();
    f.vault.deposit(&f.user, &1_000_000);
    f.vault.set_fees(&f.treasury, &0, &200);

    // Un an au taux de 2 %, puis passage a 1 % : l'annee ecoulee est
    // facturee a 2 %, dans le meme appel, avant le changement.
    at(&f, SECONDS_PER_YEAR);
    f.vault.set_fees(&f.treasury, &0, &100);
    assert_eq!(names(&f), ["fee_minted", "config_set"]);
    assert_eq!(f.vault.shares_of(&f.treasury), 20_408);

    // Un gain fait sans commission de performance n'est pas facture quand
    // elle s'active : le HWM monte au prix courant.
    donate(&f, 100_000);
    f.vault.set_fees(&f.treasury, &2_000, &100);
    let price = 1_100_000 * PRICE_SCALE / 1_020_408;
    assert_eq!(f.vault.fees().unwrap().hwm, price);
    at(&f, SECONDS_PER_YEAR + DAY);
    f.vault.checkpoint();
    // Seuls les frais de gestion du jour passent : le HWM n'a pas bouge.
    assert_eq!(names(&f), ["fee_minted", "nav"]);
    assert_eq!(f.vault.fees().unwrap().hwm, price);
}

#[test]
fn frequent_operations_do_not_erase_the_management_fee() {
    let f = setup();
    f.vault.deposit(&f.user, &10_000);
    f.vault.set_fees(&f.treasury, &0, &500);

    // Toutes les dix minutes, un depot : chaque fois, les frais tronquent a
    // zero, et le temps reste du.
    for i in 1..=50 {
        at(&f, i * 600);
        f.vault.deposit(&f.user, &1);
    }
    assert_eq!(f.vault.shares_of(&f.treasury), 0);
    assert_eq!(f.vault.fees().unwrap().accrued_at, 0);

    // Un an apres le reglage : 5 % de l'actif, sur toute la duree.
    at(&f, SECONDS_PER_YEAR);
    f.vault.checkpoint();
    let value = f.vault.shares_of(&f.treasury) * f.vault.total_assets() / f.vault.total_shares();
    assert!((500..=502).contains(&value), "{value}");
}

#[test]
fn one_charge_covers_at_most_a_year() {
    let f = setup();
    f.vault.deposit(&f.user, &1_000_000);
    f.vault.set_fees(&f.treasury, &0, &200);
    at(&f, 3 * SECONDS_PER_YEAR);

    f.vault.checkpoint();

    assert_eq!(f.vault.shares_of(&f.treasury), 20_408);
}

#[test]
fn no_fee_accrues_before_the_first_depositor() {
    let f = setup();
    f.vault.set_fees(&f.treasury, &2_000, &200);
    assert_eq!(f.vault.fees().unwrap().hwm, PRICE_SCALE);
    at(&f, SECONDS_PER_YEAR);

    f.vault.deposit(&f.user, &1_000_000);

    assert_eq!(names(&f), ["deposit"]);
    assert_eq!(f.vault.fees().unwrap().accrued_at, SECONDS_PER_YEAR);
}

#[test]
fn holders_at_a_loss_pay_nothing_on_their_recovery() {
    let f = setup();
    f.vault.deposit(&f.user, &1_000_000);
    lose(&f, 100_000);

    // Premier reglage sous le prix de genese : le HWM reste a une unite par
    // part.
    f.vault.set_fees(&f.treasury, &2_000, &0);
    assert_eq!(f.vault.fees().unwrap().hwm, PRICE_SCALE);
    donate(&f, 100_000);
    at(&f, DAY);
    f.vault.checkpoint();

    assert_eq!(names(&f), ["nav"]);
}

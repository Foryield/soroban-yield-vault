#![cfg(test)]
use super::{YieldVault, YieldVaultClient};
use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Events, IssuerFlags},
    token::{StellarAssetClient, TokenClient},
    vec, Address, Env, IntoVal,
};

struct Fixture<'a> {
    env: Env,
    admin: Address,
    user: Address,
    token: TokenClient<'a>,
    vault: YieldVaultClient<'a>,
}

fn setup<'a>(initial_mint: i128) -> Fixture<'a> {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let user = Address::generate(&env);

    // USDC de test : StellarAssetContract sous notre cle d'admin.
    // Clawback active a l'emission pour pouvoir simuler une perte de strategie
    // dans les tests (sans effet sur les operations normales).
    let sac = env.register_stellar_asset_contract_v2(admin.clone());
    sac.issuer().set_flag(IssuerFlags::ClawbackEnabledFlag);
    let asset = sac.address();
    let token = TokenClient::new(&env, &asset);
    StellarAssetClient::new(&env, &asset).mint(&user, &initial_mint);

    let vault_id = env.register(YieldVault, ());
    let vault = YieldVaultClient::new(&env, &vault_id);
    vault.initialize(&admin, &asset, &None);

    Fixture {
        env,
        admin,
        user,
        token,
        vault,
    }
}

/// Simule un rendement accru : l'actif arrive directement sur le vault
/// (comme des interets Blend), sans emission de parts.
fn donate_yield(f: &Fixture, amount: i128) {
    StellarAssetClient::new(&f.env, &f.token.address).mint(&f.vault.address, &amount);
}

/// Cree un second deposant finance.
fn fund_user(f: &Fixture, amount: i128) -> Address {
    let user = Address::generate(&f.env);
    StellarAssetClient::new(&f.env, &f.token.address).mint(&user, &amount);
    user
}

/// Simule une perte de strategie : l'actif du vault est detruit (clawback
/// emetteur), comme si une allocation externe avait perdu de la valeur.
fn simulate_loss(f: &Fixture, amount: i128) {
    StellarAssetClient::new(&f.env, &f.token.address).clawback(&f.vault.address, &amount);
}

#[test]
fn deposit_mints_shares_and_moves_tokens() {
    let f = setup(10_000);

    let shares = f.vault.deposit(&f.user, &4_000);

    assert_eq!(shares, 3_000); // 4 000 - 1 000 parts mortes (premier depot)
    assert_eq!(f.vault.shares_of(&f.user), 3_000);
    assert_eq!(f.vault.total_shares(), 4_000);
    assert_eq!(f.token.balance(&f.user), 6_000);
    assert_eq!(f.vault.total_assets(), 4_000);
}

#[test]
fn withdraw_burns_shares_and_returns_tokens() {
    let f = setup(10_000);
    f.vault.deposit(&f.user, &4_000);

    let amount = f.vault.withdraw(&f.user, &1_500);

    assert_eq!(amount, 1_500); // sans rendement, 1 part vaut 1 unite d'actif
    assert_eq!(f.vault.shares_of(&f.user), 1_500);
    assert_eq!(f.vault.total_shares(), 2_500);
    assert_eq!(f.token.balance(&f.user), 7_500);
    assert_eq!(f.vault.total_assets(), 2_500);
}

#[test]
fn full_withdraw_leaves_only_dead_shares() {
    let f = setup(10_000);
    f.vault.deposit(&f.user, &5_000);
    f.vault.withdraw(&f.user, &4_000);

    // Les 1 000 parts mortes et leur contre-valeur restent dans le vault.
    assert_eq!(f.vault.shares_of(&f.user), 0);
    assert_eq!(f.vault.total_shares(), 1_000);
    assert_eq!(f.token.balance(&f.user), 9_000);
    assert_eq!(f.vault.total_assets(), 1_000);
}

#[test]
#[should_panic(expected = "Error(Contract, #5)")]
fn withdraw_beyond_balance_panics() {
    let f = setup(10_000);
    f.vault.deposit(&f.user, &2_000);
    f.vault.withdraw(&f.user, &1_001); // 2 000 deposes => 1 000 parts detenues
}

#[test]
#[should_panic(expected = "Error(Contract, #2)")]
fn deposit_zero_panics() {
    let f = setup(1_000);
    f.vault.deposit(&f.user, &0);
}

#[test]
#[should_panic(expected = "Error(Contract, #8)")]
fn deposit_while_paused_panics() {
    let f = setup(1_000);
    f.vault.pause();
    f.vault.deposit(&f.user, &100);
}

#[test]
fn unpause_restores_deposit() {
    let f = setup(10_000);
    f.vault.pause();
    f.vault.unpause();
    let shares = f.vault.deposit(&f.user, &2_000);
    assert_eq!(shares, 1_000); // 2 000 - 1 000 parts mortes
}

#[test]
#[should_panic(expected = "Error(Contract, #1)")]
fn double_initialize_panics() {
    let f = setup(1_000);
    f.vault.initialize(&f.admin, &f.user, &None); // second appel : doit paniquer
}

// --- Parts proportionnelles (D1) ---

#[test]
fn first_deposit_locks_minimum_liquidity() {
    let f = setup(100_000);

    let shares = f.vault.deposit(&f.user, &10_000);

    // 1 000 parts mortes verrouillées au premier dépôt (anti-inflation) :
    // le déposant reçoit amount - 1000, le total inclut les parts mortes.
    assert_eq!(shares, 9_000);
    assert_eq!(f.vault.shares_of(&f.user), 9_000);
    assert_eq!(f.vault.total_shares(), 10_000);
    assert_eq!(f.vault.total_assets(), 10_000);
}

#[test]
#[should_panic(expected = "Error(Contract, #3)")]
fn first_deposit_at_minimum_liquidity_panics() {
    let f = setup(100_000);
    f.vault.deposit(&f.user, &1_000);
}

#[test]
fn deposit_after_yield_mints_proportional_shares() {
    let f = setup(100_000);
    f.vault.deposit(&f.user, &10_000); // total_shares 10 000
    donate_yield(&f, 10_000); // total_assets 20 000

    let user2 = fund_user(&f, 10_000);
    let shares = f.vault.deposit(&user2, &10_000);

    assert_eq!(shares, 5_000); // 10 000 x 10 000 / 20 000
    assert_eq!(f.vault.shares_of(&user2), 5_000);
    assert_eq!(f.vault.total_shares(), 15_000);
    assert_eq!(f.vault.total_assets(), 30_000);
}

#[test]
fn deposit_rounding_favors_vault() {
    let f = setup(100_000);
    f.vault.deposit(&f.user, &10_000);
    donate_yield(&f, 20_000); // 10 000 parts pour 30 000 d'actif

    let shares = f.vault.deposit(&f.user, &100);

    assert_eq!(shares, 33); // 100 x 10 000 / 30 000 = 33,33 tronque en faveur du vault
}

#[test]
#[should_panic(expected = "Error(Contract, #3)")]
fn deposit_rounding_to_zero_panics() {
    let f = setup(100_000);
    f.vault.deposit(&f.user, &10_000);
    donate_yield(&f, 20_000);
    f.vault.deposit(&f.user, &2); // 2 x 10 000 / 30 000 = 0 part
}

#[test]
fn donation_before_first_deposit_is_absorbed_into_genesis() {
    let f = setup(100_000);
    donate_yield(&f, 5_000); // actif present AVANT tout depot

    let shares = f.vault.deposit(&f.user, &10_000);

    // La genese compte TOUT l'actif detenu (donation incluse) : l'invariant
    // total_parts == actifs vaut des l'origine, la donation revient au
    // premier deposant (personne d'autre ne detient de parts).
    assert_eq!(shares, 14_000); // 15 000 - 1 000 parts mortes
    assert_eq!(f.vault.total_shares(), 15_000);
    assert_eq!(f.vault.total_assets(), 15_000);
}

#[test]
#[should_panic(expected = "Error(Contract, #6)")]
fn withdraw_rounding_to_zero_panics() {
    let f = setup(100_000);
    f.vault.deposit(&f.user, &10_000); // 10 000 parts
    simulate_loss(&f, 5_000); // 1 part = 0,5 unite

    f.vault.withdraw(&f.user, &1); // 1 x 5 000 / 10 000 = 0 unite
}

#[test]
#[should_panic(expected = "Error(Contract, #7)")]
fn deposit_into_insolvent_vault_panics() {
    let f = setup(100_000);
    f.vault.deposit(&f.user, &10_000);
    simulate_loss(&f, 10_000); // perte totale : actifs 0, parts 10 000

    f.vault.deposit(&f.user, &5_000);
}

#[test]
fn withdraw_returns_proportional_amount_after_yield() {
    let f = setup(100_000);
    f.vault.deposit(&f.user, &10_000); // user 9 000 parts, total 10 000
    donate_yield(&f, 10_000); // total_assets 20 000

    let amount = f.vault.withdraw(&f.user, &4_500);

    assert_eq!(amount, 9_000); // 4 500 x 20 000 / 10 000
    assert_eq!(f.vault.shares_of(&f.user), 4_500);
    assert_eq!(f.vault.total_shares(), 5_500);
    assert_eq!(f.vault.total_assets(), 11_000);
}

#[test]
fn withdraw_rounding_favors_vault() {
    let f = setup(100_000);
    f.vault.deposit(&f.user, &10_000); // total 10 000 parts
    donate_yield(&f, 5_000); // 15 000 d'actif => 1 part = 1,5

    let amount = f.vault.withdraw(&f.user, &33);

    assert_eq!(amount, 49); // 33 x 15 000 / 10 000 = 49,5 tronque en faveur du vault
}

#[test]
fn deposit_and_withdraw_emit_structured_events() {
    let f = setup(100_000);

    // all() ne retourne que les events de la derniere invocation : capturer
    // apres chaque appel.
    f.vault.deposit(&f.user, &10_000);
    assert_eq!(
        f.env.events().all().filter_by_contract(&f.vault.address),
        vec![
            &f.env,
            (
                f.vault.address.clone(),
                (symbol_short!("deposit"), f.user.clone()).into_val(&f.env),
                (10_000_i128, 9_000_i128).into_val(&f.env),
            ),
        ]
    );

    f.vault.withdraw(&f.user, &4_000);
    assert_eq!(
        f.env.events().all().filter_by_contract(&f.vault.address),
        vec![
            &f.env,
            (
                f.vault.address.clone(),
                (symbol_short!("withdraw"), f.user.clone()).into_val(&f.env),
                (4_000_i128, 4_000_i128).into_val(&f.env),
            ),
        ]
    );
}

#[test]
fn interleaved_operations_keep_vault_solvent() {
    let f = setup(1_000_000);
    let user2 = fund_user(&f, 1_000_000);
    let user3 = fund_user(&f, 1_000_000);

    // Sequence aux ratios non ronds : la troncature laisse la poussiere au vault.
    f.vault.deposit(&f.user, &10_001);
    donate_yield(&f, 3_333);
    f.vault.deposit(&user2, &7_777);
    f.vault.withdraw(&f.user, &2_500);
    donate_yield(&f, 1_111);
    f.vault.deposit(&user3, &5_555);
    f.vault.withdraw(&user2, &1_234);

    // Solvabilite : la valeur reclamable par toutes les parts vivantes
    // (chaque retrait etant tronque) ne depasse jamais l'actif detenu.
    let assets = f.vault.total_assets();
    let total = f.vault.total_shares();
    let live = f.vault.shares_of(&f.user) + f.vault.shares_of(&user2) + f.vault.shares_of(&user3);
    assert!(live * assets / total <= assets);
    // Les parts mortes restent verrouillees dans le total.
    assert_eq!(total - live, 1_000);
}

#[test]
fn dead_shares_keep_their_backing_after_full_exit() {
    let f = setup(100_000);
    f.vault.deposit(&f.user, &10_000);
    donate_yield(&f, 10_000);

    let amount = f.vault.withdraw(&f.user, &9_000); // sortie totale du user

    assert_eq!(amount, 18_000); // 9 000 x 20 000 / 10 000
    assert_eq!(f.vault.total_shares(), 1_000); // parts mortes
    assert_eq!(f.vault.total_assets(), 2_000); // leur contre-valeur (avec rendement)
}

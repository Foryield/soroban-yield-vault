#![cfg(test)]
use super::{YieldVault, YieldVaultClient};
use soroban_sdk::{
    testutils::Address as _,
    token::{StellarAssetClient, TokenClient},
    Address, Env,
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
    let sac = env.register_stellar_asset_contract_v2(admin.clone());
    let asset = sac.address();
    let token = TokenClient::new(&env, &asset);
    StellarAssetClient::new(&env, &asset).mint(&user, &initial_mint);

    let vault_id = env.register(YieldVault, ());
    let vault = YieldVaultClient::new(&env, &vault_id);
    vault.initialize(&admin, &asset);

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
#[should_panic(expected = "insufficient shares")]
fn withdraw_beyond_balance_panics() {
    let f = setup(10_000);
    f.vault.deposit(&f.user, &2_000);
    f.vault.withdraw(&f.user, &1_001); // 2 000 deposes => 1 000 parts detenues
}

#[test]
#[should_panic(expected = "amount must be positive")]
fn deposit_zero_panics() {
    let f = setup(1_000);
    f.vault.deposit(&f.user, &0);
}

#[test]
#[should_panic(expected = "contract is paused")]
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
#[should_panic(expected = "already initialized")]
fn double_initialize_panics() {
    let f = setup(1_000);
    f.vault.initialize(&f.admin, &f.user); // second appel : doit paniquer
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
#[should_panic(expected = "deposit too small")]
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
#[should_panic(expected = "deposit too small")]
fn deposit_rounding_to_zero_panics() {
    let f = setup(100_000);
    f.vault.deposit(&f.user, &10_000);
    donate_yield(&f, 20_000);
    f.vault.deposit(&f.user, &2); // 2 x 10 000 / 30 000 = 0 part
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
fn dead_shares_keep_their_backing_after_full_exit() {
    let f = setup(100_000);
    f.vault.deposit(&f.user, &10_000);
    donate_yield(&f, 10_000);

    let amount = f.vault.withdraw(&f.user, &9_000); // sortie totale du user

    assert_eq!(amount, 18_000); // 9 000 x 20 000 / 10 000
    assert_eq!(f.vault.total_shares(), 1_000); // parts mortes
    assert_eq!(f.vault.total_assets(), 2_000); // leur contre-valeur (avec rendement)
}

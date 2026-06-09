#![cfg(test)]
use super::{YieldVault, YieldVaultClient};
use soroban_sdk::{
    testutils::Address as _,
    token::{StellarAssetClient, TokenClient},
    Address, Env,
};

struct Fixture<'a> {
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
        admin,
        user,
        token,
        vault,
    }
}

#[test]
fn deposit_mints_shares_and_moves_tokens() {
    let f = setup(1_000);

    let shares = f.vault.deposit(&f.user, &400);

    assert_eq!(shares, 400);
    assert_eq!(f.vault.shares_of(&f.user), 400);
    assert_eq!(f.vault.total_shares(), 400);
    assert_eq!(f.token.balance(&f.user), 600);
    assert_eq!(f.vault.total_assets(), 400);
}

#[test]
fn withdraw_burns_shares_and_returns_tokens() {
    let f = setup(1_000);
    f.vault.deposit(&f.user, &400);

    let amount = f.vault.withdraw(&f.user, &150);

    assert_eq!(amount, 150);
    assert_eq!(f.vault.shares_of(&f.user), 250);
    assert_eq!(f.vault.total_shares(), 250);
    assert_eq!(f.token.balance(&f.user), 750);
    assert_eq!(f.vault.total_assets(), 250);
}

#[test]
fn shares_are_conserved_round_trip() {
    let f = setup(1_000);
    f.vault.deposit(&f.user, &500);
    f.vault.withdraw(&f.user, &500);

    assert_eq!(f.vault.shares_of(&f.user), 0);
    assert_eq!(f.vault.total_shares(), 0);
    assert_eq!(f.token.balance(&f.user), 1_000);
    assert_eq!(f.vault.total_assets(), 0);
}

#[test]
#[should_panic(expected = "insufficient shares")]
fn withdraw_beyond_balance_panics() {
    let f = setup(1_000);
    f.vault.deposit(&f.user, &100);
    f.vault.withdraw(&f.user, &101);
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
    let f = setup(1_000);
    f.vault.pause();
    f.vault.unpause();
    let shares = f.vault.deposit(&f.user, &100);
    assert_eq!(shares, 100);
}

#[test]
#[should_panic(expected = "already initialized")]
fn double_initialize_panics() {
    let f = setup(1_000);
    f.vault.initialize(&f.admin, &f.user); // second appel : doit paniquer
}

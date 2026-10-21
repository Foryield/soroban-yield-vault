#![cfg(test)]
use super::{
    DataKey, DepositEvent, KeeperSetEvent, NavEvent, PausedEvent, StrategyConfig, UnpausedEvent,
    VaultError, WithdrawEvent, YieldVault, YieldVaultClient, TTL_EXTEND_TO, TTL_THRESHOLD,
};
use soroban_sdk::{
    testutils::{
        storage::Instance as _, storage::Persistent as _, Address as _, Events, IssuerFlags,
        Ledger as _, MockAuth, MockAuthInvoke,
    },
    token::{StellarAssetClient, TokenClient},
    Address, Env, Event as _, IntoVal, Symbol, Val, Vec,
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

    let vault_id = env.register(
        YieldVault,
        (
            admin.clone(),
            admin.clone(),
            asset.clone(),
            Vec::<StrategyConfig>::new(&env),
            500u32,
        ),
    );
    let vault = YieldVaultClient::new(&env, &vault_id);

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
#[should_panic(expected = "Error(Context, InvalidAction)")]
fn constructor_cannot_be_invoked_again() {
    // Le constructeur remplace l'initialize de D1 : un second appel, meme
    // direct, doit etre refuse par l'hote.
    let f = setup(1_000);
    let args: Vec<Val> = (
        f.admin.clone(),
        f.admin.clone(),
        f.user.clone(),
        Vec::<StrategyConfig>::new(&f.env),
        500u32,
    )
        .into_val(&f.env);
    f.env.invoke_contract::<()>(
        &f.vault.address,
        &Symbol::new(&f.env, "__constructor"),
        args,
    );
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
        [DepositEvent {
            owner: f.user.clone(),
            assets: 10_000,
            shares: 9_000,
            seq: 1,
            total_assets: 10_000,
            total_shares: 10_000,
        }
        .to_xdr(&f.env, &f.vault.address)]
    );

    f.vault.withdraw(&f.user, &4_000);
    assert_eq!(
        f.env.events().all().filter_by_contract(&f.vault.address),
        [WithdrawEvent {
            owner: f.user.clone(),
            shares: 4_000,
            assets: 4_000,
            seq: 2,
            total_assets: 6_000,
            total_shares: 6_000,
        }
        .to_xdr(&f.env, &f.vault.address)]
    );
    assert_eq!(f.vault.last_seq(), 2);
}

#[test]
fn a_reverted_call_leaves_no_hole_in_the_sequence() {
    let f = setup(100_000);
    f.vault.deposit(&f.user, &10_000);
    assert!(f.vault.try_withdraw(&f.user, &1_000_000).is_err());

    f.vault.deposit(&f.user, &1_000);

    assert_eq!(f.vault.last_seq(), 2);
}

#[test]
fn pause_unpause_and_keeper_changes_are_recorded() {
    let f = setup(100_000);
    let keeper = Address::generate(&f.env);

    f.vault.pause();
    assert_eq!(
        f.env.events().all().filter_by_contract(&f.vault.address),
        [PausedEvent {
            admin: f.admin.clone(),
            seq: 1,
        }
        .to_xdr(&f.env, &f.vault.address)]
    );
    f.vault.unpause();
    assert_eq!(
        f.env.events().all().filter_by_contract(&f.vault.address),
        [UnpausedEvent {
            admin: f.admin.clone(),
            seq: 2,
        }
        .to_xdr(&f.env, &f.vault.address)]
    );
    // L'ancienne valeur reste lisible (art. 2(1)(b)).
    f.vault.set_keeper(&keeper);
    assert_eq!(
        f.env.events().all().filter_by_contract(&f.vault.address),
        [KeeperSetEvent {
            old: f.admin.clone(),
            new: keeper,
            admin: f.admin.clone(),
            seq: 3,
        }
        .to_xdr(&f.env, &f.vault.address)]
    );
}

#[test]
fn checkpoint_publishes_the_nav_once_per_utc_day() {
    let f = setup(100_000);
    f.vault.deposit(&f.user, &10_000);
    donate_yield(&f, 500);
    f.env.ledger().set_timestamp(82_800); // jour 0, 23 h

    f.vault.checkpoint();

    assert_eq!(
        f.env.events().all().filter_by_contract(&f.vault.address),
        [NavEvent {
            total_assets: 10_500,
            total_shares: 10_000,
            seq: 2,
        }
        .to_xdr(&f.env, &f.vault.address)]
    );
    // Meme jour : refuse, sans consommer de numero.
    f.env.ledger().set_timestamp(86_399);
    let err = f.vault.try_checkpoint().unwrap_err().unwrap();
    assert_eq!(err, VaultError::NavAlreadyPublished.into());
    assert_eq!(f.vault.last_seq(), 2);
    // Lendemain 0 h 10, moins de 24 h apres le precedent : accepte, le jour
    // ne saute pas.
    f.env.ledger().set_timestamp(87_000);
    f.vault.checkpoint();
    assert_eq!(f.vault.last_seq(), 3);
}

#[test]
fn checkpoint_needs_no_signature_and_works_in_pause() {
    let f = setup(100_000);
    f.vault.pause();
    f.env.set_auths(&[]);

    f.vault.checkpoint();

    assert_eq!(f.vault.last_seq(), 2);
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

// --- Duree de vie des entrees (archivage)
//
// Sans prolongation, l'instance et les parts d'un porteur s'archivent a leur
// echeance : la duree par defaut du reseau est de 120 960 ledgers, soit environ
// sept jours. Passee l'echeance, le protocole restaure automatiquement, donc
// rien n'est perdu, mais l'operation qui declenche la restauration coute
// environ 240 fois le tarif nominal (mesure sur une instance de ce contrat :
// 22 920 895 stroops contre 96 282 une fois les entrees vivantes).
// Les tests ci-dessous mordent : sans les appels a extend_ttl, la TTL reste a
// la valeur par defaut de l'environnement de test.

/// TTL courantes de l'instance et de l'entree de parts d'un porteur.
fn ttls(f: &Fixture, holder: &Address) -> (u32, u32) {
    f.env.as_contract(&f.vault.address, || {
        (
            f.env.storage().instance().get_ttl(),
            f.env
                .storage()
                .persistent()
                .get_ttl(&DataKey::Shares(holder.clone())),
        )
    })
}

#[test]
fn deposit_extends_instance_and_holder_ttl() {
    let f = setup(100_000);

    f.vault.deposit(&f.user, &10_000);

    let (instance, holder) = ttls(&f, &f.user);
    assert!(instance >= TTL_EXTEND_TO - 1, "instance ttl = {instance}");
    assert!(holder >= TTL_EXTEND_TO - 1, "holder ttl = {holder}");
}

#[test]
fn withdraw_extends_instance_and_holder_ttl() {
    let f = setup(100_000);
    f.vault.deposit(&f.user, &10_000);

    // On repart d'une TTL erodee : sans prolongation au retrait, un porteur qui
    // ne fait que sortir verrait ses parts s'archiver.
    f.env
        .ledger()
        .with_mut(|li| li.sequence_number += TTL_EXTEND_TO - TTL_THRESHOLD + 1);

    f.vault.withdraw(&f.user, &4_000);

    let (instance, holder) = ttls(&f, &f.user);
    assert!(instance >= TTL_EXTEND_TO - 1, "instance ttl = {instance}");
    assert!(holder >= TTL_EXTEND_TO - 1, "holder ttl = {holder}");
}

// Les bornes des constantes de TTL ne sont pas testees ici mais verifiees a la
// compilation, par un `const _: () = assert!(...)` dans lib.rs : un relevement
// hors plafond reseau casse le build plutot qu'un test.

// --- Controle admin du coupe-circuit : auth CIBLEE, pas mock_all_auths.
//
// Tout le reste du fichier passe par setup(), qui appelle mock_all_auths() :
// sous ce mode, TOUTE signature est reputee fournie, donc retirer les
// `Self::admin(&env).require_auth()` de pause()/unpause() ne fait echouer aucun
// test. Mesure faite : la suite reste integralement verte sans ces deux gardes.
// Les tests ci-dessous sont les seuls a mordre - ils mockent nommement l'auth
// d'une adresse et verifient qui peut, et qui ne peut pas, geler le vault.
// Meme motif que `set_aqua_pool_rejects_non_admin` cote routeur.

/// Vault sans mock_all_auths. Le constructeur n'exige aucune auth, l'admin et
/// l'actif sont donc poses sans qu'aucune signature soit mockee. L'actif est
/// une adresse nue : pause/unpause ne touchent que le stockage d'instance.
fn unmocked_vault<'a>() -> (Env, Address, Address, YieldVaultClient<'a>) {
    let env = Env::default();
    let admin = Address::generate(&env);
    let outsider = Address::generate(&env);
    let asset = Address::generate(&env);
    let vault = YieldVaultClient::new(
        &env,
        &env.register(
            YieldVault,
            (
                admin.clone(),
                admin.clone(),
                asset,
                Vec::<StrategyConfig>::new(&env),
                500u32,
            ),
        ),
    );
    (env, admin, outsider, vault)
}

#[test]
fn pause_rejects_non_admin() {
    let (env, _admin, outsider, vault) = unmocked_vault();

    // Seule l'auth de l'inconnu est mockee : le require_auth de l'admin, lui,
    // n'a rien pour se satisfaire.
    env.mock_auths(&[MockAuth {
        address: &outsider,
        invoke: &MockAuthInvoke {
            contract: &vault.address,
            fn_name: "pause",
            args: Vec::new(&env),
            sub_invokes: &[],
        },
    }]);

    // Echec d'auth = erreur HOTE, pas un VaultError. Si le require_auth
    // disparaissait, l'appel reussirait et ce test echouerait.
    assert!(vault.try_pause().is_err());
    assert!(!vault.is_paused());
}

#[test]
fn unpause_rejects_non_admin() {
    let (env, admin, outsider, vault) = unmocked_vault();

    env.mock_auths(&[MockAuth {
        address: &admin,
        invoke: &MockAuthInvoke {
            contract: &vault.address,
            fn_name: "pause",
            args: Vec::new(&env),
            sub_invokes: &[],
        },
    }]);
    vault.pause();

    env.mock_auths(&[MockAuth {
        address: &outsider,
        invoke: &MockAuthInvoke {
            contract: &vault.address,
            fn_name: "unpause",
            args: Vec::new(&env),
            sub_invokes: &[],
        },
    }]);

    // Le pendant du test precedent : un inconnu ne peut pas non plus DEGELER
    // le vault, ce qui compte autant que de ne pas pouvoir le geler.
    assert!(vault.try_unpause().is_err());
    assert!(vault.is_paused());
}

#[test]
fn pause_and_unpause_accept_the_admin_alone() {
    let (env, admin, _outsider, vault) = unmocked_vault();

    // Contre-epreuve : sans elle, les deux tests ci-dessus passeraient aussi
    // si pause() echouait pour TOUT LE MONDE, y compris l'admin.
    env.mock_auths(&[MockAuth {
        address: &admin,
        invoke: &MockAuthInvoke {
            contract: &vault.address,
            fn_name: "pause",
            args: Vec::new(&env),
            sub_invokes: &[],
        },
    }]);
    vault.pause();
    assert!(vault.is_paused());

    env.mock_auths(&[MockAuth {
        address: &admin,
        invoke: &MockAuthInvoke {
            contract: &vault.address,
            fn_name: "unpause",
            args: Vec::new(&env),
            sub_invokes: &[],
        },
    }]);
    vault.unpause();
    assert!(!vault.is_paused());
}

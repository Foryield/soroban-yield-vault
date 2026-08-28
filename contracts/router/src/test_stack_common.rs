#![cfg(test)]
// Montants ecrits en convention Stellar 7 decimales (X_XXXXXXX, ex. 5_0000000
// = 5,0) : le groupement d'underscores suit les decimales de l'actif, pas les
// milliers, comme dans test_blend.rs du vault.
#![allow(clippy::inconsistent_digit_grouping, clippy::zero_prefixed_literal)]
// L'arite des clients generes par contractimport! est dictee par les ABI
// externes (8-9 arguments), meme justification que dans venues/.
#![allow(clippy::too_many_arguments)]
//! Socle commun de la fixture « stack reelle » : env + budget, tokens SAC
//! USDC/EURC, financement de l'utilisateur, routeur ForYield, helper de
//! reordonnancement des reserves et garde anti-derive des wasm vendorises
//! (manifeste SHA256SUMS).
//!
//! Le stack Soroswap et sa garde de wasm construit localement ont ete retires
//! le 28/08/2026 avec la venue (cf.
//! docs/plans/2026-08-28-retrait-soroswap-aquarius-seul.md). La fixture Aqua
//! (test_aqua_stack.rs) est desormais la seule a batir sur ce socle.

extern crate std;

use super::{SwapRouter, SwapRouterClient};
use soroban_sdk::{
    contract, contractimpl,
    testutils::{Address as _, Ledger},
    token::{StellarAssetClient, TokenClient},
    vec, Address, Bytes, BytesN, Env, Vec,
};

/// Reserves initiales du pool : 1000 USDC / 1000 EURC (prix 1:1).
pub const RESERVE: i128 = 1_000_0000000;
pub const AMOUNT_IN: i128 = 5_0000000;
pub const MIN_OUT: i128 = 4_9000000;

/// fee_bps COMPTABLE du routeur ForYield.
pub const AQUARIUS_FEE_BPS: u32 = 10;

/// Timestamp de ledger fixe et non nul : les tests de deadline comparent
/// contre cette valeur.
pub const LEDGER_TIME: u64 = 1_700_000_000;

pub struct BaseFixture<'a> {
    pub env: Env,
    pub admin: Address,
    pub user: Address,
    pub usdc: TokenClient<'a>,
    pub eurc: TokenClient<'a>,
}

/// Socle : env (auths mockees, timestamp fixe, budget illimite -- les wasm
/// reels depassent le budget CPU par defaut de l'env de test), tokens SAC
/// USDC/EURC, utilisateur finance de AMOUNT_IN en USDC.
pub fn setup_base<'a>() -> BaseFixture<'a> {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().with_mut(|li| li.timestamp = LEDGER_TIME);
    env.cost_estimate().budget().reset_unlimited();

    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let usdc = TokenClient::new(
        &env,
        &env.register_stellar_asset_contract_v2(admin.clone())
            .address(),
    );
    let eurc = TokenClient::new(
        &env,
        &env.register_stellar_asset_contract_v2(admin.clone())
            .address(),
    );
    StellarAssetClient::new(&env, &usdc.address).mint(&user, &AMOUNT_IN);

    BaseFixture {
        env,
        admin,
        user,
        usdc,
        eurc,
    }
}

/// Routeur ForYield enregistre et initialise sur la venue fournie, fee_bps du
/// socle.
pub fn init_router<'a>(base: &BaseFixture, aquarius_router: &Address) -> SwapRouterClient<'a> {
    let router = SwapRouterClient::new(&base.env, &base.env.register(SwapRouter, ()));
    router.initialize(&base.admin, aquarius_router, &AQUARIUS_FEE_BPS);
    router
}

/// Reordonne un couple de reserves (reserve_0, reserve_1), rendu dans
/// l'ordre des tokens TRIES par adresse (convention du router Aqua), vers
/// l'ordre fixe (usdc, eurc). L'ordre trie n'est pas deterministe entre deux
/// runs (adresses generees) : les lecteurs de reserves des fixtures passent
/// tous par ce helper (suivi de revue Task 11).
pub fn order_usdc_eurc(
    usdc: &Address,
    eurc: &Address,
    reserve_0: i128,
    reserve_1: i128,
) -> (i128, i128) {
    if usdc < eurc {
        (reserve_0, reserve_1)
    } else {
        (reserve_1, reserve_0)
    }
}

pub mod aqua_router_wasm {
    soroban_sdk::contractimport!(file = "test_wasms/soroban_liquidity_pool_router_contract.wasm");
}
mod aqua_pool_wasm {
    soroban_sdk::contractimport!(file = "test_wasms/soroban_liquidity_pool_contract.wasm");
}
mod aqua_plane_wasm {
    soroban_sdk::contractimport!(file = "test_wasms/soroban_liquidity_pool_plane_contract.wasm");
}
mod aqua_calculator_wasm {
    soroban_sdk::contractimport!(
        file = "test_wasms/soroban_liquidity_pool_liquidity_calculator_contract.wasm"
    );
}
mod aqua_token_wasm {
    soroban_sdk::contractimport!(file = "test_wasms/soroban_token_contract.wasm");
}

/// fee_fraction du pool standard, en unites de 1/10 000 : 30 = 0,3 %. Le
/// router Aqua n'accepte que la liste blanche [10, 30, 100] (miroir,
/// liquidity_pool_router/src/constants.rs, CONSTANT_PRODUCT_FEE_AVAILABLE ;
/// erreur BadFee=302 du spec embarque sinon).
pub const AQUA_FEE_FRACTION: u32 = 30;

/// Feed de boost factice : la chaine d'init du router Aqua EXIGE un feed
/// (set_reward_boost_config, lu sans garde par init_standard_pool -- miroir,
/// pool_utils.rs), et le checkpoint de rewards du deposit invoque
/// feed.total_supply() sans try_ (miroir, rewards/src/manager.rs,
/// get_total_locked) : l'adresse doit porter un CONTRAT exportant
/// total_supply. Le locker feed canonique n'est pas vendorise (absent du
/// perimetre Task 9) ; total_supply = 0 rend le boost neutre (miroir,
/// calculate_effective_balance : total_locked = 0 -> balance effective =
/// balance de parts, aucun effet sur les rewards ni sur le swap).
#[contract]
struct MockBoostFeed;

#[contractimpl]
impl MockBoostFeed {
    pub fn total_supply(_env: Env) -> u128 {
        0
    }
}

pub struct AquaStack<'a> {
    pub router: aqua_router_wasm::Client<'a>,
    pub pool_index: BytesN<32>,
}

/// Chaine d'init Aqua complete depuis les wasm vendorises. Chaque etape est
/// OBLIGATOIRE : init_standard_pool lit token_hash, reward_token, boost
/// token/feed, plane et la config de paiement sans garde (absents ->
/// StorageError 501, miroir pool_utils.rs / rewards/src/storage.rs). Les
/// roles privilegies (rewards/operations/pause/emergency) retombent sur
/// l'admin via get_role_safe : set_privileged_addrs est omis a dessein.
/// Le calculator n'est pas exige par init_standard_pool mais fait partie du
/// cablage de reference (aqua_setup.rs) : branche pour rester conforme.
pub fn deploy_aqua_stack<'a>(base: &BaseFixture, with_liquidity: bool) -> AquaStack<'a> {
    let env = &base.env;
    let admin = &base.admin;

    let pool_hash = env.deployer().upload_contract_wasm(aqua_pool_wasm::WASM);
    let token_hash = env.deployer().upload_contract_wasm(aqua_token_wasm::WASM);

    let aqua_router = aqua_router_wasm::Client::new(env, &env.register(aqua_router_wasm::WASM, ()));
    aqua_router.init_admin(admin);
    aqua_router.set_pool_hash(admin, &pool_hash);
    aqua_router.set_token_hash(admin, &token_hash);

    let reward_token = env
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let boost_token = env
        .register_stellar_asset_contract_v2(admin.clone())
        .address();
    let boost_feed = env.register(MockBoostFeed, ());
    aqua_router.set_reward_token(admin, &reward_token);
    aqua_router.set_reward_boost_config(admin, &boost_token, &boost_feed);
    // Le token de paiement est lu meme a montant nul (miroir, contract.rs,
    // init_standard_pool) : configure a 0, aucun transfert a la creation.
    aqua_router.configure_init_pool_payment(admin, &reward_token, &0, &0, admin);

    let plane = env.register(aqua_plane_wasm::WASM, ());
    aqua_router.set_pools_plane(admin, &plane);
    let calculator =
        aqua_calculator_wasm::Client::new(env, &env.register(aqua_calculator_wasm::WASM, ()));
    calculator.init_admin(admin);
    calculator.set_pools_plane(admin, &plane);
    aqua_router.set_liquidity_calculator(admin, &calculator.address);

    // Paire TRIEE par adresse : convention du router Aqua
    // (assert_tokens_sorted, erreur TokensNotSorted=2002 du spec embarque),
    // la meme que notre venue et notre registre appliquent.
    let tokens = sorted_pair_vec(env, &base.usdc.address, &base.eurc.address);
    let (pool_index, _pool) = aqua_router.init_standard_pool(admin, &tokens, &AQUA_FEE_FRACTION);

    if with_liquidity {
        StellarAssetClient::new(env, &base.usdc.address).mint(admin, &RESERVE);
        StellarAssetClient::new(env, &base.eurc.address).mint(admin, &RESERVE);
        aqua_router.deposit(
            admin,
            &tokens,
            &pool_index,
            &vec![env, RESERVE as u128, RESERVE as u128],
            &0,
        );
    }

    AquaStack {
        router: aqua_router,
        pool_index,
    }
}

pub fn sorted_pair_vec(env: &Env, a: &Address, b: &Address) -> Vec<Address> {
    if a < b {
        vec![env, a.clone(), b.clone()]
    } else {
        vec![env, b.clone(), a.clone()]
    }
}

/// Octets embarques des 5 wasm vendorises, indexes par nom de fichier : la
/// garde vendored_wasms_match_sha256sums confronte chaque entree de
/// SHA256SUMS a ces octets. include_bytes! plutot que les WASM des
/// contractimport! : la garde couvre les fichiers du manifeste,
/// independamment de ce que les fixtures importent.
const VENDORED_WASMS: [(&str, &[u8]); 5] = [
    (
        "soroban_liquidity_pool_contract.wasm",
        include_bytes!("../test_wasms/soroban_liquidity_pool_contract.wasm"),
    ),
    (
        "soroban_liquidity_pool_liquidity_calculator_contract.wasm",
        include_bytes!("../test_wasms/soroban_liquidity_pool_liquidity_calculator_contract.wasm"),
    ),
    (
        "soroban_liquidity_pool_plane_contract.wasm",
        include_bytes!("../test_wasms/soroban_liquidity_pool_plane_contract.wasm"),
    ),
    (
        "soroban_liquidity_pool_router_contract.wasm",
        include_bytes!("../test_wasms/soroban_liquidity_pool_router_contract.wasm"),
    ),
    (
        "soroban_token_contract.wasm",
        include_bytes!("../test_wasms/soroban_token_contract.wasm"),
    ),
];

/// Decode une empreinte SHA-256 hexadecimale (64 caracteres, minuscules).
fn sha256_from_hex(hex: &str) -> [u8; 32] {
    fn nibble(c: u8) -> u8 {
        match c {
            b'0'..=b'9' => c - b'0',
            b'a'..=b'f' => c - b'a' + 10,
            _ => panic!("hex invalide dans l'empreinte consignee"),
        }
    }
    let hex = hex.as_bytes();
    assert_eq!(hex.len(), 64);
    let mut out = [0_u8; 32];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = (nibble(hex[2 * i]) << 4) | nibble(hex[2 * i + 1]);
    }
    out
}

fn sha256_of(env: &Env, wasm: &[u8]) -> [u8; 32] {
    env.crypto()
        .sha256(&Bytes::from_slice(env, wasm))
        .to_array()
}

/// Garde anti-derive des 5 wasm vendorises : SHA256SUMS est parse au moment
/// du test et chaque entree confrontee aux octets presents sur le disque au
/// moment de la compilation (include_bytes! est une dependance de build :
/// tout changement de fichier force la recompilation, la garde voit donc
/// toujours les octets courants).
///
/// Cette garde a PRIS DE L'IMPORTANCE le 28/08/2026 : la source amont de ces
/// wasm (depot soroswap/aggregator, seul a publier les binaires Aqua, le
/// depot canonique AquaToken/soroban-amm etant en 404) a ete coupee suite a
/// la compromission de Soroswap. Elle est desormais la seule chose qui
/// garantit que les binaires du depot n'ont pas ete alteres depuis leur
/// epinglage au commit 84de10e0 de juillet 2026.
#[test]
fn vendored_wasms_match_sha256sums() {
    let manifest = include_str!("../test_wasms/SHA256SUMS");
    let env = Env::default();
    let mut checked = 0_usize;
    for line in manifest.lines().filter(|line| !line.trim().is_empty()) {
        let (hex, name) = line
            .split_once("  ")
            .expect("ligne SHA256SUMS invalide (attendu : empreinte, deux espaces, nom)");
        let (_, wasm) = VENDORED_WASMS
            .iter()
            .find(|(entry, _)| *entry == name)
            .unwrap_or_else(|| panic!("wasm absent de VENDORED_WASMS : {name}"));
        assert_eq!(
            sha256_of(&env, wasm),
            sha256_from_hex(hex),
            "empreinte divergente pour {name}"
        );
        checked += 1;
    }
    assert_eq!(
        checked,
        VENDORED_WASMS.len(),
        "SHA256SUMS doit lister les 5 wasm vendorises"
    );
}

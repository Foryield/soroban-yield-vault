#![cfg(test)]
// Montants ecrits en convention Stellar 7 decimales (X_XXXXXXX, ex. 5_0000000
// = 5,0) : le groupement d'underscores suit les decimales de l'actif, pas les
// milliers, comme dans test_blend.rs du vault.
#![allow(clippy::inconsistent_digit_grouping, clippy::zero_prefixed_literal)]
// L'arite des clients generes par contractimport! est dictee par les ABI
// externes (8-9 arguments), meme justification que dans venues/.
#![allow(clippy::too_many_arguments)]
//! Socle commun des fixtures « stack reelle » (tasks 10 et 11, suivi de revue
//! Task 10) : env + budget, tokens SAC USDC/EURC, financement de
//! l'utilisateur, routeur ForYield, deploiement du stack Soroswap complet,
//! helper de reordonnancement des reserves et gardes anti-derive des wasm
//! vendorises (aggregator local + manifeste SHA256SUMS).
//! La fixture Aqua (test_aqua_stack.rs) reutilise ce socle ET le stack
//! Soroswap pour le test de fallback reel ; la fixture Soroswap
//! (test_soroswap_stack.rs) n'y ajoute que ses derivations propres.

extern crate std;

use super::{SwapRouter, SwapRouterClient};
use soroban_sdk::{
    testutils::{Address as _, Ledger},
    token::{StellarAssetClient, TokenClient},
    Address, Bytes, Env,
};

pub mod factory_wasm {
    soroban_sdk::contractimport!(file = "test_wasms/soroswap_factory.wasm");
}
pub mod pair_wasm {
    soroban_sdk::contractimport!(file = "test_wasms/soroswap_pair.wasm");
}
pub mod router_wasm {
    soroban_sdk::contractimport!(file = "test_wasms/soroswap_router.wasm");
}
pub mod aggregator_wasm {
    soroban_sdk::contractimport!(file = "test_wasms/soroswap_aggregator.wasm");
}

/// Reserves initiales des pools : 1000 USDC / 1000 EURC (prix 1:1), memes
/// valeurs pour les deux venues afin que les derivations soient comparables.
pub const RESERVE: i128 = 1_000_0000000;
pub const AMOUNT_IN: i128 = 5_0000000;
pub const MIN_OUT: i128 = 4_9000000;

/// fee_bps COMPTABLES du routeur ForYield par venue. Chaque fixture note en
/// son sein laquelle des deux constantes est inerte chez elle (configuree a
/// l'initialize mais jamais lue faute de swap conclu sur la venue).
pub const SOROSWAP_FEE_BPS: u32 = 30;
pub const AQUARIUS_FEE_BPS: u32 = 10;

/// Timestamp de ledger fixe et non nul : les tests de deadline comparent
/// contre cette valeur.
pub const LEDGER_TIME: u64 = 1_700_000_000;

/// Empreinte SHA-256 consignee du wasm aggregator construit localement
/// (test_wasms/README.md, section « construit localement ») : ce wasm est
/// HORS de SHA256SUMS (non re-telechargeable), cette constante est sa seule
/// garde anti-derive.
const AGGREGATOR_WASM_SHA256_HEX: &str =
    "4ee0fddf79d695d48e694413d8eee7ba592d38b626d94c8b4e3c54f725eb2f40";

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

/// Routeur ForYield enregistre et initialise sur les deux venues fournies,
/// fee_bps du socle. Les fixtures passent une adresse SANS contrat pour une
/// venue absente de leur perimetre (registre Aqua vide ou aggregator jamais
/// atteint) : la venue rend false sans appel, le fallback la traverse.
pub fn init_router<'a>(
    base: &BaseFixture,
    soroswap_aggregator: &Address,
    aquarius_router: &Address,
) -> SwapRouterClient<'a> {
    let router = SwapRouterClient::new(&base.env, &base.env.register(SwapRouter, ()));
    router.initialize(
        &base.admin,
        soroswap_aggregator,
        aquarius_router,
        &SOROSWAP_FEE_BPS,
        &AQUARIUS_FEE_BPS,
    );
    router
}

/// Reordonne un couple de reserves (reserve_0, reserve_1), rendu dans
/// l'ordre des tokens TRIES par adresse (convention commune au pair Soroswap
/// et au router Aqua), vers l'ordre fixe (usdc, eurc). L'ordre trie n'est
/// pas deterministe entre deux runs (adresses generees) : les trois lecteurs
/// de reserves des fixtures passent par ce helper (suivi de revue Task 11).
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

pub struct SoroswapStack<'a> {
    pub aggregator: Address,
    pub router: router_wasm::Client<'a>,
    pub pair: pair_wasm::Client<'a>,
}

/// Stack Soroswap complet : factory (initialisee avec le hash du wasm du
/// pair), router Soroswap, aggregator reel (initialize admin + adapter
/// Soroswap pointant le router), paire USDC/EURC creee et alimentee
/// RESERVE/RESERVE par add_liquidity (semantique Uniswap V2, miroir de
/// scripts/seed_soroswap_pool.sh ; mins = desired : premiere fourniture,
/// prix libre, aucun arrondi). Les reserves sont mintees a l'admin ici.
pub fn deploy_soroswap_stack<'a>(base: &BaseFixture) -> SoroswapStack<'a> {
    let env = &base.env;
    let admin = &base.admin;

    let pair_hash = env.deployer().upload_contract_wasm(pair_wasm::WASM);
    let factory = env.register(factory_wasm::WASM, ());
    factory_wasm::Client::new(env, &factory).initialize(admin, &pair_hash);

    let soroswap_router_id = env.register(router_wasm::WASM, ());
    let soroswap_router = router_wasm::Client::new(env, &soroswap_router_id);
    soroswap_router.initialize(&factory);

    let aggregator = env.register(aggregator_wasm::WASM, ());
    aggregator_wasm::Client::new(env, &aggregator).initialize(
        admin,
        &soroban_sdk::vec![
            env,
            aggregator_wasm::Adapter {
                protocol_id: aggregator_wasm::Protocol::Soroswap,
                router: soroswap_router_id.clone(),
                paused: false,
            },
        ],
    );

    StellarAssetClient::new(env, &base.usdc.address).mint(admin, &RESERVE);
    StellarAssetClient::new(env, &base.eurc.address).mint(admin, &RESERVE);
    soroswap_router.add_liquidity(
        &base.usdc.address,
        &base.eurc.address,
        &RESERVE,
        &RESERVE,
        &RESERVE,
        &RESERVE,
        admin,
        &(LEDGER_TIME + 3600),
    );
    let pair = pair_wasm::Client::new(
        env,
        &soroswap_router.router_pair_for(&base.usdc.address, &base.eurc.address),
    );

    SoroswapStack {
        aggregator,
        router: soroswap_router,
        pair,
    }
}

/// Octets embarques des 8 wasm re-telechargeables, indexes par nom de
/// fichier : la garde vendored_wasms_match_sha256sums confronte chaque
/// entree de SHA256SUMS a ces octets. include_bytes! plutot que les WASM
/// des contractimport! : la garde couvre les fichiers du manifeste,
/// independamment de ce que les fixtures importent.
const VENDORED_WASMS: [(&str, &[u8]); 8] = [
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
    (
        "soroswap_factory.wasm",
        include_bytes!("../test_wasms/soroswap_factory.wasm"),
    ),
    (
        "soroswap_pair.wasm",
        include_bytes!("../test_wasms/soroswap_pair.wasm"),
    ),
    (
        "soroswap_router.wasm",
        include_bytes!("../test_wasms/soroswap_router.wasm"),
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

/// Garde anti-derive du wasm aggregator construit localement (suivi de revue
/// Task 10) : hors SHA256SUMS, seul ce test detecte un binaire regenere sans
/// mise a jour du README (ou altere). SHA-256 via le host crypto de l'env de
/// test : aucune dependance ajoutee. L'empreinte attendue est decodee depuis
/// la chaine hex consignee, identique caractere pour caractere au README.
#[test]
fn locally_built_aggregator_wasm_matches_recorded_sha256() {
    let env = Env::default();
    assert_eq!(
        sha256_of(&env, aggregator_wasm::WASM),
        sha256_from_hex(AGGREGATOR_WASM_SHA256_HEX)
    );
}

/// Garde anti-derive des 8 wasm re-telechargeables (suivi de revue Task 11,
/// durcissement supply-chain du repo public) : SHA256SUMS est parse au
/// moment du test et chaque entree confrontee aux octets reellement presents
/// sur le disque. Complement du script fetch (qui ne verifie qu'au
/// re-telechargement) : ici la verification court a chaque run de tests.
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
        "SHA256SUMS doit lister les 8 wasm vendorises"
    );
}

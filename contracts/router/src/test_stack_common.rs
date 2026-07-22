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
//! l'utilisateur, routeur ForYield, et deploiement du stack Soroswap complet.
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

/// Garde anti-derive du wasm aggregator construit localement (suivi de revue
/// Task 10) : hors SHA256SUMS, seul ce test detecte un binaire regenere sans
/// mise a jour du README (ou altere). SHA-256 via le host crypto de l'env de
/// test : aucune dependance ajoutee. L'empreinte attendue est decodee depuis
/// la chaine hex consignee, identique caractere pour caractere au README.
#[test]
fn locally_built_aggregator_wasm_matches_recorded_sha256() {
    fn nibble(c: u8) -> u8 {
        match c {
            b'0'..=b'9' => c - b'0',
            b'a'..=b'f' => c - b'a' + 10,
            _ => panic!("hex invalide dans l'empreinte consignee"),
        }
    }
    let hex = AGGREGATOR_WASM_SHA256_HEX.as_bytes();
    assert_eq!(hex.len(), 64);
    let mut expected = [0_u8; 32];
    for (i, byte) in expected.iter_mut().enumerate() {
        *byte = (nibble(hex[2 * i]) << 4) | nibble(hex[2 * i + 1]);
    }

    let env = Env::default();
    let digest = env
        .crypto()
        .sha256(&Bytes::from_slice(&env, aggregator_wasm::WASM));
    assert_eq!(digest.to_array(), expected);
}

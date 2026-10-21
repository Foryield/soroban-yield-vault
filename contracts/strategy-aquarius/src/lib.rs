#![no_std]
//! ForYield, strategie de liquidite Aquarius au trait DeFindex (D5, etape 5).
//!
//! Strategie mono-actif (l'actif du vault, USDC) qui detient des parts d'un
//! pool Aquarius a produit constant actif / actif apparie (EURC). Plan de
//! conception : docs/plans/2026-10-20-d5-allocateur-conception.md, section 5.
//!
//! - Valeur equitable : la position LP vaut `2 * sqrt(r_actif * r_apparie *
//!   p) * parts / total_parts`, ou `p` est le prix de reference de l'apparie
//!   en actif. Le produit des reserves ne bouge pas sous un swap (il ne fait
//!   que croitre des frais) : aucun swap ne deplace cette valeur, qui entre
//!   dans total_assets du vault et donc dans le prix de sa part.
//! - Prix de reference publie par le keeper (taux BCE du jour) ou l'admin,
//!   jamais lu sur un oracle tiers : un prix ne s'ecarte pas de plus de
//!   `max_step_bps` du precedent, une publication par heure au plus, et un
//!   prix plus vieux que `max_age` fait echouer toute operation qui valorise.
//!   Un keeper muet se remplace : l'admin publie, ou nomme un autre keeper.
//! - Entree : swap optimal d'une part de l'actif vers l'apparie, depot des
//!   deux jambes ; sortie : retrait de parts LP, swap de l'apparie vers
//!   l'actif. Chaque operation est jugee sur la valeur equitable avant et
//!   apres : la perte (frais, glissement, pool decale du prix de reference)
//!   ne passe que sous `max_slippage_bps`, sinon tout revert. A la sortie, le
//!   cout est a la charge du deposant qui sort (decision du 20/10).
//! - Parts internes par deposant, MINIMUM de 1000 parts mortes au premier
//!   depot (meme modele que le vault et la strategie Blend de DeFindex).
//! - `harvest` sans effet : les recompenses AQUA ne sont pas reclamees en D5.
//!
//! Autorisations imbriquees : le pool Aquarius exige l'auth de la strategie
//! dans sa propre frame (`deposit`, `withdraw`) puis tire les tokens ou brule
//! les parts LP ; chaque appel pre-autorise exactement cet arbre (fonction,
//! arguments et montants), meme convention que le vault et le routeur D4.
//! Arbres verifies contre les wasm Aquarius vendorises du routeur ; le pool
//! deploye sur testnet est une version plus recente, a eprouver sur le
//! reseau avant toute preuve.

use defindex_strategy_core::{event, DeFindexStrategyTrait, StrategyError};
use soroban_sdk::{
    auth::{ContractContext, InvokerContractAuthEntry, SubContractInvocation},
    contract, contractclient, contracterror, contractimpl, contracttype, panic_with_error,
    symbol_short,
    token::TokenClient,
    vec, Address, Bytes, BytesN, Env, IntoVal, Map, String, Symbol, Val, Vec,
};

mod math;

/// Erreurs propres a la strategie. Les codes 4xx sont ceux de
/// `StrategyError` (crate DeFindex) : les notres commencent a 501.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum AquaStrategyError {
    StalePrice = 501,
    PriceTooSoon = 502,
    PriceStepTooLarge = 503,
    NotAuthorized = 504,
    Slippage = 505,
    InsufficientShares = 506,
    PoolNotFound = 507,
    NotConstantProduct = 508,
    InvalidConfig = 509,
    MathOverflow = 510,
}

/// Echelle du prix de reference : unites d'actif pour une unite d'apparie,
/// a 9 decimales (1,1221 USDC par EURC = 1_122_100_000).
pub const PRICE_SCALE: i128 = 1_000_000_000;

/// Parts mortes au premier depot, jamais rachetables : bornent le cout d'une
/// attaque par inflation du prix de la premiere part.
pub const DEAD_SHARES: i128 = 1_000;

/// Intervalle minimal entre deux publications du prix de reference.
pub const MIN_PUBLISH_INTERVAL: u64 = 3_600;

const BPS: i128 = 10_000;

const STRATEGY_NAME: &str = "AquariusLpStrategy";

/// Durees de vie, memes valeurs que le vault : sous 30 jours restants, on
/// prolonge a 120 jours.
const LEDGERS_PER_DAY: u32 = 17_280;
const TTL_THRESHOLD: u32 = 30 * LEDGERS_PER_DAY;
const TTL_EXTEND_TO: u32 = 120 * LEDGERS_PER_DAY;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Config {
    pub asset: Address,
    pub paired: Address,
    pub router: Address,
    pub pool: Address,
    pub pool_hash: BytesN<32>,
    /// Paire triee par adresse, convention du router Aquarius.
    pub tokens: Vec<Address>,
    /// Token des parts LP et frais du pool, lus une fois au deploiement :
    /// chaque appel inter-contrats coute de la memoire d'invocation, bornee
    /// par le reseau.
    pub share_token: Address,
    pub fee_bps: u32,
    pub max_age: u64,
    pub max_step_bps: u32,
    pub max_slippage_bps: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReferencePrice {
    pub price: i128,
    pub updated_at: u64,
    /// Date de la source (taux BCE), en secondes Unix, telle que publiee.
    pub source_date: u64,
}

#[contracttype]
#[derive(Clone)]
enum DataKey {
    Config,
    Admin,
    Keeper,
    Price,
    TotalShares,
    Shares(Address),
}

/// Client du router Aquarius : signatures relues sur le wasm vendorise et
/// sur le contrat deploye sur testnet le 20/10, identiques.
#[allow(dead_code)]
#[contractclient(name = "AquaRouterClient")]
trait AquaRouter {
    fn get_pools(env: Env, tokens: Vec<Address>) -> Map<BytesN<32>, Address>;
    fn deposit(
        env: Env,
        user: Address,
        tokens: Vec<Address>,
        pool_index: BytesN<32>,
        desired_amounts: Vec<u128>,
        min_shares: u128,
    ) -> (Vec<u128>, u128);
    fn withdraw(
        env: Env,
        user: Address,
        tokens: Vec<Address>,
        pool_index: BytesN<32>,
        share_amount: u128,
        min_amounts: Vec<u128>,
    ) -> Vec<u128>;
    fn swap_chained(
        env: Env,
        user: Address,
        swaps_chain: Vec<(Vec<Address>, BytesN<32>, Address)>,
        token_in: Address,
        in_amount: u128,
        out_min: u128,
    ) -> u128;
}

#[allow(dead_code)]
#[contractclient(name = "AquaPoolClient")]
trait AquaPool {
    fn pool_type(env: Env) -> Symbol;
    fn get_fee_fraction(env: Env) -> u32;
    fn share_id(env: Env) -> Address;
    fn get_reserves(env: Env) -> Vec<u128>;
    fn get_total_shares(env: Env) -> u128;
}

/// Etat de la position lu en une fois : reserves dans l'ordre (actif,
/// apparie), parts LP du pool et de la strategie, soldes oisifs.
struct Position {
    r_asset: i128,
    r_paired: i128,
    lp_total: i128,
    lp_held: i128,
    idle_asset: i128,
    idle_paired: i128,
}

#[contract]
pub struct AquariusLpStrategy;

#[contractimpl]
impl DeFindexStrategyTrait for AquariusLpStrategy {
    /// `init_args`, dans l'ordre : admin, keeper, router Aquarius, hash du
    /// pool (BytesN<32>), token apparie, prix initial (i128, PRICE_SCALE),
    /// max_age (u64, secondes), max_step_bps (u32), max_slippage_bps (u32).
    fn __constructor(env: Env, asset: Address, init_args: Vec<Val>) {
        let arg = |i: u32| {
            init_args
                .get(i)
                .unwrap_or_else(|| panic_with_error!(&env, AquaStrategyError::InvalidConfig))
        };
        let admin: Address = arg(0).into_val(&env);
        let keeper: Address = arg(1).into_val(&env);
        let router: Address = arg(2).into_val(&env);
        let pool_hash: BytesN<32> = arg(3).into_val(&env);
        let paired: Address = arg(4).into_val(&env);
        let initial_price: i128 = arg(5).into_val(&env);
        let max_age: u64 = arg(6).into_val(&env);
        let max_step_bps: u32 = arg(7).into_val(&env);
        let max_slippage_bps: u32 = arg(8).into_val(&env);

        if paired == asset
            || initial_price <= 0
            || max_age == 0
            || max_step_bps as i128 > BPS
            || max_slippage_bps as i128 > BPS
        {
            panic_with_error!(&env, AquaStrategyError::InvalidConfig);
        }

        let tokens = if asset < paired {
            vec![&env, asset.clone(), paired.clone()]
        } else {
            vec![&env, paired.clone(), asset.clone()]
        };
        // Le pool doit exister pour cette paire, et etre a produit constant :
        // la formule de valeur equitable ne vaut que pour lui.
        let pool = AquaRouterClient::new(&env, &router)
            .get_pools(&tokens)
            .get(pool_hash.clone())
            .unwrap_or_else(|| panic_with_error!(&env, AquaStrategyError::PoolNotFound));
        let pool_client = AquaPoolClient::new(&env, &pool);
        if pool_client.pool_type() != Symbol::new(&env, "constant_product") {
            panic_with_error!(&env, AquaStrategyError::NotConstantProduct);
        }
        let share_token = pool_client.share_id();
        let fee_bps = pool_client.get_fee_fraction();

        let config = Config {
            asset,
            paired,
            router,
            pool,
            pool_hash,
            tokens,
            share_token,
            fee_bps,
            max_age,
            max_step_bps,
            max_slippage_bps,
        };
        let storage = env.storage().instance();
        storage.set(&DataKey::Config, &config);
        storage.set(&DataKey::Admin, &admin);
        storage.set(&DataKey::Keeper, &keeper);
        storage.set(
            &DataKey::Price,
            &ReferencePrice {
                price: initial_price,
                updated_at: env.ledger().timestamp(),
                source_date: 0,
            },
        );
        storage.set(&DataKey::TotalShares, &0i128);
    }

    fn asset(env: Env) -> Result<Address, StrategyError> {
        Ok(Self::config(env).asset)
    }

    fn deposit(env: Env, amount: i128, from: Address) -> Result<i128, StrategyError> {
        if amount <= 0 {
            return Err(StrategyError::OnlyPositiveAmountAllowed);
        }
        from.require_auth();
        Self::extend_instance(&env);
        let cfg = Self::config(env.clone());
        let price = Self::fresh_price(&env, &cfg);
        let this = env.current_contract_address();
        let pos = Self::position(&env, &cfg);
        let value_before = Self::value(&env, &pos, price);

        TokenClient::new(&env, &cfg.asset).transfer(&from, &this, &amount);

        // Swap optimal d'une part de l'actif oisif (depot compris) vers
        // l'apparie, puis depot des deux jambes. Aucun minimum par etape : la
        // seule garde qui compte est la valeur equitable gagnee, mesuree a la
        // fin. Reserves et oisif viennent de la lecture d'avant le transfert :
        // une seule lecture de position de moins.
        let to_swap = math::zap_swap_amount(
            pos.r_asset as u128,
            (pos.idle_asset + amount) as u128,
            cfg.fee_bps as u128,
        )
        .unwrap_or_else(|| panic_with_error!(&env, AquaStrategyError::MathOverflow));
        if to_swap > 0 {
            Self::swap(&env, &cfg, &cfg.asset, &cfg.paired, to_swap as i128);
        }
        Self::add_liquidity(&env, &cfg);

        let value_after = Self::value(&env, &Self::position(&env, &cfg), price);
        let gained = value_after - value_before;
        Self::check_slippage(&env, &cfg, gained, amount);

        // Parts au pro-rata de la valeur gagnee, tronque en faveur des
        // deposants en place.
        let total = Self::total_shares(&env);
        let key = DataKey::Shares(from.clone());
        let held: i128 = env.storage().persistent().get(&key).unwrap_or(0);
        let (minted, new_total) = if total == 0 {
            // Genese : toute la valeur (don compris) entre dans le total, les
            // parts mortes y comptent sans appartenir a personne.
            if value_after <= DEAD_SHARES {
                return Err(StrategyError::InvalidSharesMinted);
            }
            (value_after - DEAD_SHARES, value_after)
        } else {
            if value_before <= 0 {
                return Err(StrategyError::InvalidSharesMinted);
            }
            let minted = Self::mul_div_floor(&env, gained, total, value_before);
            if minted <= 0 {
                return Err(StrategyError::InvalidSharesMinted);
            }
            (minted, total + minted)
        };
        env.storage().persistent().set(&key, &(held + minted));
        env.storage()
            .instance()
            .set(&DataKey::TotalShares, &new_total);
        Self::extend_holder(&env, &key);

        event::emit_deposit(
            &env,
            String::from_str(&env, STRATEGY_NAME),
            amount,
            from.clone(),
        );
        // Meme calcul que `balance`, sur la valeur deja lue : le pool n'a pas
        // bouge depuis, et chaque lecture de position coute de la memoire
        // d'invocation.
        Ok(Self::mul_div_floor(
            &env,
            value_after,
            held + minted,
            new_total,
        ))
    }

    fn harvest(_env: Env, _from: Address, _data: Option<Bytes>) -> Result<(), StrategyError> {
        Ok(())
    }

    fn balance(env: Env, from: Address) -> Result<i128, StrategyError> {
        let shares: i128 = env
            .storage()
            .persistent()
            .get(&DataKey::Shares(from))
            .unwrap_or(0);
        if shares == 0 {
            return Ok(0);
        }
        Self::extend_instance(&env);
        let cfg = Self::config(env.clone());
        let price = Self::fresh_price(&env, &cfg);
        let value = Self::value(&env, &Self::position(&env, &cfg), price);
        Ok(Self::mul_div_floor(
            &env,
            value,
            shares,
            Self::total_shares(&env),
        ))
    }

    fn withdraw(env: Env, amount: i128, from: Address, to: Address) -> Result<i128, StrategyError> {
        if amount <= 0 {
            return Err(StrategyError::OnlyPositiveAmountAllowed);
        }
        from.require_auth();
        Self::extend_instance(&env);
        let cfg = Self::config(env.clone());
        let price = Self::fresh_price(&env, &cfg);
        let this = env.current_contract_address();
        let pos = Self::position(&env, &cfg);
        let value_before = Self::value(&env, &pos, price);
        let total = Self::total_shares(&env);
        let key = DataKey::Shares(from.clone());
        let held: i128 = env.storage().persistent().get(&key).unwrap_or(0);
        // Le droit du deposant se juge d'entree, sur la valeur avant
        // l'operation : au-dela, refuse. En deca, une sortie complete ne peut
        // pas echouer d'une unite de parts par arrondi (plafond plus bas).
        if total == 0 || amount > Self::mul_div_floor(&env, value_before, held, total) {
            panic_with_error!(&env, AquaStrategyError::InsufficientShares);
        }

        // Ce que l'oisif ne couvre pas sort du pool : parts LP au prorata de
        // leur valeur equitable, arrondi vers le haut, puis l'apparie recu
        // (et l'oisif apparie) est revendu contre l'actif.
        let need = amount - pos.idle_asset;
        if need > 0 {
            let lp_value = Self::lp_value(&env, &pos, price);
            if lp_value <= 0 {
                return Err(StrategyError::InsufficientBalance);
            }
            let burn = Self::mul_div_ceil(&env, pos.lp_held, need, lp_value).min(pos.lp_held);
            Self::remove_liquidity(&env, &cfg, burn);
        }
        let paired = TokenClient::new(&env, &cfg.paired).balance(&this);
        if paired > 0 {
            Self::swap(&env, &cfg, &cfg.paired, &cfg.asset, paired);
        }

        let obtained = TokenClient::new(&env, &cfg.asset).balance(&this);
        let delivered = obtained.min(amount);
        Self::check_slippage(&env, &cfg, delivered, amount);
        TokenClient::new(&env, &cfg.asset).transfer(&this, &to, &delivered);

        // Parts brulees au prorata du plus grand de la valeur sortie et du
        // montant livre. Une sortie qui coute (frais, glissement) sort plus
        // que le montant livre : le deposant qui sort paie ce cout, pas ceux
        // qui restent. Une sortie qui realise plus que la valeur equitable
        // (pool ecarte du prix de reference : la valeur realisee depasse
        // 2.sqrt(k.p)) laisse l'excedent oisif sur la strategie : ce gain
        // appartient a tous les deposants, le sortant ne le capte pas.
        let value_after = Self::value(&env, &Self::position(&env, &cfg), price);
        let removed = (value_before - value_after).max(delivered);
        let burn = Self::mul_div_ceil(&env, removed, total, value_before).min(held);
        env.storage().persistent().set(&key, &(held - burn));
        env.storage()
            .instance()
            .set(&DataKey::TotalShares, &(total - burn));
        Self::extend_holder(&env, &key);

        event::emit_withdraw(
            &env,
            String::from_str(&env, STRATEGY_NAME),
            delivered,
            from.clone(),
        );
        // Meme calcul que `balance`, sur la valeur deja lue (voir deposit).
        if held == burn {
            return Ok(0);
        }
        Ok(Self::mul_div_floor(
            &env,
            value_after,
            held - burn,
            total - burn,
        ))
    }
}

#[contractimpl]
impl AquariusLpStrategy {
    /// Publie le prix de reference (keeper ou admin). `source_date` date le
    /// taux source (BCE) ; le week-end, le keeper republie celui du vendredi.
    pub fn set_reference_price(env: Env, caller: Address, price: i128, source_date: u64) {
        caller.require_auth();
        if caller != Self::admin(&env) && caller != Self::keeper(env.clone()) {
            panic_with_error!(&env, AquaStrategyError::NotAuthorized);
        }
        let cfg = Self::config(env.clone());
        let last = Self::reference_price(env.clone());
        let now = env.ledger().timestamp();
        if now < last.updated_at + MIN_PUBLISH_INTERVAL {
            panic_with_error!(&env, AquaStrategyError::PriceTooSoon);
        }
        // Pas borne : une cle keeper compromise deplace le prix d'au plus
        // max_step_bps par heure, le temps pour l'admin de reagir.
        let step = (price - last.price)
            .abs()
            .checked_mul(BPS)
            .unwrap_or(i128::MAX);
        if price <= 0 || step > last.price * cfg.max_step_bps as i128 {
            panic_with_error!(&env, AquaStrategyError::PriceStepTooLarge);
        }
        let reference = ReferencePrice {
            price,
            updated_at: now,
            source_date,
        };
        env.storage().instance().set(&DataKey::Price, &reference);
        Self::extend_instance(&env);
        #[allow(deprecated)]
        env.events().publish(
            (symbol_short!("price"), caller),
            (price, source_date, last.price),
        );
    }

    /// Remplace la cle keeper (admin).
    pub fn set_keeper(env: Env, keeper: Address) {
        Self::admin(&env).require_auth();
        env.storage().instance().set(&DataKey::Keeper, &keeper);
        #[allow(deprecated)]
        env.events().publish((symbol_short!("keeper"),), keeper);
    }

    pub fn reference_price(env: Env) -> ReferencePrice {
        env.storage().instance().get(&DataKey::Price).unwrap()
    }

    pub fn config(env: Env) -> Config {
        env.storage().instance().get(&DataKey::Config).unwrap()
    }

    pub fn keeper(env: Env) -> Address {
        env.storage().instance().get(&DataKey::Keeper).unwrap()
    }

    pub fn shares_of(env: Env, owner: Address) -> i128 {
        env.storage()
            .persistent()
            .get(&DataKey::Shares(owner))
            .unwrap_or(0)
    }

    fn admin(env: &Env) -> Address {
        env.storage().instance().get(&DataKey::Admin).unwrap()
    }

    fn total_shares(env: &Env) -> i128 {
        env.storage()
            .instance()
            .get(&DataKey::TotalShares)
            .unwrap_or(0)
    }

    fn fresh_price(env: &Env, cfg: &Config) -> i128 {
        let reference = Self::reference_price(env.clone());
        if env.ledger().timestamp() > reference.updated_at + cfg.max_age {
            panic_with_error!(env, AquaStrategyError::StalePrice);
        }
        reference.price
    }

    fn position(env: &Env, cfg: &Config) -> Position {
        // Lecture directe sur le pool, sans passer par le router.
        let pool = AquaPoolClient::new(env, &cfg.pool);
        let reserves = pool.get_reserves();
        let (r_asset, r_paired) = if cfg.tokens.get(0).unwrap() == cfg.asset {
            (reserves.get(0).unwrap(), reserves.get(1).unwrap())
        } else {
            (reserves.get(1).unwrap(), reserves.get(0).unwrap())
        };
        let this = env.current_contract_address();
        Position {
            r_asset: Self::to_i128(env, r_asset),
            r_paired: Self::to_i128(env, r_paired),
            lp_total: Self::to_i128(env, pool.get_total_shares()),
            lp_held: TokenClient::new(env, &cfg.share_token).balance(&this),
            idle_asset: TokenClient::new(env, &cfg.asset).balance(&this),
            idle_paired: TokenClient::new(env, &cfg.paired).balance(&this),
        }
    }

    /// Valeur equitable des parts LP detenues, en actif :
    /// 2 * sqrt(r_actif * r_apparie * p / PRICE_SCALE) * detenues / total.
    fn lp_value(env: &Env, pos: &Position, price: i128) -> i128 {
        if pos.lp_held <= 0 || pos.lp_total <= 0 {
            return 0;
        }
        // sqrt(r_a * r_p * p / S) = sqrt(r_a * r_p) * sqrt(p * S) / S : les
        // racines sont prises avant le produit par le prix, ce qui tient en
        // u128 jusqu'a des reserves de 1e19 unites par jambe (un trillion
        // d'unites d'actif a 7 decimales), au lieu de 5e14.
        let k = (pos.r_asset as u128)
            .checked_mul(pos.r_paired as u128)
            .unwrap_or_else(|| panic_with_error!(env, AquaStrategyError::MathOverflow));
        let root_price = math::isqrt(price as u128 * PRICE_SCALE as u128);
        let pool_value = (2 * math::isqrt(k))
            .checked_mul(root_price)
            .unwrap_or_else(|| panic_with_error!(env, AquaStrategyError::MathOverflow))
            / PRICE_SCALE as u128;
        Self::mul_div_floor(
            env,
            Self::to_i128(env, pool_value),
            pos.lp_held,
            pos.lp_total,
        )
    }

    /// Valeur totale de la strategie : parts LP a la valeur equitable, plus
    /// l'oisif (l'apparie au prix de reference).
    fn value(env: &Env, pos: &Position, price: i128) -> i128 {
        let idle_paired = Self::mul_div_floor(env, pos.idle_paired, price, PRICE_SCALE);
        Self::lp_value(env, pos, price) + pos.idle_asset + idle_paired
    }

    /// Une perte au-dela de `max_slippage_bps` de `reference` fait tout
    /// revert : `obtained` est ce que l'operation a reellement produit.
    fn check_slippage(env: &Env, cfg: &Config, obtained: i128, reference: i128) {
        if obtained * BPS < reference * (BPS - cfg.max_slippage_bps as i128) {
            panic_with_error!(env, AquaStrategyError::Slippage);
        }
    }

    /// Swap `amount` de `token_in` vers `token_out` par le router Aquarius.
    /// Arbre d'auth verifie en D4 : swap_chained exige l'auth de `user` dans
    /// sa frame (appel direct) puis un transfert sequestre user -> router.
    fn swap(env: &Env, cfg: &Config, token_in: &Address, token_out: &Address, amount: i128) {
        let this = env.current_contract_address();
        env.authorize_as_current_contract(vec![
            env,
            InvokerContractAuthEntry::Contract(SubContractInvocation {
                context: ContractContext {
                    contract: token_in.clone(),
                    fn_name: Symbol::new(env, "transfer"),
                    args: (this.clone(), cfg.router.clone(), amount).into_val(env),
                },
                sub_invocations: Vec::new(env),
            }),
        ]);
        let chain = vec![
            env,
            (cfg.tokens.clone(), cfg.pool_hash.clone(), token_out.clone()),
        ];
        AquaRouterClient::new(env, &cfg.router).swap_chained(
            &this,
            &chain,
            token_in,
            &(amount as u128),
            &1,
        );
    }

    /// Depose tout l'oisif des deux jambes. Le pool exige l'auth de la
    /// strategie dans sa frame, puis tire les deux montants desires et rend
    /// l'excedent : l'arbre exact est pre-autorise.
    fn add_liquidity(env: &Env, cfg: &Config) {
        let this = env.current_contract_address();
        let first = TokenClient::new(env, &cfg.tokens.get(0).unwrap()).balance(&this);
        let second = TokenClient::new(env, &cfg.tokens.get(1).unwrap()).balance(&this);
        if first == 0 || second == 0 {
            return;
        }
        let desired = vec![env, first as u128, second as u128];
        let min_shares: u128 = 0;
        let pull = |token: Address, amount: i128| {
            InvokerContractAuthEntry::Contract(SubContractInvocation {
                context: ContractContext {
                    contract: token,
                    fn_name: Symbol::new(env, "transfer"),
                    args: (this.clone(), cfg.pool.clone(), amount).into_val(env),
                },
                sub_invocations: Vec::new(env),
            })
        };
        env.authorize_as_current_contract(vec![
            env,
            InvokerContractAuthEntry::Contract(SubContractInvocation {
                context: ContractContext {
                    contract: cfg.pool.clone(),
                    fn_name: Symbol::new(env, "deposit"),
                    args: (this.clone(), desired.clone(), min_shares).into_val(env),
                },
                sub_invocations: vec![
                    env,
                    pull(cfg.tokens.get(0).unwrap(), first),
                    pull(cfg.tokens.get(1).unwrap(), second),
                ],
            }),
        ]);
        AquaRouterClient::new(env, &cfg.router).deposit(
            &this,
            &cfg.tokens,
            &cfg.pool_hash,
            &desired,
            &min_shares,
        );
    }

    /// Brule `lp` parts LP contre les deux jambes. Le pool exige l'auth de la
    /// strategie dans sa frame, puis le token de parts celle du `burn`.
    fn remove_liquidity(env: &Env, cfg: &Config, lp: i128) {
        if lp <= 0 {
            return;
        }
        let this = env.current_contract_address();
        let min_amounts = vec![env, 0u128, 0u128];
        env.authorize_as_current_contract(vec![
            env,
            InvokerContractAuthEntry::Contract(SubContractInvocation {
                context: ContractContext {
                    contract: cfg.pool.clone(),
                    fn_name: Symbol::new(env, "withdraw"),
                    args: (this.clone(), lp as u128, min_amounts.clone()).into_val(env),
                },
                sub_invocations: vec![
                    env,
                    InvokerContractAuthEntry::Contract(SubContractInvocation {
                        context: ContractContext {
                            contract: cfg.share_token.clone(),
                            fn_name: Symbol::new(env, "burn"),
                            args: (this.clone(), lp).into_val(env),
                        },
                        sub_invocations: Vec::new(env),
                    }),
                ],
            }),
        ]);
        AquaRouterClient::new(env, &cfg.router).withdraw(
            &this,
            &cfg.tokens,
            &cfg.pool_hash,
            &(lp as u128),
            &min_amounts,
        );
    }

    fn mul_div_floor(env: &Env, a: i128, b: i128, c: i128) -> i128 {
        if c == 0 {
            panic_with_error!(env, AquaStrategyError::MathOverflow);
        }
        a.checked_mul(b)
            .unwrap_or_else(|| panic_with_error!(env, AquaStrategyError::MathOverflow))
            / c
    }

    fn mul_div_ceil(env: &Env, a: i128, b: i128, c: i128) -> i128 {
        if c == 0 {
            panic_with_error!(env, AquaStrategyError::MathOverflow);
        }
        let n = a
            .checked_mul(b)
            .unwrap_or_else(|| panic_with_error!(env, AquaStrategyError::MathOverflow));
        n.checked_add(c - 1)
            .unwrap_or_else(|| panic_with_error!(env, AquaStrategyError::MathOverflow))
            / c
    }

    fn to_i128(env: &Env, v: u128) -> i128 {
        i128::try_from(v)
            .unwrap_or_else(|_| panic_with_error!(env, AquaStrategyError::MathOverflow))
    }

    fn extend_instance(env: &Env) {
        env.storage()
            .instance()
            .extend_ttl(TTL_THRESHOLD, TTL_EXTEND_TO);
    }

    fn extend_holder(env: &Env, key: &DataKey) {
        env.storage()
            .persistent()
            .extend_ttl(key, TTL_THRESHOLD, TTL_EXTEND_TO);
    }
}

#[cfg(test)]
mod test;
#[cfg(test)]
mod test_vault;

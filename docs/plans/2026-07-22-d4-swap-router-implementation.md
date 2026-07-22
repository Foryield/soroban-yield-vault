# D4 Swap Router — Plan d'implémentation

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans (ou
> superpowers:subagent-driven-development en session) pour exécuter ce plan
> tâche par tâche.

**Goal:** Contrat `SwapRouter` (Soroswap aggregator primaire, fallback direct
router Aquarius) avec min-out, fallback atomique, comptabilité des frais et
events D6a, prouvé par un rebalance USDC↔EURC sur testnet.

**Architecture:** Nouveau crate `contracts/router` dans le workspace, vault
intact. Sélection de venue off-chain (`preferred`), garanties on-chain
(min-out revérifié par delta de solde, fallback `try_` atomique). Design
validé : [2026-07-22-d4-swap-router-design.md](2026-07-22-d4-swap-router-design.md).

**Tech Stack:** soroban-sdk 25 (`#[contractclient]` pour les venues,
`#[contractevent]`), proptest, wasm venues vendorisés pour l'intégration
(PR B), stellar CLI pour le testnet (PR C).

**Conventions repo (non négociables) :**

- Commentaires de code Rust SANS accents (convention `contracts/vault/src/lib.rs`).
  Docs markdown AVEC accents.
- Montants en convention 7 décimales `X_XXXXXXX` dans les tests.
- Adresses tierces jamais en dur dans les scripts : relues aux sources.
- Chaque PR mergée sur CI verte (4 checks). Couverture workspace ≥ 90 %.
- Branche de travail : `feat/d4-swap-router` (déjà créée, design committé).

**Sources d'interface vérifiées le 22/07/2026** (règle : citer, jamais déduire) :

- Aggregator Soroswap : `soroswap/aggregator` `contracts/aggregator/src/lib.rs`
  (trait `swap_exact_tokens_for_tokens`) et `src/models.rs` (`DexDistribution`
  à 4 champs : `protocol_id: Protocol`, `path: Vec<Address>`, `parts: u32`,
  `bytes: Option<Vec<BytesN<32>>>` ; `Protocol { Soroswap=0, Phoenix=1,
  Aqua=2, Comet=3 }`).
- Router Aquarius : implémentation de référence
  `contracts/aggregator/src/adapters/aqua.rs` du même repo :
  `swap_chained(user: Address, swaps_chain: Vec<(Vec<Address>, BytesN<32>,
  Address)>, token_in: Address, in_amount: u128, out_min: u128) -> u128`,
  chaque maillon = (paire de tokens TRIÉE par ordre d'adresse, pool_hash,
  token_out). Le repo canonique `AquaToken/soroban-amm` est en 404 : la
  référence vivante est cet adapter + les wasm vendorisés
  `contracts/aggregator/aqua_contracts/*.wasm` (épingler le commit).
- Router Soroswap (utile aux fixtures PR B) :
  `swap_exact_tokens_for_tokens(amount_in, amount_out_min, path, to, deadline)`.

**Amendement de design intégré** (consigné dans le doc de design) : Aquarius
exige un `pool_hash` par paire → registre admin `set_aqua_pool(token_a,
token_b, pool_hash)`. Sans entrée, la venue Aquarius échoue en
`AquaPoolNotSet` (le fallback la traverse proprement).

---

## PR A — contrat routeur + tests mockés

### Task 1 : squelette du crate

**Files:**
- Create: `contracts/router/Cargo.toml`
- Create: `contracts/router/src/lib.rs`

**Step 1 : Cargo.toml** (miroir du vault, sans blend) :

```toml
[package]
name = "swap-router"
description = "ForYield SwapRouter - best-execution Soroswap/Aquarius, min-out, fallback atomique"
version = { workspace = true }
edition = { workspace = true }
license = { workspace = true }
publish = false

[lib]
crate-type = ["cdylib", "rlib"]
doctest = false

[dependencies]
soroban-sdk = "25"

[dev-dependencies]
proptest = "1"
soroban-sdk = { version = "25", features = ["testutils"] }
```

**Step 2 : lib.rs minimal** : en-tête doc (rôle, invariant « solde routeur nul
hors transaction », immuabilité des venues, registre pool Aqua admin),
`contractmeta!`, types publics et erreurs, `initialize` non implémenté :

```rust
#![no_std]
use soroban_sdk::{
    contract, contracterror, contractimpl, contractmeta, contracttype,
    Address, Env,
};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum RouterError {
    AlreadyInitialized = 1,
    AmountMustBePositive = 2,
    MinOutMustBePositive = 3,
    SameToken = 4,
    AquaPoolNotSet = 5,
    AllVenuesFailed = 6,
    SlippageExceeded = 7,
    AmountConversion = 8,
    MathOverflow = 9,
}

#[contracttype]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Venue {
    SoroswapAggregator = 0,
    AquariusRouter = 1,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SwapResult {
    pub amount_out: i128,
    pub venue: Venue,
    pub fee: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PairStats {
    pub volume_in: i128,
    pub volume_out: i128,
    pub fees: i128,
    pub swaps: u64,
}

#[contracttype]
#[derive(Clone)]
enum DataKey {
    Admin,
    SoroswapAggregator,
    AquariusRouter,
    SoroswapFeeBps,
    AquariusFeeBps,
    AquaPool(Address, Address),
    Stats(Address, Address),
}

#[contract]
pub struct SwapRouter;
```

**Step 3 :** `cargo build -p swap-router` → vert. `cargo build --target
wasm32v1-none --release -p swap-router` → vert.

**Step 4 : Commit** `feat(router): squelette du crate swap-router - types et erreurs`

### Task 2 : initialize + getters (TDD)

**Files:**
- Modify: `contracts/router/src/lib.rs`
- Create: `contracts/router/src/test.rs`

**Step 1 : tests qui échouent** : `initialize` stocke admin, venues, fee_bps ;
un second `initialize` → `AlreadyInitialized` (via `try_initialize`, motif
`Err(Ok(RouterError::...))` comme dans `contracts/vault/src/test.rs`).

**Step 2 :** `cargo test -p swap-router` → échec de compilation attendu.

**Step 3 : implémentation** `initialize(admin, soroswap_aggregator,
aquarius_router, soroswap_fee_bps, aquarius_fee_bps)` + getters privés
(`admin`, `venue_addr`, `fee_bps`) + `pair_stats(token_in, token_out) ->
PairStats` (zéros par défaut). Clé de stats sur la paire ORDONNÉE
(token_in, token_out) telle que swappée, pas triée : le sens du flux compte.

**Step 4 :** `cargo test -p swap-router` → vert.

**Step 5 : Commit** `feat(router): initialize immuable + pair_stats`

### Task 3 : clients de venues + mocks

**Files:**
- Create: `contracts/router/src/venues.rs` (+ sous-modules `soroswap`, `aqua`)
- Create: `contracts/router/src/test_mocks.rs` (cfg(test))

**Step 1 : module `venues`** : les types externes répliqués À L'IDENTIQUE
(noms de types ET de champs : l'encodage `contracttype` en dépend) depuis les
sources citées en tête de plan, avec le lien source en commentaire :

```rust
// venues/soroswap.rs
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Protocol { Soroswap = 0, Phoenix = 1, Aqua = 2, Comet = 3 }

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DexDistribution {
    pub protocol_id: Protocol,
    pub path: Vec<Address>,
    pub parts: u32,
    pub bytes: Option<Vec<BytesN<32>>>,
}

#[contractclient(name = "SoroswapAggregatorClient")]
pub trait SoroswapAggregator {
    fn swap_exact_tokens_for_tokens(
        env: Env, token_in: Address, token_out: Address, amount_in: i128,
        amount_out_min: i128, distribution: Vec<DexDistribution>,
        to: Address, deadline: u64,
    ) -> Vec<Vec<i128>>;
}

// venues/aqua.rs
#[contractclient(name = "AquaRouterClient")]
pub trait AquaRouter {
    fn swap_chained(
        env: Env, user: Address,
        swaps_chain: Vec<(Vec<Address>, BytesN<32>, Address)>,
        token_in: Address, in_amount: u128, out_min: u128,
    ) -> u128;
}
```

Chaque sous-module expose `fn attempt(env, addr, token_in, token_out,
amount_in, min_out, ...) -> bool` : construit l'appel (Soroswap :
distribution unique `{Protocol::Soroswap, path=[in,out], parts=1,
bytes=None}`, `deadline = env.ledger().timestamp()` ; Aqua : chaîne d'un
maillon avec paire triée par adresse + pool_hash du registre, conversions
u128 vérifiées → `AmountConversion` si négatif ou > i128::MAX au retour),
appelle la variante `try_`, rend `false` sur toute `Err`. Le succès est
jugé PAR LE ROUTEUR sur delta de solde, pas sur la valeur de retour venue.

**Step 2 : mocks** dans `test_mocks.rs` : `MockAggregator` et `MockAqua`
implémentant EXACTEMENT les signatures des traits, pilotés par storage
(`set_behavior` : montant à servir, ou panique) ; ils tirent `token_in`
depuis `to`/`user` et servent `token_out` (mint), pour exercer le vrai flux
de fonds et l'auth imbriquée.

**Step 3 :** test de fumée : mock répond, client `try_` OK. `cargo test -p
swap-router` → vert.

**Step 4 : Commit** `feat(router): clients de venues (sources citees) + mocks pilotables`

### Task 4 : swap_exact_in — chemin nominal (TDD)

**Files:**
- Modify: `contracts/router/src/lib.rs`, `contracts/router/src/test.rs`

**Step 1 : tests d'abord** : gardes (`amount_in <= 0`, `min_out <= 0`,
`token_in == token_out` → erreurs typées) ; happy path préférée Soroswap :
`from` débité de `amount_in`, crédité de `amount_out >= min_out`, solde
routeur NUL après, `SwapResult { venue: SoroswapAggregator, fee =
amount_in * bps / 10_000 }`, stats incrémentées.

**Step 2 :** rouge. **Step 3 : implémentation** :

```rust
pub fn swap_exact_in(env: Env, from: Address, token_in: Address,
    token_out: Address, amount_in: i128, min_out: i128,
    preferred: Venue) -> SwapResult
{
    from.require_auth();
    // gardes typees...
    let this = env.current_contract_address();
    TokenClient::new(&env, &token_in).transfer(&from, &this, &amount_in);
    let out_token = TokenClient::new(&env, &token_out);
    let before = out_token.balance(&this);

    let order = match preferred {
        Venue::SoroswapAggregator => [Venue::SoroswapAggregator, Venue::AquariusRouter],
        Venue::AquariusRouter => [Venue::AquariusRouter, Venue::SoroswapAggregator],
    };
    let mut venue_used = None;
    for v in order {
        if Self::attempt_venue(&env, v, &token_in, &token_out, amount_in, min_out) {
            let received = out_token.balance(&this) - before;
            if received >= min_out { venue_used = Some((v, received)); break; }
            // venue a menti sur min_out : defense en profondeur
            panic_with_error!(&env, RouterError::SlippageExceeded);
        }
    }
    let (venue, amount_out) =
        venue_used.unwrap_or_else(|| panic_with_error!(&env, RouterError::AllVenuesFailed));
    out_token.transfer(&this, &from, &amount_out);
    // fee, stats, event, SwapResult...
}
```

Pré-autorisation `authorize_as_current_contract` du `transfer(this -> venue,
amount_in)` avant chaque tentative (même motif que `pool_supply` du vault).
Arithmétique : `checked_*` partout, `MathOverflow`.

**Step 4 :** vert. **Step 5 : Commit** `feat(router): swap_exact_in - chemin nominal Soroswap`

### Task 5 : matrice de fallback (TDD)

**Step 1 : tests** : préférée panique → secours sert (venue effective =
secours dans `SwapResult` ET dans les stats) ; préférée sert sous `min_out`
→ la venue revert côté mock (min propagé) → secours sert ; les deux échouent
→ `AllVenuesFailed` ET `from` n'a rien perdu (atomicité) ; préférée =
Aquarius sans `set_aqua_pool` → `AquaPoolNotSet` interne → secours Soroswap
sert.

**Step 2-4 :** rouge → implémentation (déjà largement en place en Task 4,
compléter `attempt_venue` pour Aqua) → vert.

**Step 5 : Commit** `feat(router): fallback atomique - matrice complete`

### Task 6 : registre pool Aqua + conversions (TDD)

**Step 1 : tests** : `set_aqua_pool` admin-only (non-admin → auth échoue) ;
clé = paire TRIÉE par adresse (un pool sert les deux sens) ; conversion
retour u128 > i128::MAX → `AmountConversion`.

**Step 2-4 :** rouge → implémentation → vert.
**Step 5 : Commit** `feat(router): registre admin des pools Aquarius`

### Task 7 : events #[contractevent] + proptest

**Step 1 : tests** : event `swap` émis avec venue EFFECTIVE (cas fallback
couvert) ; champs : from, token_in, token_out, amount_in, amount_out, venue,
fee, min_out. Proptest (motif `contracts/vault/src/test_props.rs`) :
pour toute séquence de swaps mockés, solde du routeur nul dans les deux
tokens après chaque appel, et stats = somme exacte des swaps servis.

**Step 2-4 :** rouge → implémentation `#[contractevent]` struct `SwapEvent`
→ vert.

**Step 5 : Commit** `feat(router): event swap (schema D6a) + invariants proptest`

### Task 8 : clôture PR A

**Step 1 :** relecture du diff complet (imports morts, TODO, code commenté).
**Step 2 :** `cargo test --workspace` vert, `cargo build --target
wasm32v1-none --release` vert, couverture locale si cargo-llvm-cov dispo
(sinon CI fait foi, gotcha connu).
**Step 3 :** push par Pierrick, PR « feat(router): contrat SwapRouter -
best-execution, min-out, fallback atomique », revue subagent contexte frais
(brief = design doc + diff), merge sur CI verte.

---

## PR B — fixtures d'intégration stack réelle

### Task 9 : vendoring des wasm de venues

**Files:**
- Create: `scripts/fetch_test_wasms.sh`
- Create: `contracts/router/test_wasms/README.md` (+ wasm committés)

**Step 1 :** script qui télécharge depuis `soroswap/aggregator` à COMMIT
ÉPINGLÉ : `soroswap_contracts/*.wasm` (factory, router, pair) et
`aqua_contracts/{soroban_liquidity_pool_router_contract,
soroban_liquidity_pool_contract, soroban_liquidity_pool_plane_contract,
soroban_liquidity_pool_liquidity_calculator_contract,
soroban_token_contract}.wasm`, vérifie leurs SHA-256 consignés dans le
README (provenance : repo, commit, date ; motivation : repo canonique Aqua
en 404).
**Step 2 :** committer wasm + README. **Commit**
`test(router): wasm de venues vendorises (provenance epinglee)`

### Task 10 : fixture Soroswap réelle

**Files:**
- Create: `contracts/router/src/test_soroswap_stack.rs`

**Step 1 :** `contractimport!` des wasm ; fixture qui déploie factory (avec
hash du pair wasm), router, crée la paire USDC/EURC (SAC de test), fournit
la liquidité. ATTENTION : notre routeur appelle l'AGGREGATOR, pas le router
Soroswap : déployer aussi l'aggregator réel si son wasm est constructible
(sinon le documenter et tester la venue Soroswap contre le router via un
adapter de test, décision consignée). Spike time-boxé à une demi-journée.
**Step 2 :** test : swap réel via `swap_exact_in`, montant sorti cohérent
avec x*y=k et 0,3 %, auth imbriquée validée (`env.auths()`).
**Step 3 : Commit** `test(router): integration stack Soroswap reelle`

### Task 11 : fixture Aqua réelle (spike time-boxé)

**Files:**
- Create: `contracts/router/src/test_aqua_stack.rs`

**Step 1 :** chaîne d'init Aqua depuis les wasm (router + plane +
calculator + pool hash + `init_standard_pool` + `deposit`) : l'interface
d'init se lit dans le spec embarqué des wasm (`stellar contract info
interface --wasm ...`). Time-box : 1 jour. Si l'init s'avère trop profonde,
repli acté au design : venue Aqua couverte par mocks + preuve testnet PR C,
décision documentée dans le design doc et l'évidence.
**Step 2 :** si GO : test fallback RÉEL (pool Aqua vide ou absent → bascule
Soroswap) + swap Aqua nominal.
**Step 3 : Commit** `test(router): integration stack Aqua reelle` (ou doc du repli)

### Task 12 : clôture PR B

Couverture workspace ≥ 90 % maintenue, relecture diff, push Pierrick,
PR « test(router): fixtures d'integration venues reelles », revue, merge.

---

## PR C — testnet : seed Aquarius, déploiement, évidences

### Task 13 : seed du pool Aquarius testnet

**Files:**
- Create: `scripts/seed_aquarius_pool.sh`

Miroir de `seed_soroswap_pool.sh` : router Aquarius testnet
`CBCFTQSPDBAIZ6R6PJQKSQWKNKWH2QIV3I4J72SHWBIK3ADRRAM5A6GD` (spike S1,
annoncé stable aux resets ; consigné en variable d'environnement documentée,
pas de registre JSON public connu — le documenter), `init_standard_pool`
USDC-Blend/EURC-Circle puis `deposit`, affiche le `pool_hash` (nécessaire à
`set_aqua_pool`). Clé ops `d1-ops` (~17 EURC + 20 USDC en trustline, cf.
mémoire projet ; re-provisionner si insuffisant : emprunt Blend + faucet
Circle, chemins connus de D1/D3). Exécution consignée dans l'évidence.

### Task 14 : déploiement + initialisation du routeur

`stellar contract deploy` du wasm release ; `initialize` avec aggregator
Soroswap et router Aquarius testnet (adresses relues des registres/spike),
`soroswap_fee_bps=30`, `aquarius_fee_bps` = frais du pool créé ;
`set_aqua_pool` avec le hash de Task 13. Contract ID + hashes consignés
le jour même dans `docs/evidence/d4-dex-routing.md`.

### Task 15 : démo best-execution + rebalance + fallback

1. Cotation des deux venues consignée (simulations `--build-only` +
   `stellar tx simulate` sur chaque venue, sorties archivées) : la
   best-execution off-chain du livrable.
2. Chaîne rebalance en 3 hashes : `withdraw` vault USDC → `swap_exact_in`
   (préférée = la meilleure cotation) → `deposit` vault EURC.
3. Preuve fallback : `preferred = AquariusRouter` avec un `min_out` que le
   petit pool Aqua ne peut pas servir mais que Soroswap sert → l'event
   `swap` de la tx montre venue effective = Soroswap.
4. `pair_stats` lu on-chain après la campagne, cohérent avec les swaps.

### Task 16 : évidences + clôture

`docs/evidence/d4-dex-routing.md` complété (section « routing layer » après
la section seed existante), pointeur D4 au README racine, relecture,
push Pierrick, PR « docs(evidence): D4 routing + rebalance testnet »,
merge. Vidéo walkthrough : Pierrick, script de tournage fourni sur demande.
Trailer roadmap si applicable côté foryield-v2.

---

## Vérification finale (avant de déclarer D4 clos)

1. `cargo test --workspace` : 100 % vert, local ET CI.
2. `cargo build --target wasm32v1-none --release` : vert.
3. Couverture CI ≥ 90 % (gate bloquant, la CI fait foi).
4. Évidences complètes : seed Aqua, déploiement, cotations, 3 hashes de
   rebalance, hash de fallback, `pair_stats`.
5. Relecture du design doc : tout écart implémenté ↔ design consigné en
   amendement daté.

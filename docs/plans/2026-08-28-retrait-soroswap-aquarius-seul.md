# Retrait de Soroswap, SwapRouter mono-venue Aquarius, plan d'implementation

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** retirer integralement Soroswap du projet (contrat, tests, fixtures,
scripts, documentation) et faire du SwapRouter un routeur mono-venue sur
Aquarius, redeploye sur testnet.

**Architecture:** simplification a fond plutot que conservation d'une forme a
venues. L'enum `Venue`, le parametre `preferred` et l'ordre d'essai
disparaissent ; `swap_exact_in` appelle directement la venue Aquarius. Le
contrat conserve ce qui garde du sens sans fallback : min-out reverifie sur
delta de solde, comptabilite de frais par paire, registre admin des pools Aqua,
endossement d'auth du tirage de tokens. Venues toujours immuables, donc
redeploiement.

**Tech Stack:** Rust / Soroban SDK, `#![no_std]`, cargo workspace, stellar CLI,
bash pour les scripts d'ops, testnet SDF.

---

## Contexte

Le 28/08/2026, Pierrick rapporte une compromission de Soroswap, confirmee par
le developpeur smart contract de l'equipe et par The Arch dans le mois. Aucune
source publique n'a ete trouvee (recherches du 28/08/2026 : trackers
d'exploits, actualite DeFi 2026, depots et documentation Soroswap). Le perimetre
exact n'etant pas connu, la decision prise est le pire cas : Soroswap est
considere comme integralement compromis et sort du projet.

Phoenix a ete evalue comme venue de remplacement puis ecarte : la creation de
pool y est reservee aux comptes whitelistes par le factory
(`contracts/factory/src/contract.rs:116-122` du depot
`Phoenix-Protocol-Group/phoenix-contracts`, verifie le 28/08/2026), alors que
nos pools Soroswap et Aquarius ont tous deux ete semes permissionnellement par
la cle d'ops. Notre paire USDC-Blend / EURC-Circle ne peut donc pas exister sur
Phoenix sans intervention de leur equipe. Phoenix ne publie par ailleurs aucun
registre d'adresses testnet.

Decision de Pierrick : Aquarius seul, sans venue de remplacement.

### Ce que le projet perd, assume

Le secours atomique disparait. Aquarius devient un point de defaillance unique :
router indisponible, pool vide ou pool non enregistre, et le swap echoue. Le
risque est reel et deja observe : le 28/08/2026, les reserves EURC du pool Aqua
etaient tombees a 0,0127 EURC (contre 1,0 au seed du 22/07/2026), drainees par
des tiers du testnet.

L'exigence D4 "Soroswap aggregator as primary venue" ne peut plus etre
satisfaite. Elle sera reecrite dans l'evidence en documentant l'incident comme
cause, plutot que laissee en ecart silencieux.

### Exposition actuelle, mesuree

Le contrat `CC25CDFP3L65HHHTTFTEYOCXAVQRDVXGG7RWN7EGYB3JMWTTXB2PDAKK` reste
vivant sur testnet et route toujours vers l'aggregator compromis. Il n'expose
aucune pause et ses venues sont immuables : rien ne permet de le neutraliser
on-chain, seul le redeploiement le remplace.

L'exposition reste neanmoins limitee, verifie a l'inventaire du 28/08/2026 :
aucun contrat ForYield n'appelle le SwapRouter (le rebalance est une chaine de
trois transactions manuelles), et ni `web/` ni `onboarding/` ne le referencent.
Le seul appelant est la cle d'ops. Le retrait est un chantier propre, pas une
urgence de securite.

### Chaine d'approvisionnement, risque residuel

Tous les wasm de test viennent du depot `soroswap/aggregator`, y compris les
cinq wasm Aquarius (`soroban_liquidity_pool_*.wasm`, `soroban_token_contract.wasm`),
faute de depot canonique Aqua (`AquaToken/soroban-amm` en 404). Le perimetre de
la compromission cote organisation GitHub est inconnu.

Posture retenue : `scripts/fetch_test_wasms.sh` cesse de pointer vers ce depot,
et les wasm Aquarius deja vendorises sont CONSERVES. Ils sont epingles au commit
`84de10e0` de juillet 2026, anterieur a l'incident rapporte, et leurs SHA-256
sont verifies a chaque execution des tests par
`vendored_wasms_match_sha256sums` (`contracts/router/src/test_stack_common.rs:288`).
Une alteration retroactive du depot ne peut donc pas passer inapercue.

Risque residuel assume et consigne ici : si la compromission etait anterieure au
commit epingle et avait touche les wasm Aqua eux-memes, nos fixtures de test le
refleteraient. Rien ne l'indique, et ces wasm ne servent qu'aux tests, jamais au
deploiement. A rouvrir si un depot canonique Aquarius redevient disponible.

---

## Perimetre

### Supprime

- `contracts/router/src/venues/soroswap.rs` (226 lignes, adaptateur entier)
- `contracts/router/src/test_soroswap_stack.rs` (270 lignes, 4 tests)
- `contracts/router/test_wasms/soroswap_factory.wasm`
- `contracts/router/test_wasms/soroswap_pair.wasm`
- `contracts/router/test_wasms/soroswap_router.wasm`
- `contracts/router/test_wasms/soroswap_aggregator.wasm`
- `scripts/seed_soroswap_pool.sh`

### Modifie

- `contracts/router/src/lib.rs`
- `contracts/router/src/venues.rs`
- `contracts/router/src/venues/aqua.rs` (commentaires de reference uniquement)
- `contracts/router/src/venues/convert.rs` (commentaires uniquement)
- `contracts/router/src/test.rs`
- `contracts/router/src/test_mocks.rs`
- `contracts/router/src/test_stack_common.rs`
- `contracts/router/src/test_aqua_stack.rs`
- `contracts/router/src/test_rebalance.rs`
- `contracts/router/src/test_props.rs`
- `contracts/router/Cargo.toml`
- `contracts/router/test_wasms/README.md`
- `contracts/router/test_wasms/SHA256SUMS`
- `scripts/fetch_test_wasms.sh`
- `scripts/deploy_swap_router.sh`
- `scripts/quote_venues.sh`
- `scripts/seed_aquarius_pool.sh` (mentions indirectes)
- `README.md`
- `docs/the-arch.md`
- `docs/evidence/d4-dex-routing.md`
- `docs/evidence/README.md`
- `contracts/vault/src/lib.rs:27` (mention de roadmap)

### Non touche

Les plans historiques `docs/plans/2026-07-*.md` restent tels quels : ce sont des
documents dates qui decrivent ce qui a ete decide et fait a leur date. Les
reecrire falsifierait l'historique du projet. Le present plan les remplace pour
la suite.

---

## API cible

```rust
pub fn initialize(env: Env, admin: Address, aquarius_router: Address, aquarius_fee_bps: u32)

pub fn swap_exact_in(
    env: Env,
    from: Address,
    token_in: Address,
    token_out: Address,
    amount_in: i128,
    min_out: i128,
) -> SwapResult

pub struct SwapResult { pub amount_out: i128, pub fee: i128 }
```

`enum Venue` supprime. `SwapEvent` perd `venue` et `preferred`, conserve
`from`, `token_in`, `token_out`, `amount_in`, `amount_out`, `fee`, `min_out`.

### Codes d'erreur

Les codes publies ne changent jamais de sens (convention de `lib.rs:55`).

- `5` : REINTRODUIT sous son sens d'origine `AquaPoolNotSet`. Sans fallback a
  traverser, un registre vide n'a plus de chemin vers un echec generique : c'est
  une condition d'ops nette, le client merite le code qui la nomme.
- `6` : `AllVenuesFailed` renomme `VenueFailed`, meme sens (la venue n'a pas
  servi), meme code.
- `2`, `3`, `4`, `7`, `9`, `10` inchanges. `8` reste un trou.

---

## PR A, contrat mono-venue et tests

### Task 1 : retrait de l'enum Venue et de l'adaptateur Soroswap

**Files:**
- Delete: `contracts/router/src/venues/soroswap.rs`
- Modify: `contracts/router/src/venues.rs:1-13`
- Modify: `contracts/router/src/lib.rs` (voir etapes)

**Step 1: supprimer l'adaptateur et sa declaration**

```bash
git rm contracts/router/src/venues/soroswap.rs
```

Dans `contracts/router/src/venues.rs`, retirer `pub mod soroswap;` (ligne 13) et
reecrire la doc de tete au singulier : le module ne decrit plus "chaque
sous-module" mais la venue Aquarius. Conserver le contrat de `attempt` (rend
`false` sur toute `Err`, aucune panique ne traverse) : il reste vrai et reste la
raison pour laquelle le routeur juge sur delta de solde.

**Step 2: retirer Venue de lib.rs**

Supprimer `enum Venue` (`lib.rs:74-82`). Retirer le champ `venue` de
`SwapResult` (`lib.rs:90`), les champs `venue` et `preferred` de `SwapEvent`
(`lib.rs:122-123`), et les variantes `SoroswapAggregator` / `SoroswapFeeBps` de
`DataKey` (`lib.rs:157`, `lib.rs:159`).

**Step 3: run**

Run: `cargo build -p swap-router --target wasm32v1-none --release`
Expected: FAIL, de nombreuses references a `Venue` subsistent dans `lib.rs` et
les tests. C'est attendu, les taches 2 et 3 les traitent.

**Step 4: pas de commit ici**

Cette tache n'est pas committable seule (le crate ne compile pas). Elle est
groupee avec les taches 2 et 3 dans un commit unique en fin de Task 3.

---

### Task 2 : initialize et swap_exact_in mono-venue

**Files:**
- Modify: `contracts/router/src/lib.rs:176-205` (initialize)
- Modify: `contracts/router/src/lib.rs:212-307` (swap_exact_in)
- Modify: `contracts/router/src/lib.rs:355-415` (attempt_venue, a inliner)
- Modify: `contracts/router/src/lib.rs:509-523` (venue_addr, fee_bps)

**Step 1: initialize**

```rust
pub fn initialize(env: Env, admin: Address, aquarius_router: Address, aquarius_fee_bps: u32) {
    if env.storage().instance().has(&DataKey::Admin) {
        panic_with_error!(&env, RouterError::AlreadyInitialized);
    }
    if aquarius_fee_bps > MAX_FEE_BPS {
        panic_with_error!(&env, RouterError::InvalidFeeBps);
    }
    env.storage().instance().set(&DataKey::Admin, &admin);
    env.storage().instance().set(&DataKey::AquariusRouter, &aquarius_router);
    env.storage().instance().set(&DataKey::AquariusFeeBps, &aquarius_fee_bps);
}
```

**Step 2: swap_exact_in**

Gardes d'entree inchangees (`amount_in`, `min_out`, `token_in == token_out`),
`from.require_auth()` inchange, transfert entrant et lecture de `before`
inchanges.

Remplacer la boucle sur `order` par l'appel direct a la venue. Le registre vide
devient une erreur typee AVANT toute tentative :

```rust
let Some(pool_hash) = Self::aqua_pool(&env, &token_in, &token_out) else {
    panic_with_error!(&env, RouterError::AquaPoolNotSet);
};
let venue_addr = Self::venue_addr(&env);
Self::authorize_venue_pull(&env, &venue_addr, &token_in, amount_in);
if !venues::aqua::attempt(
    &env, &venue_addr, &token_in, &token_out, amount_in, min_out, &this, &pool_hash,
) {
    panic_with_error!(&env, RouterError::VenueFailed);
}
let amount_out = out_token
    .balance(&this)
    .checked_sub(before)
    .unwrap_or_else(|| panic_with_error!(&env, RouterError::MathOverflow));
if amount_out < min_out {
    panic_with_error!(&env, RouterError::SlippageExceeded);
}
```

**INVARIANT a preserver dans les commentaires** : `attempt` rendant `false` DOIT
paniquer, jamais retourner. C'est ce qui garantit le revert integral quand la
venue a execute mais que son retour est indecodable. Sans fallback, l'invariant
est plus simple mais tout aussi vital, et le commentaire de `lib.rs:259-263` doit
etre reecrit en ce sens, pas supprime.

La suite (calcul du `fee`, `record_swap`, event, transfert sortant) est
inchangee dans son ordre : ETAT D'ABORD, TRANSFERT ENSUITE (CEI).

**Step 3: helpers**

`attempt_venue` disparait, son corps Aqua etant inline ci-dessus. `venue_addr` et
`fee_bps` perdent leur parametre `venue` et leur `match` :

```rust
fn venue_addr(env: &Env) -> Address {
    env.storage().instance().get(&DataKey::AquariusRouter).unwrap()
}

fn fee_bps(env: &Env) -> u32 {
    env.storage().instance().get(&DataKey::AquariusFeeBps).unwrap()
}
```

**Step 4: doc de tete du module**

Reecrire `lib.rs:2-28`. Points a conserver : venue immuable et redeploiement,
registre admin des pools Aqua, invariant de solde nul hors transaction, modele de
confiance des tokens. Points a reecrire : la best-execution off-chain et le
fallback atomique n'existent plus ; dire ce que le contrat garantit desormais,
c'est-a-dire min-out et revert integral sur une venue unique. Ajouter une ligne
sur la raison du mono-venue, avec la date, en renvoyant au present plan.

Mettre a jour `contractmeta!` (`lib.rs:69-72`) : la valeur mentionne
"best-execution, min-out, fallback atomique".

---

### Task 3 : erreurs et compilation verte du crate

**Files:**
- Modify: `contracts/router/src/lib.rs:42-61`
- Modify: `contracts/router/Cargo.toml:3`

**Step 1: enum RouterError**

```rust
pub enum RouterError {
    AlreadyInitialized = 1,
    AmountMustBePositive = 2,
    MinOutMustBePositive = 3,
    SameToken = 4,
    AquaPoolNotSet = 5,
    VenueFailed = 6,
    SlippageExceeded = 7,
    MathOverflow = 9,
    InvalidFeeBps = 10,
}
```

Reecrire le commentaire de `lib.rs:50-56` : expliquer que 5 est REINTRODUIT sous
son sens d'origine parce que l'architecture mono-venue n'a plus de fallback ou
router un registre vide, que 6 garde son code et son sens sous un nom au
singulier, et que 8 reste un trou. Conserver la regle : un code publie ne change
jamais de sens.

**Step 2: Cargo.toml**

Ligne 3, remplacer la description par
`"ForYield SwapRouter - venue Aquarius, min-out, comptabilite de frais"`.

**Step 3: compiler sans les tests**

Run: `cargo build -p swap-router --target wasm32v1-none --release`
Expected: PASS. Les tests ne compilent pas encore, c'est l'objet des taches 4 a 7.

**Step 4: commit**

```bash
git add contracts/router/src/lib.rs contracts/router/src/venues.rs \
  contracts/router/src/venues/soroswap.rs contracts/router/Cargo.toml
git commit -m "feat(router)!: retire la venue Soroswap, routeur mono-venue Aquarius"
```

---

### Task 4 : mocks et tests unitaires

**Files:**
- Modify: `contracts/router/src/test_mocks.rs:19`, `:84-120`
- Modify: `contracts/router/src/test.rs` (30 tests, dont ~20 touchent Soroswap)

**Step 1: test_mocks.rs**

Supprimer `MockAggregator` / `MockAggregatorClient` et l'import de
`venues::soroswap::DexDistribution`. Conserver les mocks Aqua et le mock de
venue menteuse (celle qui sert moins que `min_out`) : ce dernier prouve la
defense en profondeur du re-controle de `min_out`, qui survit au retrait.

**Step 2: test.rs, tests a supprimer**

Les tests dont l'objet EST le fallback ou la venue Soroswap disparaissent avec
elle :
- `soroswap_attempt_against_mock_moves_real_tokens` (`test.rs:161`)
- `swap_exact_in_serves_via_preferred_soroswap` (`test.rs:426`)
- `fallback_preferred_soroswap_panics_aqua_serves` (`test.rs:498`)
- `preferred_aqua_without_registry_falls_back_to_soroswap` (`test.rs:612`)
- tout test de la matrice de fallback (Task 5 du plan d'implementation de
  juillet) dont les deux venues sont l'objet

**Step 3: test.rs, tests a convertir**

Fixture `init_router` : `soroswap: Address` et `SOROSWAP_FEE_BPS` disparaissent
des arguments. Tous les appels `swap_exact_in` perdent leur dernier argument
`&Venue::...`. Les assertions sur `result.venue` disparaissent.

**Step 4: test.rs, tests a AJOUTER**

Deux comportements changent de nature et doivent etre testes explicitement, sans
quoi le retrait ferait perdre de la couverture :

```rust
#[test]
fn swap_without_registered_pool_fails_with_aqua_pool_not_set() {
    // registre vide : erreur typee 5, plus de traversee silencieuse
    // ...
    assert_eq!(result, Err(Ok(RouterError::AquaPoolNotSet)));
}

#[test]
fn swap_with_failing_venue_fails_with_venue_failed_and_funds_intact() {
    // la venue panique : erreur typee 6, solde de l'appelant inchange,
    // solde du routeur nul
    // ...
}
```

**Step 5: run**

Run: `cargo test -p swap-router --lib test::`
Expected: PASS.

**Step 6: commit**

```bash
git add contracts/router/src/test.rs contracts/router/src/test_mocks.rs
git commit -m "test(router): tests unitaires mono-venue, erreurs typees 5 et 6"
```

---

### Task 5 : fixtures de stack reelle

**Files:**
- Delete: `contracts/router/src/test_soroswap_stack.rs`
- Modify: `contracts/router/src/test_stack_common.rs:27-38`, `:49`, `:108-120`, `:153-198`, `:272-300`
- Modify: `contracts/router/src/lib.rs:538-539` (declaration du module de test)
- Modify: `contracts/router/src/test_aqua_stack.rs:234-250`, `:326`

**Step 1: supprimer le fichier et sa declaration**

```bash
git rm contracts/router/src/test_soroswap_stack.rs
```

Retirer `#[cfg(test)] mod test_soroswap_stack;` de `lib.rs`.

**Step 2: test_stack_common.rs**

Supprimer les `contractimport!` des quatre wasm Soroswap, `deploy_soroswap_stack`,
`SOROSWAP_FEE_BPS`, et adapter `init_router` a la nouvelle signature
d'`initialize`. Supprimer le test
`locally_built_aggregator_wasm_matches_recorded_sha256` (`:272`), qui garde un
wasm qui n'existe plus. CONSERVER `vendored_wasms_match_sha256sums` (`:288`) :
c'est la garde d'integrite des wasm Aqua, et elle prend de l'importance au vu du
risque de chaine d'approvisionnement.

**Step 3: test_aqua_stack.rs**

Supprimer `real_fallback_empty_aqua_pool_served_by_soroswap` (`:326`) et
`setup_fallback_fixture` (`:234-250`), dont l'objet est le fallback croise.
Les trois autres tests de stack Aqua reelle restent, ils sont desormais la
couverture d'integration principale du contrat.

**Step 4: run**

Run: `cargo test -p swap-router --lib test_aqua_stack::`
Expected: PASS, 2 tests (les 3 initiaux moins le fallback croise).

**Step 5: commit**

```bash
git add -A contracts/router/src/
git commit -m "test(router): retire les fixtures de stack Soroswap"
```

---

### Task 6 : rebalance et proptest

**Files:**
- Modify: `contracts/router/src/test_rebalance.rs:84`, `:125`, `:180`
- Modify: `contracts/router/src/test_props.rs:61`, `:74-75`, `:89`, `:99-100`, `:122`, `:178`, `:240`

**Step 1: test_rebalance.rs**

Les deux tests passent par `deploy_soroswap_stack`. Les rebrancher sur le stack
Aqua reel (`test_aqua_stack.rs` fournit le deploiement). Leur objet ne change
pas : la valeur qui sort du vault USDC arrive entiere dans le vault EURC, et un
swap qui echoue sur `min_out` ne laisse rien echoue dans le routeur. Ces deux
tests sont les seuls a franchir la frontiere entre les deux contrats, ils ne
doivent pas etre perdus.

**Step 2: test_props.rs**

Le proptest modelise deux venues. Le ramener a une seule : le modele perd la
dimension de choix de venue, garde l'invariant central (solde du routeur nul,
somme exacte des stats).

**Step 3: run**

Run: `cargo test -p swap-router`
Expected: PASS, suite complete du routeur.

**Step 4: commit**

```bash
git add contracts/router/src/test_rebalance.rs contracts/router/src/test_props.rs
git commit -m "test(router): rebalance et proptest sur la venue unique"
```

---

### Task 7 : cloture PR A

**Step 1: verification complete**

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build -p swap-router --target wasm32v1-none --release
```

Toutes doivent passer. En cas d'echec, lire l'erreur et corriger la cause.

**Step 2: relecture du diff**

```bash
git diff main...HEAD
```

Chercher : imports inutilisees, references residuelles a Soroswap, commentaires
devenus faux (en particulier ceux qui parlent de fallback ou de "deux venues"),
code commente.

```bash
grep -ri "soroswap\|fallback\|preferred\|venue" contracts/router/src/
```

Chaque occurrence restante doit etre justifiee.

---

## PR B, wasm vendorises et scripts d'ops

### Task 8 : wasm de test

**Files:**
- Delete: `contracts/router/test_wasms/soroswap_{factory,pair,router,aggregator}.wasm`
- Modify: `contracts/router/test_wasms/SHA256SUMS`
- Modify: `contracts/router/test_wasms/README.md`
- Modify: `scripts/fetch_test_wasms.sh:15-30`

**Step 1: supprimer les quatre wasm Soroswap**

```bash
git rm contracts/router/test_wasms/soroswap_factory.wasm \
       contracts/router/test_wasms/soroswap_pair.wasm \
       contracts/router/test_wasms/soroswap_router.wasm \
       contracts/router/test_wasms/soroswap_aggregator.wasm
```

**Step 2: SHA256SUMS**

Retirer les trois lignes Soroswap (l'aggregator n'y figure pas, il etait
construit localement). Il doit rester 5 lignes, les wasm Aqua.

**Step 3: fetch_test_wasms.sh**

Le script telechargeait depuis `soroswap/aggregator` a l'execution. Cette source
sort du projet. Deux options ont ete pesees :

- (A) supprimer le script : les wasm sont committes, les tests les verifient par
  SHA-256, le script ne sert qu'au re-epinglage ;
- (B) le conserver en le neutralisant : il ne telecharge plus, il verifie les
  checksums des fichiers committes.

Retenu : (B). La verification d'integrite garde de la valeur, et supprimer le
script ferait disparaitre la trace ecrite de la provenance des wasm Aqua, qui est
precisement ce qu'il faut documenter. Reecrire le script pour qu'il verifie sans
telecharger, avec en tete un commentaire disant explicitement pourquoi la source
amont a ete coupee, avec la date.

**Step 4: README des wasm**

Reecrire la provenance : les wasm Aqua viennent du commit epingle `84de10e0` de
`soroswap/aggregator`, source desormais COUPEE suite a la compromission du
28/08/2026. Conserver la table des SHA-256 et la motivation d'origine (depot
canonique Aqua en 404). Retirer la procedure de re-epinglage, qui n'a plus de
source, et la remplacer par la marche a suivre si un depot canonique Aquarius
redevient disponible.

**Step 5: run**

Run: `cargo test -p swap-router --lib test_stack_common::`
Expected: PASS, `vendored_wasms_match_sha256sums` verifie 5 fichiers.

**Step 6: commit**

```bash
git add -A contracts/router/test_wasms/ scripts/fetch_test_wasms.sh
git commit -m "chore(router): retire les wasm Soroswap, coupe la source amont"
```

---

### Task 9 : scripts d'ops

**Files:**
- Delete: `scripts/seed_soroswap_pool.sh`
- Modify: `scripts/deploy_swap_router.sh:29-33`, `:42-48`, `:55`, `:63-71`
- Modify: `scripts/quote_venues.sh` (reecriture)
- Modify: `scripts/seed_aquarius_pool.sh:4`, `:39`, `:99`

**Step 1: supprimer le seed Soroswap**

```bash
git rm scripts/seed_soroswap_pool.sh
```

**Step 2: deploy_swap_router.sh**

Retirer la resolution de `SOROSWAP_AGGREGATOR` et `SOROSWAP_FEE_BPS`, adapter les
arguments d'`initialize`. Le commentaire de tete sur la fenetre de front-run
reste vrai et reste en place.

**Step 3: quote_venues.sh**

Le script comparait deux venues et designait la meilleure. Sans choix a faire, il
devient un COTATEUR : il imprime la sortie cotee par Aquarius pour un montant
donne, ce dont `min_out` decoule. Le renommer `scripts/quote_aqua.sh` et reecrire
la doc de tete : ce n'est plus un outil de best-execution, c'est l'outil qui
calibre `min_out`. Conserver la logique de parcours des pools par `get_pools` et
`estimate_swap`, ainsi que le tri de paire par octets bruts d'adresse.

**Step 4: seed_aquarius_pool.sh**

Retirer les references a `seed_soroswap_pool.sh`, qui n'existe plus (ligne 4 :
"miroir de seed_soroswap_pool.sh").

**Step 5: verification**

```bash
bash -n scripts/*.sh
grep -ri soroswap scripts/
```
Expected: syntaxe valide, aucune occurrence.

**Step 6: commit**

```bash
git add -A scripts/
git commit -m "chore(scripts): retire Soroswap des scripts d'ops"
```

---

## PR C, testnet et documentation

### Task 10 : re-seed du pool Aquarius

Les reserves EURC du pool sont a 0,0127 (constat du 28/08/2026), le pool doit
etre realimente avant tout redeploiement, sans quoi aucune demonstration n'est
possible.

**Step 1: verifier le solde EURC de la cle d'ops**

Le seed de juillet avait ete reduit de 2/2 a 1/1 faute d'EURC. Verifier avant de
planifier un montant.

**Step 2: re-seed**

```bash
./scripts/seed_aquarius_pool.sh d1-ops <usdc_7dp> <eurc_7dp>
```

Le script est idempotent : le pool existe deja, la creation est sautee, seul le
deposit est rejoue. Relever le `pool_hash` en sortie.

**Step 3: consigner**

Reserves avant et apres, hash de transaction du deposit.

---

### Task 11 : redeploiement du SwapRouter

**Step 1: deployer**

```bash
./scripts/deploy_swap_router.sh d1-ops <aqua_pool_hash>
```

Relever : hash d'upload du wasm, hash de deploiement, hash d'initialize, hash de
`set_aqua_pool`, nouvelle adresse de contrat, SHA-256 du wasm, commit de build.

**Step 2: verifier en simulation**

```bash
stellar contract invoke --id <nouveau> --network testnet --source-account d1-ops --send=no -- \
  aqua_pool_of --token_a <USDC> --token_b <EURC>
```
Expected: le `pool_hash` de la Task 10.

**Step 3: un swap reel de bout en bout**

Coter avec `quote_aqua.sh`, calibrer `min_out` avec une marge de 1 %, soumettre
un `swap_exact_in`, decoder l'event `swap` de la transaction et verifier
`pair_stats` a l'unite. C'est la preuve que la venue unique sert reellement,
preuve que le projet n'a JAMAIS eue pour Aquarius : les deux seuls swaps servis
on-chain l'ont ete par Soroswap, et l'unique transaction `preferred = Aquarius`
etait un fallback vers Soroswap. Cette tache comble ce trou.

**Step 4: verifier le comportement de registre vide**

En simulation uniquement, sur une paire non enregistree, verifier que
`swap_exact_in` echoue en `Error(Contract, #5)`.

---

### Task 12 : documentation

**Files:**
- Modify: `README.md:55`, `:61`, `:144`
- Modify: `docs/the-arch.md:58`, `:271-274`, `:383`, `:396-400`
- Modify: `docs/evidence/d4-dex-routing.md` (nouvelle section, sections existantes conservees)
- Modify: `docs/evidence/README.md:11`
- Modify: `contracts/vault/src/lib.rs:27`

**Step 1: evidence D4**

NE PAS reecrire les sections datees de juillet : elles decrivent ce qui a ete
fait a leur date et restent vraies. Ajouter une section datee du 28/08/2026 qui
enonce la compromission, la decision de retrait, l'eviction de Phoenix avec sa
raison verifiable (whitelist du factory), le redeploiement, et la preuve du swap
Aquarius reel.

Mettre a jour la checklist de cloture : l'exigence "Soroswap aggregator as
primary venue" devient sans objet, motif documente ; l'exigence de fallback
devient sans objet ; les exigences min-out, comptabilite de frais et rebalance
restent et pointent vers leurs nouvelles preuves.

Marquer l'ancien contrat `CC25CDFP...DAKK` comme DEPRECIE, avec la mention
explicite qu'il reste vivant, qu'il route vers une venue compromise, qu'il ne
peut pas etre neutralise on-chain (venues immuables, pas de pause) et qu'il ne
doit plus etre appele.

**Step 2: README et the-arch**

Nouvelle adresse de contrat, description mono-venue, roadmap corrigee.

**Step 3: verifications d'ecriture**

Pas de tirets cadratins dans `docs/`. Accents verifies. Pas d'anglicismes
evitables.

```bash
grep -rnP '\x{2014}' docs/ README.md
grep -rin "soroswap" README.md docs/the-arch.md docs/evidence/
```
La seconde commande doit ne rendre que les mentions historiques assumees
(sections datees de juillet, section d'incident du 28/08).

**Step 4: commit**

```bash
git add README.md docs/ contracts/vault/src/lib.rs
git commit -m "docs: retrait de Soroswap, SwapRouter mono-venue Aquarius"
```

---

## Verification finale

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build -p swap-router --target wasm32v1-none --release
grep -ri soroswap contracts/ scripts/ README.md docs/the-arch.md
```

La derniere commande ne doit rendre aucune occurrence hors des plans historiques
de `docs/plans/`.

Question de cloture, backend : correctness du chemin mono-venue, gestion
d'erreurs (les codes 5 et 6 sont-ils distinguables par un client), securite
(l'endossement d'auth reste-t-il etroit), et surface de test preservee malgre la
suppression de ~24 tests.

## Revue

Apres implementation, lancer un `code-reviewer` en contexte frais avec le brief
d'origine et le diff, sans le contexte d'implementation.

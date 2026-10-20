# D5 : allocateur multi-protocole, plan de conception

Date : 20 octobre 2026. Statut : proposition, à valider par Pierrick avant la
première ligne de code. Cadre fixé par Julien le 20/10 : option (B), appel
direct des stratégies via le trait `defindex-strategy-core`, sur Blend v2 et
une stratégie Aquarius LP écrite par nous, sans Soroswap.

Références : spike D5 ([2026-07-21-spikes-s1.md](2026-07-21-spikes-s1.md)),
schéma d'events D6a
([2026-10-20-d6a-schema-events-conformite.md](2026-10-20-d6a-schema-events-conformite.md)),
retrait de Soroswap
([2026-08-28-retrait-soroswap-aquarius-seul.md](2026-08-28-retrait-soroswap-aquarius-seul.md)).

---

## 1. Ce que D5 doit prouver

Mesure du livrable : allocateur DeFindex sur au moins deux stratégies (Blend
v2 et Aquarius), déclencheurs de rééquilibrage, allocation par lot.
Preuves attendues : une adresse de vault dont la répartition entre stratégies
se lit on-chain, le hash de chaque allocation, une vidéo.

## 2. Constats vérifiés le 20/10

**Le trait.** `defindex-strategy-core` 0.3.0 (crates.io, 31/01/2026, dépend
de `soroban-sdk` 25.0.2, la version du vault). Six fonctions :
`__constructor(asset, init_args)`, `asset()`, `deposit(amount, from)`,
`withdraw(amount, from, to)`, `balance(from)`, `harvest(from, data)`.
Une stratégie est mono-actif et rend ses soldes en unités de l'actif, jamais en
parts. `deposit` et `withdraw` exigent l'auth de `from` ; le transfert
`from -> stratégie` imbriqué se pré-autorise par
`authorize_as_current_contract`, exactement comme `pool_supply` dans notre
vault et comme `invest_in_strategy` dans le vault DeFindex
(`vault/src/strategies.rs`). Le crate fournit aussi les events
`DepositEvent`, `WithdrawEvent` et `HarvestEvent` au format `#[contractevent]`.

**La stratégie Blend de DeFindex** (`defindex-io/stellar-contracts`,
`strategies/blend`, lue au commit `82079667`). Elle tient ses propres parts
internes au-dessus des bTokens Blend et sert `deposit`, `withdraw` et
`balance` sans aucun appel à Soroswap. Soroswap n'intervient que dans
`harvest`, réservé au keeper, qui revend les BLND réclamés contre l'actif via
le routeur Soroswap passé au constructeur (`init_args[2]`).

**La stratégie LP Soroswap de DeFindex** n'est pas un modèle : adresses de
tokens et de paire en dur, `deposit` et `withdraw` rendent `0`, aucune
valorisation. La stratégie Aquarius LP s'écrit donc de zéro.

**Le pool Aquarius** (wasm vendorisés du routeur, interface relue) expose
`deposit`, `withdraw`, `get_reserves`, `get_total_shares`, `share_id`,
`get_info` et `claim`, au niveau du pool comme du routeur ; le pool expose en
plus `pool_type()`. Notre pool USDC/EURC
(`CDYPTHT6TO7IXXIYNTIMN6YYUGN35NE6Y2AXZJOUK3J2ORLKJS7LQDJV`) rend
`constant_product` en simulation le 20/10. Les montants y sont
en `u128`, déjà convertis par `venues::convert` dans le routeur D4.

**Un oracle de change existe sur testnet.** Reflector publie sur la page
Oracle Providers de developers.stellar.org un contrat « Fiat exchange rates »
testnet, `CCSSOHTBL3LEWUCBBEB5NJFC2OKFRC74OWEIJIZLRJBGAAU4VMU5NV4W`. Lu en
simulation le 20/10 : base `USD`, 14 décimales, résolution de 300 secondes,
`lastprice(Other("EUR"))` = 1,1221 USD, horodaté trois minutes avant la
lecture.

**Le rendement LP sur testnet est le nôtre.** Nous détenons 100 % de la
liquidité du pool USDC/EURC (réensemencé le 20/10). Les frais de swap qu'il
gagne viennent des swaps que nous faisons passer, routeur D4 compris. La
stratégie démontre la mécanique d'allocation, pas un rendement de marché. La
note aux reviewers le dira tel quel.

## 3. Décision 1 : la forme du vault

Le vault D1 gère un pool Blend optionnel et immuable, appelé en direct.

- **(A) Les stratégies remplacent l'appel direct.** Le vault ne connaît plus
  que des stratégies au trait DeFindex ; Blend devient l'une d'elles. Le
  crate `blend-contract-sdk` sort des dépendances du vault (il reste en
  dépendance de test). Avantages : un seul chemin d'allocation, un seul
  calcul de `total_assets` (solde oisif plus somme des `balance`), la règle
  d'allocation traite Blend et Aquarius de la même façon. Inconvénients : les
  tests `test_blend.rs` sont réécrits contre la stratégie, et le vault D5 est
  une nouvelle instance (signature d'`initialize` changée). Fichiers :
  `contracts/vault/src/lib.rs`, `test_blend.rs`, `Cargo.toml`.
- **(B) Blend direct conservé, stratégies ajoutées à côté.** Avantage : D1
  intact ligne à ligne. Inconvénients : deux chemins d'allocation, deux
  manières de valoriser, et Blend hors du périmètre « via DeFindex » que
  mesure le livrable.

Choix : **(A)**. Le livrable mesure une allocation via le trait DeFindex, et
(B) laisserait Blend en dehors. Les instances D1 à D4 restent sur le code qui
a produit leurs preuves (étiquette git `d1-d4-preuves` posée avant le premier
commit D5), ce qui rejoint la proposition 5 du plan D6a.

## 4. Décision 2 : quelle instance de la stratégie Blend

- **(A) Une instance déjà déployée par DeFindex sur testnet.** Rien à
  déployer. Mais son keeper n'est pas nous, il peut appeler `harvest` à tout
  moment, et `harvest` passe par le routeur Soroswap inscrit dans sa
  configuration.
- **(B) Notre propre instance, construite depuis la source DeFindex
  épinglée.** Wasm compilé au commit `82079667`, empreinte notée dans
  `docs/evidence/`, keeper = notre clé d'ops. Au paramètre `soroswap_router`,
  on passe l'adresse du routeur Aquarius, qui n'expose pas `router_pair_for`
  (interface du wasm vendorisé relue). Sous le seuil de récompense, `harvest`
  réclame les BLND et s'arrête sans swap ; au-dessus, il échoue en
  `SoroswapPairError` avant le moindre transfert vers un routeur, et toute la
  transaction revert. Soroswap ne peut jamais être appelé.

Choix : **(B)**. La décision de Julien exclut Soroswap, et (A) laisse un tiers
appeler un chemin qui le traverse. Les BLND ne sont simplement pas
réinvestis : sur testnet, ils n'ont pas de valeur à démontrer.

## 5. Décision 3 : valoriser la position LP en USDC

C'est le point dur de D5. La stratégie Aquarius est mono-actif (USDC Blend)
mais détient des parts d'un pool USDC/EURC. `balance` doit les exprimer en
USDC, et cette valeur entre dans `total_assets`, donc dans le prix de la part
du vault. Toute valeur manipulable dans une transaction se convertit en vol :
gonfler `total_assets`, retirer, rétablir.

- **(A) Valeur au comptant.** Quote-part des réserves, jambe EURC convertie au
  prix du pool lui-même. Aucune dépendance. Manipulable : un swap qui
  déséquilibre le pool déplace cette valeur dans la même transaction, et notre
  pool est petit, donc l'attaque coûte peu.
- **(B) Valeur équitable du LP, prix de l'oracle.** Pour un pool à produit
  constant, `V = 2 · sqrt(r_usdc · r_eurc · p) · parts / total_parts`, où `p`
  est le cours EUR/USD de Reflector. Le produit `r_usdc · r_eurc` ne bouge
  pas sous un swap (il ne fait que croître des frais), donc aucun swap ne
  déplace `V`. Avec la valeur au prix d'oracle `r_usdc + p · r_eurc`, en
  revanche, tout écart au prix de marché fait monter la valeur : elle est à
  écarter. Inconvénients : une dépendance externe nouvelle (Reflector), une
  garde de fraîcheur à définir, et la formule ne vaut que pour un pool à
  produit constant, ce que le constructeur vérifie via `pool_type()` du pool.
- **(C) Valeur d'acquisition.** La stratégie déclare le montant investi moins
  le montant retiré, et ne constate gain ou perte qu'à la sortie. Aucune
  manipulation possible, aucune dépendance. Mais la valeur liquidative ment
  entre deux sorties : une perte de divergence non constatée est payée par
  ceux qui restent quand un autre sort.

Choix proposé : **(B)**. C'est la seule option qui donne une valeur
liquidative exacte sans prise pour un attaquant. Elle suppose d'accepter
Reflector comme dépendance, ce qui est une décision à confirmer (section 12).
Si elle est refusée, (C) avec plafond d'allocation bas est le repli : (A) ne
l'est pas.

Détails de (B) :

- Racine carrée entière sur `i128`, arrondie vers le bas ; le facteur
  d'échelle des 14 décimales de Reflector est appliqué avant la racine pour ne
  pas perdre de précision. Valeur tronquée en faveur du vault, comme
  `strategy_assets` aujourd'hui.
- Fraîcheur : un cours plus vieux que `max_age` (proposé : 3 600 secondes, soit
  douze périodes de Reflector) fait échouer `balance`. Conséquence : dépôts et
  retraits du vault échouent tant que l'oracle est muet, comme ils échouent
  aujourd'hui si Blend gèle sa réserve. La sortie d'urgence (section 7) reste
  disponible, elle ne valorise pas.
- Sortie : `withdraw(amount)` brûle les parts LP nécessaires au prix
  équitable, reçoit USDC et EURC, revend l'EURC contre l'USDC par le pool, et
  exige que l'USDC obtenu atteigne `amount · (10 000 − max_slippage_bps) /
  10 000`, sinon tout revert. Entrée symétrique : la moitié de l'USDC est
  convertie en EURC, puis les deux jambes sont déposées avec un `min_shares`
  calculé sur la valeur équitable.

## 6. Décision 4 : allocation et rééquilibrage

- **(A) Règle on-chain, déclenchement libre.** L'admin fixe une cible en
  points de base par stratégie (somme ≤ 10 000, le reste en oisif) et un
  plafond par stratégie. Les dépôts restent oisifs. `rebalance()` est
  appelable par tous : elle calcule l'écart à la cible de chaque stratégie et
  déplace tout en une transaction, retraits d'abord, dépôts ensuite. Elle ne
  s'exécute que si un déclencheur est franchi : oisif au-dessus de
  `idle_trigger`, ou écart d'une stratégie au-dessus de `drift_bps`, sinon
  `NothingToRebalance`.
- **(B) Instructions calculées hors chaîne** (modèle du vault DeFindex,
  `rebalance(caller, instructions)`, réservé au manager). Plus souple, mais la
  décision d'allocation vit hors chaîne et n'est pas reproductible depuis les
  seuls events.

Choix : **(A)**. Le champ 41 du schéma D6a demande un identifiant stable de
l'algorithme qui décide (`fy_alloc_v1`) : une règle on-chain déterministe le
rend vérifiable par quiconque relit les events. C'est aussi ce que mesure le
livrable : des déclencheurs et une allocation par lot, visibles on-chain. Un
dépôt reste oisif jusqu'au prochain lot, ce qui allège son coût et évite un
swap Aquarius à chaque dépôt.

Retraits clients : servis d'abord par l'oisif, puis par les stratégies dans
l'ordre de liquidité déclaré (Blend avant Aquarius), sans attendre de
rééquilibrage. Le montant dû reste calculé sur `total_assets` avant
l'opération, comme en D1 ; le vault vérifie le delta de son solde après
chaque `withdraw` de stratégie et ne fait jamais confiance à la valeur de
retour (même règle que le routeur D4 envers ses venues).

## 7. Plafonds et sortie d'urgence

- Plafond par stratégie en points de base de `total_assets`, vérifié par
  `rebalance` après exécution.
- `emergency_exit(strategy)` (admin) : retire tout ce que la stratégie rend,
  marque la stratégie inactive (cible forcée à 0), émet `emergency_exit` avec
  le montant récupéré. Elle ne valorise pas, donc fonctionne oracle muet. Elle
  ferme le risque accepté en D1 (« aucune fonction de désallocation
  d'urgence »).
- La pause existante bloque dépôts, retraits et `rebalance`, pas
  `emergency_exit`.

## 8. Esquisse d'interface

```rust
initialize(admin, asset, strategies: Vec<StrategyConfig>, params: AllocParams)

struct StrategyConfig { address: Address, target_bps: u32, cap_bps: u32, active: bool }
struct AllocParams { idle_trigger: i128, drift_bps: u32 }

deposit(from, amount) -> i128          // inchangé côté appelant, reste oisif
withdraw(from, shares) -> i128         // inchangé côté appelant
total_assets() -> i128                 // oisif + somme des balance actives
allocation() -> Vec<(Address, i128)>   // répartition lisible, preuve du livrable
rebalance() -> Vec<(Address, i128)>    // mouvements effectués
set_allocation(strategies)             // admin, event config_set
emergency_exit(strategy) -> i128       // admin
```

Le nombre de stratégies est borné (proposé : 4) pour borner le coût de
`total_assets`, qui appelle `balance` sur chacune.

Stratégie Aquarius, nouveau membre `contracts/strategy-aquarius` :
`__constructor(asset, [aqua_router, pool_hash, paired_token, oracle,
max_age, max_slippage_bps])`, plus `harvest` sans effet (les récompenses AQUA
ne sont pas réclamées en D5).

## 9. Events

Ceux du plan D6a, au format `#[contractevent]` : `allocate` et `deallocate`
par mouvement de `rebalance` (`decided_by` = adresse de l'appelant, `algo` =
`fy_alloc_v1`, `reason` = déclencheur franchi), `emergency_exit`,
`config_set`. Les stratégies émettent en plus les events du crate. Leur forme
finale suit les réponses de Julien au plan D6a ; ce plan n'en fige aucune.

## 10. Tests

- **Pile réelle.** Blend via `blend-contract-sdk` testutils (déjà en place),
  wasm de la stratégie Blend compilé au commit épinglé et vendorisé avec son
  SHA-256 (même convention que `contracts/router/test_wasms`), wasm Aquarius
  vendorisés existants. Réserve connue : ils diffèrent du wasm déployé
  aujourd'hui (journal D4 du 20/10), le testnet reste le juge final. Reflector
  simulé par un mock au même client.
- **Invariants (proptest).** `total_assets == oisif + somme des balance` ;
  `rebalance` ne change pas le prix de la part au-delà de l'arrondi et du
  glissement borné ; somme des cibles ≤ 10 000 ; aucun plafond dépassé après
  `rebalance` ; un retrait rend au plus sa quote-part.
- **Attaques.** Swap massif dans le pool Aquarius avant un dépôt ou un
  retrait : la valeur équitable ne bouge pas, l'attaquant ne gagne rien.
  Stratégie qui ment sur sa valeur de retour : le vault juge au delta de
  solde. Oracle périmé : `balance` échoue, `emergency_exit` passe.
- **Régression.** Les tests de parts D1 (`test.rs`, `test_matrix.rs`,
  `test_props.rs`) passent sans changement d'assertion contre un vault sans
  stratégie.

## 11. Ordre d'exécution

Chaque étape se committe seule, tests verts.

1. Étiquette `d1-d4-preuves` sur `main`.
2. Vault : liste de stratégies au trait DeFindex, Blend direct retiré, une
   stratégie factice de test (au trait) ; tests D1 verts.
3. Stratégie Blend DeFindex vendorisée, tests sur la pile Blend réelle.
4. `rebalance`, déclencheurs, plafonds, `emergency_exit`.
5. `contracts/strategy-aquarius` : valeur équitable, entrée, sortie, gardes.
6. Tests d'invariants et d'attaque sur l'ensemble.
7. Scripts de déploiement et de preuve, déploiement testnet, preuves
   archivées le jour même (`scripts/archive_proof.sh d5 <hash>`), journal
   `docs/evidence/d5-defindex-allocator.md`.
8. Revue de sécurité en contexte frais avant le déploiement testnet.

## 12. À trancher avant de coder

1. **Reflector comme dépendance de la valorisation LP** (section 5, option B).
   Sans elle, repli sur la valeur d'acquisition avec plafond bas.
2. **Déclenchement libre de `rebalance`** (section 6). L'alternative est de le
   réserver à une clé de keeper ; le champ `decided_by` devient alors cette
   clé.
3. **Valeurs initiales** proposées pour la démonstration : cibles Blend 60 %,
   Aquarius 30 %, oisif 10 % ; plafonds 80 % et 40 % ; `drift_bps` 500 ;
   `max_slippage_bps` 100, la marge déjà employée en D4.

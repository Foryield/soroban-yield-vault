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

**Reflector, examiné et écarté.** Reflector publie un oracle « Fiat
exchange rates » testnet, `CCSSOHTBL3LEWUCBBEB5NJFC2OKFRC74OWEIJIZLRJBGAAU4VMU5NV4W`
(page Oracle Providers de developers.stellar.org). Lu en simulation le 20/10 :
base `USD`, 14 décimales, EUR à 1,1221. La lecture est gratuite (variante
publique `pulse-contract`, sans abonnement). Mais les neuf devises du contrat
échoient le 9/11/2026 (`expires`), et la source des nœuds
(`reflector-node`, `oracle-runner.js`) ne publie plus un actif échu. La
prolongation se paie en XRF (`extend_asset_ttl`, 200 XRF par jour selon
`fee_config`), en vrai argent sur mainnet. Règle posée par Pierrick le 20/10 :
une dépendance payante est exclue. D'où l'option (D) de la section 5.

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
- **(B) Valeur équitable au cours Reflector.** Formule ci-dessous, cours lu
  sur l'oracle Reflector. Écartée : lire est gratuit, mais le flux échoit et
  se paie pour rester publié (section 2).
- **(C) Valeur d'acquisition.** La stratégie déclare le montant investi moins
  le montant retiré, et ne constate gain ou perte qu'à la sortie. Aucune
  manipulation possible, aucune dépendance. Mais la position est à moitié
  exposée à l'euro : un mouvement de 5 % de l'EUR déplace sa valeur réelle
  d'environ 2,5 % sans que le vault le voie, soit jusqu'à 0,75 % de valeur
  liquidative sous un plafond de 30 %, payés par ceux qui restent quand un
  autre sort.
- **(D) Valeur équitable au prix de référence publié par notre keeper.** Même
  formule que (B), mais le cours EUR/USD est poussé chaque jour par notre clé
  keeper depuis le taux de référence de la BCE, gratuit. Coût nul, valeur
  liquidative juste. C'est le modèle de foryield-v2, qui tient la valorisation
  côté plateforme avec validation par le gérant, jamais par oracle on-chain
  (plan D4 du 22/07). Inconvénients : le keeper devient une clé de confiance
  pour le prix, et un script de publication quotidien est à faire tourner.

Choix : **(D)**, arrêté le 20/10. (A) est exclue ; (C) reste le repli si la
publication quotidienne s'avérait intenable.

Détails de (D) :

- Formule. Pour un pool à produit constant,
  `V = 2 · sqrt(r_usdc · r_eurc · p) · parts / total_parts`, où `p` est le
  prix de référence. Le produit `r_usdc · r_eurc` ne bouge pas sous un swap
  (il ne fait que croître des frais), donc aucun swap ne déplace `V`. La
  valeur au prix de référence `r_usdc + p · r_eurc`, elle, monte dès que le
  pool s'écarte du marché : elle est à écarter. Le constructeur vérifie
  `pool_type() == constant_product`.
- Calcul. Racine carrée entière sur `i128`, arrondie vers le bas, échelle
  appliquée avant la racine pour garder la précision. Valeur tronquée en
  faveur du vault, comme `strategy_assets` aujourd'hui.
- Publication. `set_reference_price(price, source_date)`, keeper seul, émet un
  event avec le prix, la date du taux BCE et l'adresse du keeper. Gardes : pas
  de plus de `max_step_bps` (100) d'un prix au suivant, une publication par
  heure au plus. Un keeper compromis déplace donc le prix d'au plus 1 % par
  heure, le temps pour l'admin de mettre en pause et de remplacer la clé. Le
  week-end, sans nouveau taux BCE, le keeper republie celui du vendredi.
  Hypothèse : l'USDC Blend vaut 1 USD, le taux EUR/USD sert de prix
  EURC/USDC.
- Fraîcheur. Un prix plus vieux que `max_age` (72 heures depuis la revue
  de sécurité du 21/10, voir section 13) fait échouer `balance` : dépôts et
  retraits du vault échouent tant que le keeper est muet, comme ils
  échouent aujourd'hui si Blend gèle sa réserve. Le remède est de republier
  le prix ; la sortie d'urgence (section 7) reste disponible, mais un
  passage en perte exige la pause du vault et ne se justifie pas pour une
  panne qui se répare.
- Écart pool contre prix de référence : la valeur équitable est celle qu'on
  réalise quand le pool cote au prix de marché. Un pool décalé fait perdre à
  l'entrée comme à la sortie, et la borne de glissement ci-dessous, mesurée
  contre la valeur équitable, le refuse déjà : pas de garde séparée. Sur
  testnet, le pool USDC/EURC cotait 1,00 le 20/10 contre environ 1,12 au
  marché ; le 21/10, un robot tiers l'arbitrait vers son propre prix (section
  16). La jambe Aquarius porte donc le FYEUR, euro de test que nous émettons,
  sans marché tiers. Nous tenons le rôle de l'arbitragiste : un script
  `align_aquarius_pool.sh` ramène le pool au prix de référence par un swap,
  avant chaque mouvement Aquarius.
- Sortie : `withdraw(amount)` brûle les parts LP nécessaires au prix
  équitable, reçoit USDC et EURC, revend l'EURC contre l'USDC par le pool, et
  exige que l'USDC obtenu atteigne `amount · (10 000 − max_slippage_bps) /
  10 000`, sinon tout revert. Entrée symétrique : la moitié de l'USDC est
  convertie en EURC, puis les deux jambes sont déposées avec un `min_shares`
  calculé sur la valeur équitable.

## 6. Décision 4 : allocation et rééquilibrage

- **(A) Règle on-chain.** L'admin fixe une cible en points de base par
  stratégie (somme ≤ 10 000, le reste en oisif) et un plafond par stratégie.
  Les dépôts restent oisifs. `rebalance()` calcule l'écart à la cible de
  chaque stratégie et déplace tout en une transaction, retraits d'abord,
  dépôts ensuite. Elle ne s'exécute que si une poche, oisif compris, s'écarte
  de sa cible de plus de `drift_bps` points de base de `total_assets`, sinon
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

Qui appelle `rebalance` : **le keeper, l'admin en secours**, plutôt que tout
le monde. Le keeper existe déjà pour le prix de référence (section 5), et un
mouvement Aquarius n'est sain que juste après l'alignement du pool : le keeper
enchaîne prix, alignement, rééquilibrage. Ouvert à tous, `rebalance` offrirait
un sandwich (déséquilibrer le pool, déclencher, rééquilibrer), borné à 1 % mais
répétable. Le keeper choisit le moment, jamais la répartition : cibles,
plafonds et déclencheur restent vérifiés on-chain.

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
  le montant récupéré. Elle ne valorise pas, donc fonctionne même keeper muet. Elle
  ferme le risque accepté en D1 (« aucune fonction de désallocation
  d'urgence »).
- La pause existante bloque dépôts, retraits et `rebalance`, pas
  `emergency_exit`.

## 8. Esquisse d'interface

```rust
initialize(admin, asset, strategies: Vec<StrategyConfig>, params: AllocParams)

struct StrategyConfig { address: Address, target_bps: u32, cap_bps: u32, active: bool }
struct AllocParams { drift_bps: u32 }

deposit(from, amount) -> i128          // inchangé côté appelant, reste oisif
withdraw(from, shares) -> i128         // inchangé côté appelant
total_assets() -> i128                 // oisif + somme des balance actives
allocation() -> Vec<(Address, i128)>   // répartition lisible, preuve du livrable
rebalance(caller) -> Vec<(Address, i128)>  // keeper ou admin, mouvements effectués
set_allocation(strategies)             // admin, event config_set
emergency_exit(strategy) -> i128       // admin
```

Le nombre de stratégies est borné (proposé : 4) pour borner le coût de
`total_assets`, qui appelle `balance` sur chacune.

Stratégie Aquarius, nouveau membre `contracts/strategy-aquarius` :
`__constructor(asset, [aqua_router, pool_hash, paired_token, keeper,
initial_price, max_age, max_step_bps, max_slippage_bps])`, plus
`set_reference_price(price, source_date)` (keeper), `reference_price()` en
lecture, et `harvest` sans effet (les récompenses AQUA ne sont pas réclamées
en D5).

## 9. Events

Ceux du plan D6a, au format `#[contractevent]` : `allocate` et `deallocate`
par mouvement de `rebalance` (`decided_by` = adresse du keeper ou de l'admin, `algo` =
`fy_alloc_v1`, `reason` = déclencheur franchi), `emergency_exit`,
`config_set`. Les stratégies émettent en plus les events du crate. Leur forme
finale suit les réponses de Julien au plan D6a ; ce plan n'en fige aucune.

## 10. Tests

- **Pile réelle.** Blend via `blend-contract-sdk` testutils (déjà en place),
  wasm de la stratégie Blend compilé au commit épinglé et vendorisé avec son
  SHA-256 (même convention que `contracts/router/test_wasms`), wasm Aquarius
  vendorisés existants. Réserve connue : ils diffèrent du wasm déployé
  aujourd'hui (journal D4 du 20/10), le testnet reste le juge final.
- **Invariants (proptest).** `total_assets == oisif + somme des balance` ;
  `rebalance` ne change pas le prix de la part au-delà de l'arrondi et du
  glissement borné ; somme des cibles ≤ 10 000 ; aucun plafond dépassé après
  `rebalance` ; un retrait rend au plus sa quote-part.
- **Attaques.** Swap massif dans le pool Aquarius avant un dépôt ou un
  retrait : la valeur équitable ne bouge pas, l'attaquant ne gagne rien.
  Stratégie qui ment sur sa valeur de retour : le vault juge au delta de
  solde. Prix de référence périmé : `balance` échoue, `emergency_exit` passe.
  Keeper compromis : un prix hors du pas autorisé ou publié trop tôt est
  refusé.
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
7. Scripts de déploiement et de preuve, `push_reference_price.sh` (taux BCE
   du jour vers `set_reference_price`, lancé chaque jour par une tâche
   planifiée) et `align_aquarius_pool.sh`, déploiement testnet, preuves
   archivées le jour même (`scripts/archive_proof.sh d5 <hash>`), journal
   `docs/evidence/d5-defindex-allocator.md`.
8. Revue de sécurité en contexte frais avant le déploiement testnet.

## 12. Décisions arrêtées le 20/10

1. **Prix de référence publié par notre keeper** (section 5, option D),
   Reflector écarté : son flux se paie pour rester vivant.
2. **`rebalance` réservé au keeper, l'admin en secours** (section 6).

## 13. Valeurs de démonstration (validées le 20/10, pool revu le 21/10)

Elles se déduisent de la profondeur du pool Aquarius. Le 20/10, l'EURC de
Circle limitait le pool à 15 EURC ; depuis le 21/10, la paire est USDC /
FYEUR (section 16) et seul l'USDC de la clé d'ops la limite.

- **Pool** amorcé au taux BCE du 21/10 (1,1234) : 100 FYEUR et 112,34 USDC.
  L'EURC de Circle reste réservé à l'instance EURC de D6b.
- **Dépôt de démonstration** : 2 USDC. La jambe Aquarius vaut alors 0,4 USDC,
  dont 0,2 swappés, soit 0,18 % de la profondeur du pool. Mesuré en
  simulation le 21/10 : un dépôt direct de 2 USDC dans la stratégie est
  valorisé 1,998 USDC, soit 0,1 % de coût d'entrée, sous la borne de 1 %.
- **Cibles** : Blend 70 %, Aquarius 20 %, oisif 10 %. Aquarius porte le plus
  de risques (prix de référence, pool étroit, perte de divergence), il
  reçoit la plus petite part ; l'oisif sert les petits retraits sans toucher aux stratégies.
- **Plafonds**, fixés à l'`initialize` et immuables : Blend 90 %, Aquarius
  30 %. Les cibles se règlent ensuite, toujours sous les plafonds.
- **Déclencheur unique** : un écart de plus de 5 points de `total_assets`
  sur l'une des trois poches, oisif compris (`drift_bps` = 500).
- **Glissement maximal** : 1 % par mouvement contre la valeur équitable, la
  marge de D4. Il suppose le pool aligné sur le prix de référence juste
  avant (section 5).
- **Âge maximal du prix de référence** : 72 heures. Le plan visait 26
  heures (publication quotidienne plus deux heures de marge) ; la revue de
  sécurité du 21/10 l'a relevé, car un prix périmé gèle tout le vault et 72
  heures couvrent un week-end ou une tâche quotidienne manquée. Pas d'un
  prix à l'autre borné à 1 %, une publication par heure au plus.

Séquence filmée : dépôt de 2 USDC (oisif à 100 %), rééquilibrage (1,4 / 0,4
/ 0,2), dépôt de 0,05 USDC refusé au rééquilibrage (`NothingToRebalance`,
écart de 2,2 points), dépôt de 0,2 USDC accepté (écart de 10 points),
retrait de 0,15 USDC servi par l'oisif, retrait de 1 USDC servi par Blend.

## 14. Écarts nés de l'implémentation (20/10)

Étapes 2 à 4 codées sur `feat/d5-vault-strategies`. Ce qui diffère des
sections précédentes, et pourquoi.

- **Constructeur au lieu d'`initialize`.** `initialize` n'exigeait aucune
  signature : entre le déploiement et l'initialisation, un tiers pouvait
  poser ses propres stratégies et son propre keeper. Le vault se configure
  désormais par `__constructor(admin, keeper, asset, strategies, drift_bps)`,
  dans la transaction même du déploiement ; l'hôte refuse tout second appel.
- **Interface finale.** `rebalance(caller)`, `set_targets(targets)`,
  `set_keeper(keeper)` (remplacer une clé keeper compromise),
  `emergency_exit(strategy, write_off)`, `allocation()`, `strategies()`.
- **Sortie d'urgence à deux modes.** Sans passage en perte, la stratégie doit
  tout rendre, sinon rien ne change : un pool gelé immobilise les fonds sans
  les perdre. Avec `write_off`, décision explicite de l'admin, la stratégie
  est retirée même en panne et ce qu'elle ne rend pas est passé en perte. Une
  stratégie retirée sort du périmètre : `total_assets`, retraits, lots.
- **Perte bornée par lot.** 1 % du volume déplacé, mesuré comme le plus
  grand des deux flux (retraits, dépôts) et non leur somme. Le déclencheur
  doit dépasser cette marge (`drift_bps` > 100), pour que l'écart laissé par
  une perte tolérée ne relance pas de lot : les pertes ne se composent pas.
- **Plafonds** vérifiés sur le plus grand des totaux avant et après le lot :
  une perte d'arrondi tolérée ailleurs ne fait pas franchir son plafond à une
  stratégie dont la cible l'égale.
- **Stratégie Blend vendorisée en binaire seul** (licence GPL-3.0 de
  DeFindex, option A du 20/10) : `contracts/vault/test_wasms/`, build
  reproductible vérifié par `scripts/build_blend_strategy.sh`.
- **Parts mortes de la stratégie Blend.** Son premier déposant cède 1 000
  unités (0,0001 USDC). Le script de déploiement fera un dépôt d'amorçage
  depuis la clé d'ops avant de brancher la stratégie au vault.
- **Arrondis.** Mesurés sur la pile Blend réelle après un an d'intérêts : la
  stratégie rend exactement le montant demandé. Le vault garde son contrôle
  strict (`StrategyShortfall`), sans tolérance de poussière.
- **Ordre.** Les scénarios vault-Blend de D1 ont été portés après
  `rebalance` (étape 4), seul chemin qui verse aux stratégies.

**Keeper muet, résolu à l'étape 5.** Un prix de référence périmé fait
échouer `balance`, donc la sortie d'urgence sans passage en perte. L'admin
peut publier le prix lui-même et remplacer le keeper de la stratégie : le
prix redevient frais et la sortie normale refonctionne. Aucun chemin de
sortie ne contourne le prix.

## 15. Étape 5 : stratégie Aquarius (20/10)

`contracts/strategy-aquarius`, au trait DeFindex. Écarts et constats.

- **Coût de sortie au partant** (décision du 20/10, option A). Sortir d'une
  position LP coûte toujours (frais, glissement). Le vault paie au client
  sortant ce qui rentre effectivement, dans la marge de 1 % de ce qui est
  demandé aux stratégies ; la stratégie brûle les parts du partant au
  prorata de la valeur réellement sortie. Ceux qui restent ne paient pas le
  départ des autres.
- **Prix de référence** publiable par le keeper ou l'admin, pas de 1 % par
  publication, une par heure au plus, péremption à 72 heures.
- **Taille d'une opération.** Le contrôle de valeur équitable refuse toute
  entrée ou sortie qui coûte plus de 1 %. Sur un pool à 0,3 % de frais, cela
  borne une opération à environ 1 à 2 % de la profondeur du pool : un dépôt
  de 3,3 % du pool est refusé. La démonstration (0,4 USDC dans un pool de
  112,34 USDC depuis le 21/10) en est loin (section 13).
- **Réalignement obligatoire.** Une entrée de 20 USDC dans un pool de 1 122
  le décale d'environ 1,8 % ; l'entrée suivante est alors refusée tant que
  `align_aquarius_pool.sh` ne l'a pas ramené au prix de référence.
- **Calcul.** La racine du produit des réserves est prise avant le produit
  par le prix : valeur exacte jusqu'à 1e19 unités par jambe (la première
  version débordait vers 5e14, ce qui aurait bloqué les fonds).

**Risque accepté.** La sortie revend l'EURC par un swap. Si l'admin
d'Aquarius coupe les swaps du pool (`kill_swap`, mode d'urgence, déjà notés
au journal D4), la sortie échoue proprement, parts intactes, jusqu'à leur
réouverture.

**Bloquant avant toute preuve testnet** (étape 7) :

1. Mémoire d'invocation. La première version dépassait la limite du réseau
   (43 Mo pour 40 Mio) ; la version réduite passe en test, où nos contrats
   tournent en natif. La marge réelle, en wasm, se mesure en simulation.
2. Arbre d'autorisation du dépôt LP. Il est calé sur les wasm Aquarius
   vendorisés, où le pool tire les montants désirés puis rend l'excédent.
   Le pool déployé est plus récent : à vérifier en simulation avant tout
   dépôt réel.

## 16. Étape 7 : déploiement testnet (21/10)

**Essai interrompu sur l'EURC de Circle.** Le pool USDC/EURC a été aligné
puis amorcé au taux BCE à 09:21 UTC. À 09:23, un compte tiers
(`GCKR2OXW…`, une dizaine d'échanges par jour depuis le 14/10) a acheté
12,73 EURC contre 69 USDC et porté le prix du pool vers 26 USDC l'EURC. La
garde de glissement de la stratégie a refusé l'amorçage en simulation :
rien n'est parti. La liquidité a été retirée. Sur un pool que d'autres
arbitrent vers un prix sans rapport avec le marché, notre alignement serait
une subvention permanente.

**Paire retenue : USDC / FYEUR.** Le FYEUR est un actif classique émis par
une clé dédiée (`scripts/issue_test_eur.sh`), avec son contrat d'actif. Il
n'a aucun marché hors de notre pool, qui ne cote donc que notre prix. Le
contrat de stratégie ne change pas : la paire et le pool sont des
paramètres de son constructeur. Contrepartie : nous pouvons émettre du
FYEUR à volonté, il ne vaut un euro que par convention de test. Le journal
de preuves le dit.

**Bloquants de la section 15, levés.**

1. Mémoire d'invocation. Le RPC ne publie plus la mémoire consommée ; une
   simulation qui dépasse la limite échoue. Ont passé en simulation, en
   wasm, sur le pool déployé : dépôt et retrait de chaque stratégie, dépôt
   du vault. Instructions relevées : 21,8 M pour le dépôt Aquarius, 21,2 M
   pour son retrait, 4,0 M et 3,8 M pour Blend, 2,7 M pour le dépôt du
   vault. `rebalance` et le retrait du vault ne se simulent qu'une fois le
   vault financé.
2. Arbre d'autorisation du dépôt LP : le dépôt d'amorçage de la stratégie
   Aquarius est passé sur le pool déployé.

**Loyer payé au premier appel.** Chaque contrat prolonge son instance et
son code à 120 jours dès son premier appel (`TTL_EXTEND_TO`). Le testnet
facture ce loyer d'avance : 527 XLM pour la stratégie Aquarius (code de
48 Ko), environ 632 XLM estimés pour le vault (57 Ko), contre 0,004 XLM pour
le dépôt lui-même. Les appels suivants ne repaient rien avant 90 jours.
Coût unique, en XLM de test.


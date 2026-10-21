# D6a : schéma d'events de conformité, proposition

Proposition à relire avec Julien avant de figer le schéma (décision du
20/10/2026). Une fois validée, elle s'applique au vault avant toute ligne de
code D5 : les events de l'allocateur et du module de frais naissent au bon
format, sans rétrofit.

Livrable visé : « Soroban event schema for AMF-compliant audit trail ».
Measure : events structurés visibles sur Stellar Expert et consommés par le
dashboard conformité (D6c).

---

## 1. Ce que le texte impose

Source : MiCA, article 68(9) et (10), et le règlement délégué (UE) 2025/1140
du 27 février 2025 sur la tenue des registres des prestataires de services
sur crypto-actifs (PSCA), lu le 20/10/2026 dans sa version publiée au JO
L du 10/06/2025.

- **Art. 2(1), qualité du registre** : l'autorité doit pouvoir reconstituer
  chaque étape de chaque opération ; toute correction doit laisser voir le
  contenu antérieur ; le registre ne doit pas pouvoir être manipulé ; il doit
  être exploitable par un système informatique.
- **Art. 7, transactions** : chaque transaction est enregistrée immédiatement
  avec les champs de la table 3 (détail de la transaction) et de la table 4
  (données on-chain) de l'annexe.
- **Art. 8 et 10, décisions** : la personne ou l'algorithme qui prend la
  décision d'investissement est identifié (champ 41 de la table 3), de même
  que celui qui fixe les conditions d'exécution (champ 43). Notre allocateur
  D5 est un tel algorithme.
- **Art. 9 et 14, clients** : une personne physique est identifiée par un
  identifiant national, une personne morale par son LEI. Ce sont des données
  personnelles ou d'identification, pas des adresses.
- **Art. 12, exécution via une entité hors MiCA** : un ordre exécuté via une
  plateforme ou un prestataire auquel MiCA ne s'applique pas est enregistré
  comme si le PSCA l'avait exécuté lui-même. C'est le cas de Blend et
  d'Aquarius.
- **Annexe** : dates et heures en UTC.
- **Considérant 18** : minimisation, on ne conserve que ce qui est nécessaire.

La table 4 liste les données on-chain attendues : hash de transaction,
adresses de wallet, adresses de smart contract, horodatage du bloc, rapport
quantité transférée sur offre totale, identifiant DTI du jeton, frais réseau,
plafond de frais, taille des données, émetteur et destinataire.

## 2. Principe : la chaîne porte les faits, foryield-v2 porte les personnes

- **Aucune donnée personnelle on-chain.** Un event Soroban est public et
  définitif : y écrire un nom, une date de naissance ou un identifiant
  national contredirait le RGPD et le considérant 18. Les events portent des
  adresses ; la jointure adresse vers client se fait dans foryield-v2, comme
  elle se fait déjà pour le vault EVM (`VaultEvent` rapproché de
  `vault_requests.owner_address`).
- **Les données natives de la transaction ne se répètent pas.** Hash,
  horodatage du ledger, frais réseau, plafond de frais, taille et signataires
  (table 4, champs 1, 4, 7, 8, 9) se lisent sur la transaction elle-même, et
  `scripts/archive_proof.sh` les fige hors chaîne.
- **Chaque event porte ce que la transaction ne dit pas** : le sens métier.
  Quel compte, combien de parts, à quelle valeur liquidative, quelle
  décision, prise par qui, pour quelle raison.

## 3. Enveloppe commune

- **Format** : `#[contractevent]`, déjà employé par le routeur (`swap`,
  `aqua_pool_set`). Les deux events actuels du vault (`env.events().publish`)
  y migrent ; aucun code du dépôt ne les consomme aujourd'hui.
- **Topics** (filtrables sans décoder) : le nom fixe de l'event, puis
  l'acteur principal et, selon l'event, l'actif ou la stratégie. Quatre au
  plus.
- **`seq: u64`** dans la data de chaque event du vault : numéro
  d'enregistrement monotone, sans trou, propre au contrat. Il sert de numéro
  d'enregistrement (table 3, champ 2) et rend la complétude vérifiable : un
  trou dans la suite indexée signale un event manqué par l'indexeur. Coût :
  une écriture de compteur en storage par opération.
- **`total_assets` et `total_shares` après l'opération**, sur tout event qui
  modifie l'un ou l'autre. La valeur liquidative par part se reconstitue ainsi
  à chaque étape (prix, table 3 champ 31) sans relire l'état historique, ce
  que le RPC ne permet pas.
- **Montants** en `i128`, unités natives de l'actif (7 décimales). Aucune
  conversion en euros on-chain : la devise de valorisation (champs 30 et 32)
  relève de foryield-v2.
- **Évolution** : pas de numéro de version dans chaque event. Un changement
  incompatible change le nom (`deposit` devient `deposit_v2`), ce que tout
  décodeur détecte.

## 4. Events proposés

### Vault

| Event | Topics | Data | Livrable |
|---|---|---|---|
| `deposit` | owner | assets, shares, seq, total_assets, total_shares | migration D6a |
| `withdraw` | owner | shares, assets, seq, total_assets, total_shares | migration D6a |
| `allocate` | strategy | protocol, assets, position_after, decided_by, algo, reason, seq, total_assets | D5 |
| `deallocate` | strategy | protocol, assets, position_after, decided_by, algo, reason, seq, total_assets | D5 |
| `emergency_exit` | strategy | assets_recovered, write_off, decided_by, seq, total_assets | D5 |
| `fee_minted` | kind | fee_shares, fee_assets, recipient, rate_bps, hwm_before, hwm_after, period_start, period_end, seq, total_assets, total_shares | D6b |
| `paused` / `unpaused` | admin | seq | migration D6a |
| `nav` | (aucun) | total_assets, total_shares, seq | D6a, voir section 8 |
| `keeper_set` | (aucun) | old, new, admin, seq | D5 |
| `targets_set` | (aucun) | old, new, admin, seq | D5 |
| `config_set` | key | old, new, admin, seq | D6b |

Précisions :

- **`decided_by` et `algo`** répondent aux articles 8 et 10. `decided_by` est
  l'adresse qui a signé le déclenchement (admin ou keeper, ou le client
  pour un `deallocate` de raison `redeem`) ; `algo` est un
  code stable de l'algorithme et de sa version, par exemple `fy_alloc_v1`.
  Le champ 41 exige un code qui ne change pas tant que l'algorithme ne change
  pas.
- **`reason`** est un symbole d'une liste fermée : `drift` (écart à la cible),
  `cap` (plafond par protocole), `yield` (meilleur rendement), `manual`,
  `emergency`, `redeem` (retrait d'un client que la poche oisive ne couvre
  pas : décidé par le client, sans `algo`).
- **`protocol`** nomme le protocole sous-jacent (`blend`, `aquarius`), ce qui
  répond à l'article 12 : l'exécution via une entité hors MiCA est
  enregistrée avec l'identité de cette entité.
- **`fee_minted`** est émis une fois par type de frais (`perf`, `mgmt`), les
  deux dans la même transaction quand la répartition est atomique. Il reprend
  les champs que foryield-v2 attend déjà dans `fee_transactions` (section 5)
  et suit l'event EVM `PerformanceFeeSharesMinted(epoch, feeShares,
  feeAmount, nav)` de `ForYieldEpochVault`.
- **`config_set`** porte l'ancienne et la nouvelle valeur : une modification
  de paramètre laisse voir le contenu antérieur (art. 2(1)(b)). Les clés
  couvrent plafonds par protocole, taux de frais, trésorerie, stratégies
  ajoutées ou retirées, transfert d'admin. Un event Soroban a des champs de
  type fixe : si les valeurs ne partagent pas un type, on le scinde par
  famille plutôt que de passer par un type générique. D5 en fait deux
  events nommés : `keeper_set` (adresses) et `targets_set` (cibles en points
  de base, dans l'ordre de `strategies()`).

### Routeur

Le routeur ne change pas. Son event `swap` porte déjà acteur, instruments,
montants, frais et `min_out`. La venue exécutante (art. 12) se déduit du pool
enregistré pour la paire, dont l'historique complet est donné par les events
`aqua_pool_set`. Le modifier imposerait un redéploiement et la reprise des
preuves D4 pour un gain nul.

## 5. Correspondance avec les registres de foryield-v2

foryield-v2 tient déjà un registre MiCA des transactions
(`mica_registers.reg_transactions`) et une table des frais
(`fee_transactions`). Le schéma ci-dessus est conçu pour les alimenter sans
champ manquant côté chaîne.

| Colonne foryield-v2 | Source | Où |
|---|---|---|
| `reg_transactions.order_reference` | hash de transaction + `seq` | chaîne |
| `execution_at` | heure de clôture du ledger, UTC | transaction |
| `transaction_type` | nom de l'event | chaîne |
| `buyer_client_id` / `seller_client_id` | jointure sur l'adresse `owner` | foryield-v2 |
| `crypto_asset_identifier` | adresse de l'actif, puis DTI | chaîne puis foryield-v2 |
| `quantity` | `assets` ou `shares` | chaîne |
| `unit_price` | `total_assets / total_shares` | chaîne |
| `total_amount_cents`, `settlement_currency` | conversion en euros | foryield-v2 |
| `fees_cents` | `fee_assets` ou `fee` du swap, converti | chaîne puis foryield-v2 |
| `blockchain_tx_hash`, `blockchain_network` | transaction, réseau | transaction |
| `sender_address` / `recipient_address` | `owner`, adresse du vault | chaîne |
| `execution_platform` | adresse du vault, `protocol`, pool Aquarius | chaîne |
| `decision_by` | vide pour une opération du client (section 8, réponse 3), sinon `decided_by` + `algo` | chaîne |
| `fee_transactions.hwm_before` / `hwm_after` | event `fee_minted` | chaîne |
| `nav_at_calculation`, `aum_at_calculation` | `total_assets`, `total_shares` | chaîne |
| `period_start` / `period_end`, `fee_type` | event `fee_minted` | chaîne |

## 6. Hors du périmètre de D6a

- **Identité des clients** (table 3, champs 6 à 23 : noms, dates de
  naissance, identifiants nationaux, LEI, pays de la succursale) : KYC de
  foryield-v2.
- **Registres de la section 1 de l'annexe** (communications commerciales,
  réclamations, conventions, externalisation) : foryield-v2.
- **Conservation** : MiCA impose 5 ans (7 sur demande de l'autorité), alors
  que le RPC Stellar ne garde les events qu'environ 7 jours (fenêtre par
  défaut de 120 960 ledgers). Le dashboard D6c doit donc ingérer les events
  et les stocker lui-même. La structure de `VaultEvent` dans foryield-v2
  (hash, index, data brute, horodatage, unicité par transaction et index)
  est le bon modèle, et `seq` permet d'y détecter les trous.

## 7. Questions pour Julien

1. Quel service MiCA le vault rend-il : gestion de portefeuille (art. 81) ou
   autre ? La réponse fixe la « capacité de négociation » (champ 28) et
   l'état périodique à fournir au client.
2. Le code d'algorithme `fy_alloc_v1` convient-il comme identifiant stable
   du champ 41 ?
3. Pour un dépôt ou un retrait, le décideur est le client lui-même (`owner`).
   Est-ce la lecture de la conformité ?
4. Faut-il un event périodique de valeur liquidative, même sans opération
   (par exemple quotidien, déclenché par un keeper) ? Sans lui, la valeur
   liquidative n'est connue qu'aux dates d'opération.
5. Le schéma doit-il couvrir dès D6a les instances déjà déployées, ou
   seulement les instances redéployées avec D5 ? Je propose la seconde
   option : les preuves D1 à D4 restent sur le code qui les a produites.

## 8. Réponses de Julien (21/10/2026)

**Statut du vault : exploratoire.** Un vault à parts mutualisées valorisées à
un prix commun relève de la gestion collective (orientations ESMA de mars
2025, §37 et 41), hors de l'agrément de PSCA. Ce vault ne sera donc jamais mis
en production réelle. On va au bout du développement pour le grant SCF : le
schéma démontre le registre qu'exigerait un service MiCA, sans ajouter ce qui
ne servirait qu'en production.

1. **Service MiCA** : le schéma est construit comme pour de la gestion de
   portefeuille (art. 81). Champ 28 : le PSCA agit pour le compte du client,
   valeur « autre capacité » (AOTC), à reprendre telle quelle de la liste du
   règlement délégué 2025/1140 au moment de coder D6c. État périodique : le
   relevé mensuel des mandats ForYield, reconstitué par D6c à partir des
   events `nav`. Aucune requalification à prévoir, puisque rien ne part en
   production.
2. **`fy_alloc_v1`** : retenu, avec trois règles. Le code passe à
   `fy_alloc_v2` quand la logique de décision change, jamais pour un
   paramètre (déjà tracé par `config_set`). D6c tient la table code
   d'algorithme vers hash du wasm, date de mise en service et description.
   Une décision `reason = manual` n'a pas de code d'algorithme : le champ 41
   désigne alors la personne, retrouvée à partir de `decided_by`. Le code
   s'écrit avec `Symbol::new` (`symbol_short!` est limité à 9 caractères).
3. **Décideur d'un dépôt ou d'un retrait** : le client (`owner`), qui signe
   lui-même (`from.require_auth()`). Le champ 41 reste vide, le PSCA ne
   décide rien. Pas de retrait en deux temps : la validation réglementaire
   des retraits prévue par les mandats relève du produit réel, hors
   périmètre du grant.
4. **Valeur liquidative périodique** : oui, quotidienne. Une fonction
   `checkpoint()` sans authentification, refusée si le dernier `nav` a moins
   d'un jour (comparaison des horodatages de ledger), émet `nav`
   (`total_assets`, `total_shares`, `seq`). Une tâche planifiée quotidienne
   (GitHub Actions) l'appelle sur les instances testnet ; si elle manque un
   jour, n'importe qui peut rattraper. D6c trace la valeur liquidative à
   partir de ces events.
5. **Instances concernées** : la proposition est retenue. Le schéma
   s'applique aux instances redéployées avec D5 ; les instances D1 à D4
   restent sur leur bytecode, pour que leurs preuves restent rejouables. Le
   README les marque « avant schéma » et D6c ne lit que les nouvelles.

## 9. Ajustements à l'implémentation (21/10/2026)

Arrêtés par Pierrick en codant D5 (commit `f2e3a79` de
`feat/d5-vault-strategies`) :

1. **`nav` une fois par jour UTC.** `checkpoint()` est accepté dès que le
   jour UTC de l'horodatage du ledger dépasse celui du dernier `nav`, et non
   24 h après lui. Une tâche planifiée GitHub Actions part avec un retard
   variable : avec un délai glissant, un appel à 0 h 47 suivi d'un appel le
   lendemain à 0 h 12 serait refusé et le jour sauterait. Un second appel le
   même jour est refusé (`NavAlreadyPublished`). `checkpoint()` fonctionne en
   pause.
2. **Retrait servi par une stratégie.** Quand la poche oisive ne couvre pas
   un retrait, chaque stratégie sollicitée émet un `deallocate` de raison
   `redeem`, `decided_by` = le client, `algo` vide, avant l'event
   `withdraw`. L'exécution via Blend ou Aquarius (art. 12) reste ainsi
   tracée à chaque étape.
3. **`config_set` scindé** en `keeper_set` et `targets_set` pour D5 ;
   `config_set` reste réservé aux paramètres de D6b.
4. **`emergency_exit` garde `write_off`** : le passage en perte est une
   décision de l'admin et doit se lire comme telle.
5. **`protocol` fixé au déploiement**, dans la configuration de chaque
   stratégie : le vault ne peut pas le lire sur la stratégie.
6. **`total_assets` et `total_shares`** portent les valeurs à la fin de
   l'appel, identiques sur tous les events d'un même appel (un `rebalance`
   ou un retrait en émet plusieurs).

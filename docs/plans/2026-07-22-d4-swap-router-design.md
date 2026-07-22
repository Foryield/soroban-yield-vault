# D4 — Routeur de swap Soroswap/Aquarius : design validé

Date : 22 juillet 2026. Statut : validé par Pierrick (session de cadrage).
Référence roadmap : [2026-07-21-rattrapage-scf.md](2026-07-21-rattrapage-scf.md),
jalon D4 du 4 septembre. Spikes fondateurs :
[2026-07-21-spikes-s1.md](2026-07-21-spikes-s1.md).

## Décision d'architecture

**Contrat routeur séparé** (`contracts/router`), le vault D1 reste intact.
Trois options examinées :

1. **Routeur séparé** (retenue) : brique d'exécution indépendante, démo de
   rebalance orchestrée entre les deux instances de vault (USDC → swap → EURC).
2. Module swap dans le vault : rejetée, casse le modèle mono-actif et impose
   une source de prix on-chain (spot manipulable ou oracle) pour `total_assets`.
3. Vault multi-actif : rejetée, réécrit la math de parts validée en D1
   (231 tests, 92,4 %) hors du gabarit de 2 semaines.

Justification par la cible d'intégration foryield-v2 : la plateforme orchestre
déjà ses swaps EVM côté backend (`Exchanges::Aggregators`, sélection auditée,
fallback sous CircuitBreaker, journalisation conformité) et tient la valorisation
côté Rails (cents EUR, validation gérant), jamais par oracle on-chain. Le routeur
Soroban est le miroir de ce modèle : la cotation et la sélection de venue restent
off-chain (script de démo aujourd'hui, backend Rails demain via la couture
`onboarding/`), le contrat garantit ce que seul l'on-chain garantit : min-out et
fallback atomique dans la même transaction.

## Interface

Nouveau membre de workspace `contracts/router`, contrat `SwapRouter` :

```rust
initialize(admin: Address, soroswap_aggregator: Address, aquarius_router: Address,
           soroswap_fee_bps: u32, aquarius_fee_bps: u32)

swap_exact_in(from: Address, token_in: Address, token_out: Address,
              amount_in: i128, min_out: i128, preferred: Venue) -> SwapResult

pair_stats(token_in: Address, token_out: Address) -> PairStats

enum Venue { SoroswapAggregator, AquariusRouter }
struct SwapResult { amount_out: i128, venue: Venue, fee: i128 }
struct PairStats { volume_in: i128, volume_out: i128, fees: i128, swaps: u64 }
```

- Adresses de venues fixées à l'`initialize`, relues des registres canoniques
  au déploiement, jamais en dur (règle spikes S1). Pas de setter en D4 :
  changement de venue = redéploiement (immuabilité assumée, documentée en tête
  de contrat, même convention que le pool du vault D1).
- `preferred` matérialise la sélection off-chain (best-execution : le client
  cote les deux venues et passe son choix).
- `min_out` obligatoire et strictement positif : unique paramètre de slippage,
  solde la décision reportée de D1. Le vault reste sans paramètre de slippage
  (opérations Blend à taux connu, sans prix de marché).
- Swap direct USDC↔EURC uniquement, pas de multi-hop en D4 (pas de `path`
  exposé). Aggregator Soroswap appelé avec une distribution 100 % Soroswap.
- Erreurs typées `#[contracterror]` dès le premier commit.

## Exécution

Déroulé de `swap_exact_in` :

1. `from.require_auth()`, gardes (`amount_in > 0`, `min_out > 0`,
   `token_in != token_out`).
2. Le routeur tire `amount_in` depuis `from` (couvert par l'auth de `from`).
3. Venue `preferred` via `try_swap…` ; sur échec (revert, liquidité, min-out
   non servi), bascule sur l'autre venue dans la même transaction. Fallback
   atomique : soit une venue sert au moins `min_out`, soit tout revert et
   `from` garde ses fonds.
4. `min_out` propagé aux venues ET revérifié par le routeur au retour
   (défense en profondeur vis-à-vis du code tiers).
5. Produit intégralement reversé à `from`. Invariant : solde du routeur nul
   hors transaction (testé en propriété).

Pré-autorisations : `authorize_as_current_contract` ciblé (fonction, args,
montant exacts) pour les `transfer` imbriqués des venues, même convention que
`pool_supply` du vault. Jamais d'approbation ouverte.

Pont d'interfaces : Soroswap parle i128, Aquarius u128. Module interne
`venues`, un sous-module par venue, conversions vérifiées (négatif = erreur
typée `AmountConversion`) ; le cœur du routeur ne connaît que i128.

Échec des deux venues : erreur typée `AllVenuesFailed`, distincte des erreurs
de garde (le client distingue slippage et panne de venue).

## Frais et events

Les venues incorporent leur commission au prix (0,3 % Soroswap style Uniswap
V2, frais de pool variables Aquarius). Comptabilité :
`fee = amount_in × fee_bps_venue / 10_000`, `fee_bps` fixés par venue à
l'`initialize`. Accumulateurs persistants par paire ordonnée
(volume entrant, volume sortant, frais, nombre de swaps), lecture publique
`pair_stats`. Matière première du dashboard D6c, sans indexeur.

Events en `#[contractevent]` (style cible D6a) dès le premier commit, le
routeur étant un contrat neuf sans format hérité :

- `swap` : from, token_in, token_out, amount_in, amount_out, venue
  **effective** (celle qui a servi après fallback, même exigence que le
  `with_fallback` de foryield-v2), fee, min_out.
- Pas d'event d'échec : l'échec des deux venues revert tout, events compris ;
  il s'observe par le statut de la transaction.

Convention D6a minimale embarquée : acteur, instruments, montants, décision
d'exécution. Champs transverses éventuels (séquence, version de schéma)
adoptés pendant le chantier D6a. Le vault garde ses events dépréciés jusqu'à
la migration D6a planifiée : on ne mélange pas les chantiers.

## Tests

- **Unitaires, venues mockées** : deux contrats de test contrôlables (rendre X,
  rendre moins que min-out, paniquer). Matrice fallback (préférée sert /
  préférée échoue et secours sert / les deux échouent), gardes, conversions
  i128↔u128 aux bornes, invariant solde nul en proptest.
- **Intégration stack réelle** : wasm Soroswap (pair + router + aggregator)
  importés en fixture, comme BlendFixture en D1. Spike court en ouverture de
  chantier ; si les wasm Aquarius ne s'importent pas proprement, venue couverte
  par mocks + preuve testnet, décision documentée.
- CI existante (cargo test + couverture 90 %, 3 checks requis) appliquée au
  nouveau crate sans changement.

## Évidences testnet

Dans `docs/evidence/d4-dex-routing.md`, complétées le jour même :

- Seed d'un pool Aquarius USDC-Blend/EURC-Circle, pendant du pool Soroswap
  déjà semé (tx `5fa01cea…`) : script rejouable `scripts/seed_aquarius_pool.sh`,
  adresses relues aux sources canoniques.
- Démo de rebalance : cotation des deux venues consignée (best-execution),
  puis chaîne complète en hashes : retrait vault USDC → `swap_exact_in` →
  dépôt vault EURC.
- Preuve de fallback dédiée : tx avec `preferred = Aquarius` calibrée pour que
  sa liquidité ne serve pas `min_out`, bascule Soroswap dans la même
  transaction, venue effective visible dans l'event `swap`.
- Vidéo walkthrough en clôture.

## Séquencement

PR A : contrat routeur + tests mockés. PR B : fixtures d'intégration.
PR C : seed Aquarius + déploiement + évidences. Chaque PR mergée sur CI verte,
revue par subagent à contexte frais (convention D2).

## Amendement du 22/07 (vérification des sources avant plan)

Faits établis en lisant les sources (`soroswap/aggregator`, dont l'adapter
Aqua de référence) avant l'écriture du plan d'implémentation :

- `DexDistribution` a un 4e champ `bytes: Option<Vec<BytesN<32>>>` (hashes
  de pool Aqua) en plus de `protocol_id`, `path`, `parts`.
- Aquarius identifie ses pools par un `pool_hash` passé à `swap_chained`
  (chaque maillon = paire triée par adresse, pool_hash, token_out). Le
  routeur gagne donc un registre admin `set_aqua_pool(token_a, token_b,
  pool_hash)` (clé = paire triée) ; sans entrée, la venue Aquarius échoue
  en erreur typée `AquaPoolNotSet` et le fallback la traverse. Justification
  d'un setter admin plutôt que l'initialize : le hash change à chaque
  re-seed après reset testnet, un redéploiement du routeur serait
  disproportionné pour un identifiant de pool.
- Le repo canonique `AquaToken/soroban-amm` n'est plus accessible (404).
  Références de substitution : l'adapter Aqua des sources Soroswap et les
  wasm Aqua vendorisés dans `soroswap/aggregator` (commit à épingler,
  provenance et hashes consignés au vendoring).

Plan d'exécution : [2026-07-22-d4-swap-router-implementation.md](2026-07-22-d4-swap-router-implementation.md).

## Amendement du 22/07 (implémentation, tâche 6)

L'architecture `attempt -> bool` retenue à l'implémentation rend deux erreurs
typées du design sans surface d'émission ; elles sont RETIRÉES de l'enum
(codes figés avec trous : 1-4, 6, 7, 9, 10) :

- `AquaPoolNotSet` : « pool non configuré » est un état statique, observable
  avant l'appel via le getter public `aqua_pool_of(token_a, token_b) ->
  Option<pool_hash>` ; au swap, la venue est traversée par le fallback et le
  client ne distingue que slippage (`SlippageExceeded`) et panne de venue
  (`AllVenuesFailed`), distinction suffisante opérationnellement.
- `AmountConversion` : côté entrée la positivité est garantie par les gardes
  de `swap_exact_in` avant toute venue ; côté retour le succès est jugé sur
  delta de solde et un retour inconvertible dégrade en fallback puis revert
  intégral (témoin de test dédié).

Limitation assumée du registre Aqua : pas de suppression d'entrée (le flux
nominal de re-seed après reset écrase avec le hash frais) ; un hash périmé
fait échouer la venue puis traverser le fallback, sans risque de fonds.

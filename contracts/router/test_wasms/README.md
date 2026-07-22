# Wasm de venues vendorisés (tests d'intégration du routeur)

Binaires wasm des venues Soroswap et Aqua, utilisés par les tests
d'intégration du routeur via `contractimport!` (fixtures « stack réelle »,
tasks 10 et 11 du plan D4). Récupérés par `scripts/fetch_test_wasms.sh`,
qui re-télécharge chaque fichier depuis la source épinglée puis vérifie
son empreinte contre `SHA256SUMS`.

## Provenance

- Dépôt source : [`soroswap/aggregator`](https://github.com/soroswap/aggregator),
  répertoire `contracts/aggregator/`
- Commit épinglé : `84de10e0f8d26168b4a76f8c23b963e50917517c`
  (HEAD de `main` au moment du vendoring, commit du 2025-12-22)
- Date de récupération : 2026-07-22

| Fichier | Sous-répertoire source | SHA-256 |
| --- | --- | --- |
| `soroswap_factory.wasm` | `soroswap_contracts` | `b8f7c4289f9f8c187efa57a5cc4598b1fa773f83d16f0e2044c1936b4ab02bfd` |
| `soroswap_pair.wasm` | `soroswap_contracts` | `f25a763b8166ccded22c30eb9aef0dc1e12ec5d7190e2febac6e20a4840b79b8` |
| `soroswap_router.wasm` | `soroswap_contracts` | `5f86422399da20b8601a7201f765afc7e4b2bbbccd0520851a8af638d3a6415a` |
| `soroban_liquidity_pool_router_contract.wasm` | `aqua_contracts` | `04b594a5f9c7ed5291e10dc019ba0845866ca701a3d05fef206a7e9eef302d76` |
| `soroban_liquidity_pool_contract.wasm` | `aqua_contracts` | `549376178582fc695a358d5e333dc568609a5e23460f01002c23ba7cd2863ead` |
| `soroban_liquidity_pool_plane_contract.wasm` | `aqua_contracts` | `3a35e48573a4aa300de8e417c8e3b01e30123c49ce67e7d67e8752d1850ac729` |
| `soroban_liquidity_pool_liquidity_calculator_contract.wasm` | `aqua_contracts` | `75161be17f8f028638b91095bbd8827a1a11e3684e11f0bc431663a5f1e75b52` |
| `soroban_token_contract.wasm` | `aqua_contracts` | `596ace8b855436478512821a2e0ecb02973b1bad0a4057dc541fd0ca4d7cf037` |

## Motivation

Le dépôt canonique d'Aqua (`AquaToken/soroban-amm`) répond en 404, constat
antérieur au 2026-07-22. Les binaires embarqués dans `soroswap/aggregator`
sont la référence vivante des deux venues : ce sont ceux contre lesquels
l'agrégateur Soroswap teste ses propres adapters. Le commit épinglé est
celui dont les sources des adapters ont été vérifiées le 2026-07-22.

## Sources miroir (sémantique Aqua)

Le dépôt canonique d'Aqua (`AquaToken/soroban-amm`) étant en 404, la
sémantique des wasm Aqua vendorisés a été établie sur un miroir des
sources :

- Dépôt : [`calc1f4r/soroban-amm`](https://github.com/calc1f4r/soroban-amm)
- Commit : `f9d4a5e0`
- Génération : la même que les wasm vendorisés (rssdkver 22.0.6 dans la
  méta des wasm)
- Date de vérification : 2026-07-22

Ce miroir est ce qui a établi :

- la sémantique de `get_amount_out` : fee prélevée sur la sortie, arrondi
  plafond (base de la dérivation de `EXPECTED_OUT_AQUA` dans
  `src/test_aqua_stack.rs`) ;
- les étapes obligatoires de la chaîne d'init du router Aqua
  (`init_standard_pool` lit token hash, reward token, boost config, plane
  et config de paiement sans garde : `StorageError` 501 si une étape
  manque) ;
- la topologie d'auth de `swap_chained` : `user.require_auth()` dans sa
  frame puis escrow `transfer(user -> router Aqua)` (le `user` de
  `swap_chained` étant l'appelant direct, notre routeur).

Une copie durable du miroir (fork sous l'org Foryield) est recommandée,
décision en attente.

## Wasm de l'agrégateur : construit localement (hors SHA256SUMS)

Aucun wasm précompilé du contrat agrégateur lui-même n'existe dans le
dépôt `soroswap/aggregator` au commit épinglé (seuls les wasm de venues
sont publiés, sous `contracts/aggregator/` et `contracts/adapters/`).
`soroswap_aggregator.wasm` a donc été **construit depuis les sources** au
même commit épinglé (`84de10e0f8d26168b4a76f8c23b963e50917517c`), le
2026-07-22 :

```sh
git clone https://github.com/soroswap/aggregator && cd aggregator
git checkout 84de10e0f8d26168b4a76f8c23b963e50917517c
cd contracts/aggregator
cargo build --target wasm32-unknown-unknown --release
# artefact : contracts/target/wasm32-unknown-unknown/release/soroswap_aggregator.wasm
```

- Chaîne de compilation : rustc 1.94.0 (4a4ef493e 2026-03-02), cible
  `wasm32-unknown-unknown`, profil release du workspace source
  (soroban-sdk 22.0.7). L'étape `soroban contract optimize` du Makefile
  amont est omise : sans effet sur la sémantique, inutile pour un wasm de
  test.
- SHA-256 : `4ee0fddf79d695d48e694413d8eee7ba592d38b626d94c8b4e3c54f725eb2f40`
- Le build est reproductible à l'octet avec cette chaîne (vérifié par deux
  builds depuis `cargo clean`), mais l'empreinte dépend de la version de
  rustc : ce n'est **pas un téléchargement canonique**, le fichier reste
  donc **hors de `SHA256SUMS`** (manifeste réservé aux binaires
  re-téléchargeables par `scripts/fetch_test_wasms.sh`) et hors du script.
  Pour le régénérer, rejouer la procédure ci-dessus et confronter
  l'empreinte à celle consignée ici.

## Re-épinglage

Pour mettre à jour les binaires sur un nouveau commit source, la procédure
est délibérée et se fait en une seule passe :

1. Changer `PINNED_SHA` dans `scripts/fetch_test_wasms.sh`.
2. Exécuter le script : la vérification échoue sur les nouveaux binaires,
   c'est attendu.
3. Régénérer les checksums depuis ce répertoire :
   `shasum -a 256 *.wasm > SHA256SUMS` (ou `sha256sum`).
4. Mettre à jour le README : la table de provenance doit être régénérée
   en même temps que `SHA256SUMS` (les deux listent les mêmes empreintes),
   ainsi que le commit épinglé et les deux dates (commit source, date de
   récupération).

Un échec de checksum en dehors d'un re-épinglage délibéré doit être
traité comme un signal de compromission (binaire altéré à la source ou en
transit), jamais résolu en régénérant `SHA256SUMS` par-dessus.

## Usage

```sh
scripts/fetch_test_wasms.sh
```

Le script est rejouable : il écrase les fichiers existants puis vérifie
`SHA256SUMS` avec `shasum -a 256 -c`. Toute divergence fait échouer
l'exécution. Les tests d'intégration consomment ces binaires avec
`soroban_sdk::contractimport!(file = "test_wasms/<fichier>.wasm")`.

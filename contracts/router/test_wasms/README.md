# Wasm de venue vendorisés (tests d'intégration du routeur)

Binaires wasm de la venue Aquarius, utilisés par les tests d'intégration du
routeur via `contractimport!` (fixture « stack réelle »). Leur intégrité est
vérifiée contre `SHA256SUMS` à chaque exécution des tests
(`vendored_wasms_match_sha256sums`, `src/test_stack_common.rs`) et par
`scripts/fetch_test_wasms.sh`.

## Source amont coupée le 2026-08-28

Ces binaires provenaient du dépôt
[`soroswap/aggregator`](https://github.com/soroswap/aggregator). Suite à la
compromission de Soroswap rapportée le 2026-08-28 (confirmée par le
développeur smart contract de l'équipe et par The Arch, aucune source
publique trouvée), ce dépôt sort du projet : la venue Soroswap a été retirée
du contrat, ses quatre wasm supprimés, et
`scripts/fetch_test_wasms.sh` ne télécharge plus rien.

Les cinq wasm Aquarius sont **conservés**. Ils sont épinglés à un commit de
juillet 2026, antérieur à l'incident rapporté, et leurs empreintes SHA-256
sont vérifiées à chaque exécution des tests : une altération rétroactive du
dépôt amont ne pourrait pas passer inaperçue.

**Risque résiduel assumé** : si la compromission était antérieure au commit
épinglé et avait touché les wasm Aqua eux-mêmes, nos fixtures de test le
refléteraient. Rien ne l'indique, et ces binaires ne servent qu'aux tests,
jamais au déploiement. À rouvrir si un dépôt canonique Aquarius redevient
disponible (voir « Si une source Aquarius redevient disponible » plus bas).

## Provenance

- Dépôt source : `soroswap/aggregator`, répertoire
  `contracts/aggregator/aqua_contracts/`
- Commit épinglé : `84de10e0f8d26168b4a76f8c23b963e50917517c`
  (HEAD de `main` au moment du vendoring, commit du 2025-12-22)
- Date de récupération : 2026-07-22
- Date de coupure de la source : 2026-08-28

| Fichier | SHA-256 |
| --- | --- |
| `soroban_liquidity_pool_router_contract.wasm` | `04b594a5f9c7ed5291e10dc019ba0845866ca701a3d05fef206a7e9eef302d76` |
| `soroban_liquidity_pool_contract.wasm` | `549376178582fc695a358d5e333dc568609a5e23460f01002c23ba7cd2863ead` |
| `soroban_liquidity_pool_plane_contract.wasm` | `3a35e48573a4aa300de8e417c8e3b01e30123c49ce67e7d67e8752d1850ac729` |
| `soroban_liquidity_pool_liquidity_calculator_contract.wasm` | `75161be17f8f028638b91095bbd8827a1a11e3684e11f0bc431663a5f1e75b52` |
| `soroban_token_contract.wasm` | `596ace8b855436478512821a2e0ecb02973b1bad0a4057dc541fd0ca4d7cf037` |

## Motivation du vendoring

Le dépôt canonique d'Aqua (`AquaToken/soroban-amm`) répond en 404, constat
antérieur au 2026-07-22. Les binaires embarqués dans `soroswap/aggregator`
étaient la seule référence publique de la venue Aquarius : ce sont ceux
contre lesquels l'agrégateur testait ses propres adapters. Cette absence de
source canonique est précisément ce qui rend la coupure de la source amont
sans conséquence pratique : il n'y avait déjà nulle part où re-télécharger
autrement.

## Sources miroir (sémantique Aqua)

Le dépôt canonique d'Aqua étant en 404, la sémantique des wasm Aqua
vendorisés a été établie sur un miroir des sources :

- Dépôt de référence :
  [`Foryield/soroban-amm`](https://github.com/Foryield/soroban-amm)
  (copie durable sous notre contrôle, forkée le 2026-07-22 depuis
  [`calc1f4r/soroban-amm`](https://github.com/calc1f4r/soroban-amm),
  crédité comme origine)
- Commit : `f9d4a5e0589e785dd8d44959f100545dd0d0f17c` (daté 2025-05-23,
  vérifié accessible dans le fork)
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

Ce fork est sous notre contrôle et n'est pas concerné par la compromission
de Soroswap. Il reste la référence sémantique de la venue.

## Si une source Aquarius redevient disponible

Le re-épinglage sur `soroswap/aggregator` n'a plus cours. Si un dépôt
canonique Aquarius réapparaît, ou si une source de confiance publie ces
binaires :

1. Comparer les empreintes de la nouvelle source à celles du tableau
   ci-dessus. Si elles coïncident, la provenance est confirmée par deux
   sources indépendantes : le noter ici, rien d'autre à faire.
2. Si elles diffèrent, ne PAS régénérer `SHA256SUMS` par-dessus. Établir
   d'abord laquelle des deux générations correspond au router Aquarius
   réellement déployé sur le réseau, en confrontant les spec embarqués
   (`stellar contract info interface --wasm ...`) et le comportement
   observé en simulation sur le contrat déployé.
3. Après décision seulement, remplacer les binaires, régénérer les
   checksums depuis ce répertoire
   (`ls *.wasm | xargs shasum -a 256 > SHA256SUMS`) et mettre à jour la
   table de provenance ci-dessus en même temps.

Un échec de checksum en dehors d'un remplacement délibéré doit être traité
comme un signal de compromission (binaire altéré), jamais résolu en
régénérant `SHA256SUMS` par-dessus.

## Usage

```sh
scripts/fetch_test_wasms.sh
```

Le script ne télécharge plus : il vérifie l'intégrité des fichiers présents
contre `SHA256SUMS`. La même vérification court à chaque exécution des tests.
Les tests d'intégration consomment ces binaires avec
`soroban_sdk::contractimport!(file = "test_wasms/<fichier>.wasm")`.

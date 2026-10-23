# D6c : dashboard de conformité et page produit D5, plan

Date : 23 octobre 2026. Statut : proposition, à valider par Pierrick avant la
première ligne de code. Décision du 23/10 : une première version complète
(fonctions et données) codée maintenant, reprise ensuite par Marius pour le
design, puis la vidéo D5 tournée par un testeur depuis le navigateur.

Références : schéma D6a
([2026-10-20-d6a-schema-events-conformite.md](2026-10-20-d6a-schema-events-conformite.md)),
plan D5 ([2026-10-20-d5-allocateur-conception.md](2026-10-20-d5-allocateur-conception.md)),
fiche Milestone 2 du 20/10 (D6c : visualiseur standalone dans `web/`).

---

## 1. Ce que les deux livrables demandent

- **D5** : une adresse de vault dont la répartition entre stratégies se lit
  sur la chaîne, le hash de chaque allocation, une vidéo.
- **D6c** : des events structurés visibles sur Stellar Expert et consommés
  par le dashboard de conformité, une capture.
- **D6a §6** : le RPC ne garde les events qu'environ 7 jours, quand MiCA
  impose 5 ans. Le dashboard ingère donc les events et les stocke lui-même,
  avec `seq` pour détecter les trous.
- **Réponses de Julien (D6a §8)** : relevé périodique reconstitué à partir
  des events `nav` ; table code d'algorithme vers hash du wasm, date de mise
  en service et description ; champ 28 en « AOTC ».

## 2. Ce que le testeur filme

Tout se passe dans le navigateur, avec Freighter :

1. La page produit, onglet USDC : il connecte son wallet, ouvre la trustline
   USDC si besoin, dépose 1 USDC. Le panneau de répartition montre le dépôt
   en attente dans la poche oisive.
2. Hors champ, l'ops lance le rééquilibrage (clé keeper). Le panneau passe à
   70 / 20 / 10 ; le dashboard affiche les deux `allocate` du même lot, avec
   leur hash.
3. Dépôt de 0,05 USDC : l'indicateur de déclenchement reste sous le seuil
   (1,5 point pour 5). L'ops tente le rééquilibrage, refusé à la simulation,
   sans transaction.
4. Dépôt de 0,2 USDC : l'indicateur passe au-dessus du seuil (6,9 points),
   le rééquilibrage passe.
5. Retrait de 0,15 USDC, servi par l'oisif ; retrait de 1 USDC, complété par
   Blend (`deallocate` de raison `redeem`, décidé par le client).
6. Le dashboard : piste d'audit complète et sans trou, valeur liquidative,
   historique des allocations, export.

Le rééquilibrage n'est pas un geste de déposant : seul le keeper l'appelle.
Le montrer déclenché hors champ est fidèle au produit.

## 3. Stockage des events : deux approches

**(A) Archive dans le dépôt, alimentée par une tâche GitHub Actions.** Une
tâche quotidienne lit les nouveaux events au RPC (`getEvents`, à partir du
dernier curseur), les ajoute à un fichier JSON par contrat sur une branche
`data/events`, et commite. Le dashboard lit cette archive (le dépôt est
public, `raw.githubusercontent.com` répond aux requêtes du navigateur) puis
complète en direct au RPC à partir du dernier ledger archivé : un event de la
minute s'affiche sans attendre la tâche.

- Pour : aucun serveur, aucun coût, le site reste statique. L'historique git
  de la branche est un registre en ajout seul, où toute correction laisse
  voir le contenu antérieur (art. 2(1)(b)). Rien ne passe par `main`, ni par
  ses vérifications de CI.
- Contre : la fraîcheur de l'archive dépend de la tâche (une fois par jour,
  rattrapable à la main) ; le dashboard dépend de GitHub pour l'historique.

**(B) Service d'indexation avec base de données** (Render web service et
Postgres), sur le modèle de `VaultEvent` de foryield-v2.

- Pour : ingestion continue, requêtes SQL, modèle déjà connu.
- Contre : un service toujours actif et une base persistante sont payants sur
  Render (la base gratuite expire au bout de 30 jours), ce qui contredit la
  règle « aucune dépendance payante » du 20/10. Un serveur de plus à
  maintenir pour un vault qui ne partira pas en production (D6a §8).

**Choix : (A).** Elle respecte la règle de coût et le déploiement statique
(`render.yaml`), et donne le registre en ajout seul que demande l'article
2(1). Le modèle de `VaultEvent` est repris dans le format de l'archive :
hash, index, ledger, horodatage, topics et data bruts en XDR, plus leur
décodage.

**Échéance** : les events du déploiement D5 (21/10) sortent de la fenêtre du
RPC vers le 28/10. La première ingestion doit tourner avant ; sinon, ils se
reconstituent depuis `docs/evidence/archive/d5/*/decoded.json`.

## 4. Tâche quotidienne d'ops

La même tâche GitHub Actions fait trois choses, dans cet ordre :

1. `checkpoint()` sur le vault D5 : event `nav` du jour (D6a §8, réponse 4).
2. Publication du prix de référence (`scripts/push_reference_price.sh`) : le
   prix ne périme plus tous les trois jours.
3. Ingestion des events vers `data/events`.

Elle signe avec une clé dédiée, `fy-ops-bot`, nommée keeper du vault et de la
stratégie Aquarius (`set_keeper`, par l'admin), stockée en secret GitHub. Elle
n'a aucun pouvoir d'admin : au pire, elle rééquilibre ou publie un prix borné
à 1 % par heure. `d1-ops` reste admin et ne sort pas du Mac de Pierrick.

Le rééquilibrage reste manuel dans cette version : pendant le tournage, l'ops
le déclenche au moment voulu.

## 5. Page produit (`/`)

- Onglet **USDC**, l'instance D5 (`CBESWJ4C…XVOV`), à côté de XLM et EURC.
  Dépôt, retrait, trustline : le code existant, inchangé (même signature
  `deposit(from, amount)` et `withdraw(from, shares)`).
- **Panneau de répartition**, pour toute instance qui a des stratégies : une
  ligne par poche (Blend, Aquarius, oisif), montant, part, cible, écart en
  points, lus sur la chaîne (`allocation`, `total_assets`, `strategies`).
- **Indicateur de déclenchement** : écart maximal comparé au seuil
  (`drift_bps`), avec la même règle que le contrat (`lib.rs`, rebalance) :
  « rééquilibrage possible » ou « sous le seuil ».
- Lien vers le dashboard de l'instance.

## 6. Dashboard (`/dashboard`)

Une page de la même application (export statique), en anglais comme la page
produit : elle est jointe au dossier de relecture SCF.

1. **Vue d'ensemble** : actif total, parts, valeur liquidative par part,
   état (pause), keeper, répartition courante et indicateur de
   déclenchement.
2. **Piste d'audit** : tous les events du vault dans l'ordre de `seq`, avec
   heure UTC, type, acteur, montants, `decided_by`, `algo`, `reason`,
   `protocol`, lien Stellar Expert. Filtres par type. **Contrôle de
   complétude** : trou dans la suite des `seq` signalé en tête.
3. **Allocations** : chaque `allocate` et `deallocate`, groupés par
   transaction, avec la position de chaque stratégie après coup.
4. **Valeur liquidative** : courbe à partir des events `nav` et des
   `total_assets / total_shares` portés par chaque opération. Relevé
   mensuel reconstitué (réponse 1 de Julien).
5. **Gouvernance** : `keeper_set`, `targets_set`, `paused`, `unpaused`,
   `emergency_exit`, et les publications de prix de référence de la
   stratégie Aquarius (event `price`).
6. **Registre des algorithmes** : `fy_alloc_v1`, hash du wasm du vault,
   date de mise en service, description (réponse 2 de Julien).
7. **Export** : CSV et JSON de la piste d'audit, champs alignés sur la
   correspondance D6a §5 (registre exploitable par un système informatique,
   art. 2(1)).
8. **Frais** : section prête pour `fee_minted` (D6b), vide tant que D6b
   n'émet rien.

Graphique dessiné en SVG à la main : pas de bibliothèque de graphiques à
ajouter pour une courbe. Le design reste sobre, sur les styles existants
(`globals.css`) : c'est la part que Marius reprend.

## 7. Fichiers

- `web/lib/vaults.ts` : instance USDC, noms des stratégies.
- `web/lib/stellar.ts` : lecture générique en simulation, exportée.
- `web/lib/allocator.ts` (nouveau) : état de l'allocateur et règle de
  déclenchement.
- `web/lib/events.ts` (nouveau) : archive et RPC, décodage, fusion,
  contrôle des `seq`.
- `web/app/allocation.tsx` (nouveau) : panneau de répartition partagé.
- `web/app/page.tsx` : onglet USDC, panneau, lien dashboard.
- `web/app/dashboard/page.tsx` (nouveau).
- `web/app/globals.css`.
- `web/scripts/ingest-events.mjs` (nouveau) : ingestion, réutilise
  `@stellar/stellar-sdk` déjà installé.
- `.github/workflows/ops-daily.yml` (nouveau).
- `render.yaml`, `web/.env.example`, `web/README.md`.
- `docs/plans/2026-10-23-d5-tournage.md` : réécrit pour le parcours
  navigateur.

Aucune dépendance ajoutée, aucun contrat modifié.

## 8. Découpage

Une branche, un commit par étape, chacun vérifiable seul :

1. Lectures de l'allocateur et onglet USDC avec panneau de répartition.
2. Ingestion et tâche quotidienne (avant le 28/10).
3. Dashboard.
4. Brief de tournage et documentation.

## 9. Vérifications

- `npm run typecheck` et `npm run build` dans `web/` (la CI les rejoue).
- Ingestion lancée en local contre le testnet : archive produite, `seq` de
  1 à N sans trou, comparée aux events des transactions archivées.
- Page et dashboard servis en local (`npm run dev`), vérifiés dans Chrome :
  connexion Freighter, dépôt d'un compte de test, répartition après le
  rééquilibrage, piste d'audit, export, largeur mobile.
- Tâche GitHub Actions lancée une fois à la main (`workflow_dispatch`).

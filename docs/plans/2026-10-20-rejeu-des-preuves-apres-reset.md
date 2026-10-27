# Rejeu des preuves SCF après un reset du testnet

Procédure pour reconstruire sur un testnet réinitialisé les preuves on-chain de
D1 à D4, puis de D5 et D6 à mesure qu'elles existent. Décidée avec Julien le
20/10/2026 : on vise une soumission complète de la Tranche 2 avant le reset
annoncé les 16 et 17 décembre, et cette procédure sert si la revue glisse
au-delà.

Ce que le rejeu produit : de nouvelles instances, de nouvelles adresses et de
nouveaux hash. Les preuves d'origine ne sont pas perdues pour autant : elles
restent consignées dans les journaux datés et archivées hors chaîne dans
[`docs/evidence/archive/`](../evidence/archive/INDEX.md). Le rejeu montre que
le code publié reproduit les mêmes résultats ; il ne remplace pas l'historique.

Toutes les commandes partent de la racine du dépôt, sur `main` à jour :
`redeploy_vault.sh` refuse de déployer autre chose que `origin/main`.

---

## 0. Avant de commencer

Trois tiers redéploient eux-mêmes après un reset. Le rejeu ne peut commencer
qu'une fois qu'ils l'ont fait.

- **Le reset a bien eu lieu** : Horizon testnet répond, et le compte d'ops
  `d1-ops` n'existe plus (`curl https://horizon-testnet.stellar.org/accounts/<adresse>`
  rend 404).
- **Blend** a republié ses adresses testnet dans
  `blend-capital/blend-utils/testnet.contracts.json` (date du dernier commit
  postérieure au reset). Nos scripts relisent ce registre à chaque exécution.
- **Aquarius** : l'adresse du routeur testnet publiée sur
  [docs.aqua.network](https://docs.aqua.network/developers/reference/addresses-and-networks)
  est la même qu'avant, ou bien elle a changé. Dans ce cas, exporter
  `AQUA_ROUTER=<nouvelle adresse>` pour toute la session et consigner le
  changement dans le journal D4.
- **Circle** : le faucet EURC testnet ([faucet.circle.com](https://faucet.circle.com))
  sert de nouveau Stellar testnet.

## 1. Compte d'ops et XLM

```bash
stellar keys fund d1-ops --network testnet
```

Même identité locale qu'avant le reset : l'adresse ne change pas, seul son
solde est à recréer.

## 2. USDC Blend

Le vault D1 et le pool Aquarius utilisent l'USDC de test de Blend, le seul que
le pool Blend accepte. Le 21/07, le compte d'ops l'a obtenu en empruntant sur
le pool `TestnetV2` avec du XLM en collatéral, sans faucet. Les commandes
exactes n'ont pas été consignées à l'époque : les noter ici à la première
exécution.

Il faut au moins 101,5 USDC : 100 pour le dépôt de preuve D1 et 1,5 pour
ensemencer le pool Aquarius.

## 3. EURC

```bash
stellar tx new change-trust --source d1-ops --network testnet \
  --line EURC:GB3Q6QDZYTHWT7E5PVS3W7FUT5GVAFC5KSZFFLPU25GO7VTC3NM2ZTVO
```

Puis demander de l'EURC au faucet Circle pour l'adresse de `d1-ops`. Il faut
au moins 6,5 EURC : 5 pour le dépôt de preuve D3 et 1,5 pour ensemencer le pool
Aquarius.

## 4. AQUA, seulement si le pool USDC/EURC est à recréer

Le routeur Aquarius déployé fait payer 1 AQUA à la création d'un pool. Si le
pool existe encore après le reset, cette étape est inutile : la vérification
de l'étape 5 le dit.

```bash
stellar tx new change-trust --source d1-ops --network testnet \
  --line AQUA:GAHPYWLK6YRN7CVYZOO4H3VDRZ7PVF5UJGLZCSPAEIKJE2XSWF5LAGER
```

Puis un `swap_chained` de XLM vers AQUA sur le pool XLM/AQUA du même routeur.
L'appel du 22/07 est archivé avec ses arguments exacts dans
`docs/evidence/archive/d4/c3d0fe1f27f905da5a5c36b1398833f4898ffa663abeba83e8be3ccc96730f3f/decoded.json` :
0,1 XLM échangés contre 2,1 AQUA, la paire triée par octets d'adresse comme
pour tout appel Aquarius.

## 5. Vérification préalable

```bash
scripts/replay_preflight.sh d1-ops
```

Le script lit les soldes et l'état d'Aquarius sans rien soumettre, et renvoie
à l'étape de cette procédure pour chaque manque. Ne passer à la suite qu'avec
un code de sortie 0.

## 6. D1, D3 et la démo D2 : instances du vault

Une instance par profil, toutes issues du même wasm. `SKIP_DRAIN=1` parce que
les anciennes instances ont disparu avec le reset : il n'y a rien à vider.

```bash
SKIP_DRAIN=1 VAULT_PROFILE=d1 scripts/redeploy_vault.sh d1-ops
SKIP_DRAIN=1 VAULT_PROFILE=eurc scripts/redeploy_vault.sh d1-ops
```

Pour la démo, la clé est celle qui administre l'instance de démonstration
(voir `web/HANDOVER.local.md`, non versionné) :

```bash
SKIP_DRAIN=1 VAULT_PROFILE=demo scripts/redeploy_vault.sh <clé admin démo>
```

Chaque exécution imprime en fin de course un bloc « A CONSIGNER » : contract
ID, hash des transactions, état final. Le reporter dans une nouvelle entrée
datée du journal concerné, et suivre les mises à jour qu'il liste : défaut du
profil dans le script, tableau du README, et pour la démo
`NEXT_PUBLIC_VAULT_ID` dans `render.yaml` et le défaut testnet de
`web/lib/stellar.ts`.

Le parcours DFNS de D2 se rejoue à la main depuis la page de démo locale du
paquet `onboarding/` (`npm run demo`), une fois l'instance de démo redéployée.

## 7. D4 : pool Aquarius, SwapRouter et rééquilibrage

Le pool de D4 apparie l'EURC de Circle. `seed_aquarius_pool.sh` apparie le
FYEUR par défaut depuis D5 : `PAIRED_ASSET` est donc obligatoire ici.

```bash
PAIRED_ASSET=EURC:GB3Q6QDZYTHWT7E5PVS3W7FUT5GVAFC5KSZFFLPU25GO7VTC3NM2ZTVO \
  scripts/seed_aquarius_pool.sh d1-ops 15000000 15000000   # imprime pool_hash
scripts/deploy_swap_router.sh d1-ops <pool_hash>         # imprime contract_id
SWAP_ROUTER=<contract_id> scripts/check_aquarius.sh d1-ops
```

Si `check_aquarius.sh` signale des empreintes wasm différentes du relevé du
20/10, Aquarius a redéployé un autre code au reset. Consigner les nouvelles
empreintes dans une entrée datée du journal D4, puis relancer avec
`EXPECTED_ROUTER_WASM` et `EXPECTED_POOL_WASM` à jour.

La chaîne de rééquilibrage reste trois transactions manuelles signées par la
clé d'ops, comme le 22/07 et le 28/08. Le pool ensemencé à 1,5 / 1,5 impose un
petit montant : 0,2 USDC, comme le 28/08, pour garder un impact de prix
représentatif.

1. Retrait du vault USDC D1 :

   ```bash
   stellar contract invoke --id <vault D1> --source d1-ops --network testnet -- \
     withdraw --from <adresse d1-ops> --shares 2000000
   ```

2. Cotation et calcul de `min_out` (cotation moins 1 %), puis swap :

   ```bash
   scripts/quote_aqua.sh d1-ops <montant retiré>
   stellar contract invoke --id <contract_id SwapRouter> --source d1-ops --network testnet -- \
     swap_exact_in --from <adresse d1-ops> --token_in <USDC> --token_out <EURC> \
     --amount_in <montant retiré> --min_out <min_out_1pct>
   ```

3. Dépôt dans le vault EURC :

   ```bash
   stellar contract invoke --id <vault EURC> --source d1-ops --network testnet -- \
     deposit --from <adresse d1-ops> --amount <amount_out du swap>
   ```

## 8. D5 et D6

Reconstitue ce que consignent `docs/evidence/d5-defindex-allocator.md` et
`docs/evidence/d6-fees-audit-trail.md` : l'allocateur USDC sur Blend et
Aquarius, puis l'instance EURC à frais. Le site, la tâche quotidienne et
l'archive des events sont ensuite rebranchés sur les nouvelles adresses.

### 8.1 Soldes en plus des étapes 1 à 4

- **USDC** : 218 au total. Aux 101,5 de l'étape 2 s'ajoutent environ 112 pour
  ensemencer le pool USDC / FYEUR au taux BCE (100 FYEUR en face), 0,02 pour
  l'amorçage des deux stratégies, 2 pour le dépôt de preuve et 1,25 pour la
  séquence de démonstration. Un pool dix fois plus petit ferait dépasser au
  rééquilibrage la marge de glissement de 1 % de la stratégie Aquarius.
- **EURC** : 12,5 au total. Aux 6,5 de l'étape 3 s'ajoutent 5 pour le dépôt
  de preuve D6, 0,5 de rendement simulé, et 0,5 pour la prise 9 du tournage.
- **AQUA** : 2, un par pool créé (USDC / EURC en D4, USDC / FYEUR ici).
  L'échange de l'étape 4 en rend 2,1.
- **XLM** : le premier appel de chaque contrat prépaie 120 jours de loyer,
  527 XLM pour la stratégie Aquarius et 623 XLM pour le vault D5 en octobre.
  Les 10 000 XLM du Friendbot suffisent, le minimum par défaut de la
  vérification préalable ne le dit pas.

La vérification préalable de l'étape 5, avec ces minimums (unités 7
décimales) :

```bash
MIN_USDC=2180000000 MIN_EURC=125000000 MIN_AQUA=20000000 MIN_XLM=20000000000 \
  scripts/replay_preflight.sh d1-ops
```

### 8.2 Clés d'exploitation

`fy-ops-bot` (keeper, clé de la tâche quotidienne) et `fy-treasury`
(trésorerie des frais) sont des identités locales, comme `d1-ops` : leurs
adresses ne changent pas, seuls leurs comptes sont à recréer. Le secret
GitHub `FY_OPS_BOT_SECRET` reste valable.

```bash
stellar keys fund fy-ops-bot --network testnet
stellar keys fund fy-treasury --network testnet
stellar tx new change-trust --source fy-treasury --network testnet \
  --line EURC:GB3Q6QDZYTHWT7E5PVS3W7FUT5GVAFC5KSZFFLPU25GO7VTC3NM2ZTVO
```

La ligne de confiance permet à la trésorerie de racheter ses parts de frais.

### 8.3 D5 : FYEUR et pool USDC / FYEUR

Le FYEUR remplace l'EURC de Circle dans la jambe Aquarius : le pool USDC /
EURC du testnet est arbitré par un robot tiers loin du taux BCE (journal D5,
21/10). Le script recrée la clé émettrice `fy-eur-issuer` si elle manque,
puis le contrat d'actif, la ligne de confiance et l'émission.

```bash
scripts/issue_test_eur.sh d1-ops 10000000000          # 1 000 FYEUR
scripts/seed_aquarius_pool.sh d1-ops <USDC> 1000000000 # imprime pool_hash
```

`<USDC>` vaut 100 fois le taux BCE du jour, en unités 7 décimales : le
21/10, à 1,1234, c'était `1123400000`. Le pool naît ainsi au prix de
référence que la stratégie publiera à son déploiement.

### 8.4 D5 : allocateur

```bash
scripts/build_blend_strategy.sh        # facultatif : prouve le wasm Blend vendorisé
scripts/deploy_d5.sh d1-ops <pool_hash>
```

Le script vérifie l'empreinte du wasm Blend, compile le vault et la
stratégie Aquarius depuis `main`, déploie et amorce les deux stratégies,
puis le vault avec ses cibles (Blend 70 %, Aquarius 20 %, déclencheur
500 pb). Il imprime `blend_strategy`, `aquarius_strategy` et le vault.

Le keeper est d'abord `d1-ops`. Le confier ensuite à `fy-ops-bot` sur le
vault et sur la stratégie Aquarius, `d1-ops` restant admin :

```bash
stellar contract invoke --id <vault D5> --source d1-ops --network testnet -- \
  set_keeper --keeper $(stellar keys address fy-ops-bot)
stellar contract invoke --id <stratégie Aquarius> --source d1-ops --network testnet -- \
  set_keeper --keeper $(stellar keys address fy-ops-bot)
```

### 8.5 D5 : transactions de preuve

Dans l'ordre du journal D5 du 23/10 :

```bash
stellar contract invoke --id <vault D5> --source d1-ops --network testnet -- \
  deposit --from $(stellar keys address d1-ops) --amount 20000000
stellar contract invoke --id <vault D5> --source fy-ops-bot --network testnet -- \
  checkpoint
scripts/align_aquarius_pool.sh d1-ops <stratégie Aquarius>
stellar contract invoke --id <vault D5> --source fy-ops-bot --network testnet -- \
  rebalance --caller $(stellar keys address fy-ops-bot)
```

Attendu : le dépôt reste oisif ; `nav` publié ; un seul lot de deux
`allocate`, Blend 1,4 USDC et Aquarius 0,4 USDC, décidés par le keeper, soit
70 / 20 / 10 %.

Pour répéter la séance filmée, `scripts/d5_demo_sequence.sh` déroule les six
étapes du brief de tournage, une fois ses adresses mises à jour (8.7).

### 8.6 D6 : instance EURC à frais

```bash
scripts/deploy_d6b.sh d1-ops $(stellar keys address fy-treasury) $(stellar keys address fy-ops-bot)
```

Le script compile le vault, le déploie sans stratégie sur l'EURC de Circle,
puis règle les frais : performance 20 %, gestion 2 % par an. Transactions de
preuve, dans l'ordre du journal D6 du 23/10 :

```bash
EURC_SAC=$(stellar contract id asset --network testnet \
  --asset EURC:GB3Q6QDZYTHWT7E5PVS3W7FUT5GVAFC5KSZFFLPU25GO7VTC3NM2ZTVO)
stellar contract invoke --id <vault EURC> --source d1-ops --network testnet -- \
  deposit --from $(stellar keys address d1-ops) --amount 50000000
stellar contract invoke --id $EURC_SAC --source d1-ops --network testnet -- \
  transfer --from $(stellar keys address d1-ops) --to <vault EURC> --amount 5000000
stellar contract invoke --id <vault EURC> --source fy-ops-bot --network testnet -- \
  checkpoint
stellar contract invoke --id <vault EURC> --source fy-treasury --network testnet -- \
  withdraw --from $(stellar keys address fy-treasury) --shares <fee_shares>
```

Le transfert simule un rendement, à présenter comme tel. Attendu : un
`fee_minted` de type `perf` de 0,1 EURC, puis le rachat par la trésorerie des
`<fee_shares>` lues dans cet event. Le premier `fee_minted` de type `mgmt`
vient du checkpoint de la tâche quotidienne, le lendemain. Les deux frais dans
une même transaction relèvent de la prise 9 du tournage.

### 8.7 Rebrancher le site, la tâche et l'archive

Les nouvelles adresses remplacent les anciennes, sur une branche relue
avant fusion :

- `web/lib/vaults.ts` et `render.yaml` (`NEXT_PUBLIC_*`) : vaults D5 et EURC ;
- `.github/workflows/ops-daily.yml` : `VAULT`, `FEE_VAULT`,
  `AQUARIUS_STRATEGY` ;
- `web/scripts/ingest-events.mjs` : le vault D5, ses deux stratégies et le
  vault EURC ;
- `scripts/d5_demo_sequence.sh` : `VAULT`, `AQUARIUS` et les noms des
  stratégies ;
- `README.md` et `docs/the-arch.md` : tableaux des contrats.

La branche `data/events` garde les fichiers des anciens contrats : ce sont
leurs preuves. Ceux des nouveaux se créent à la première ingestion. Après la
fusion, lancer `ops-daily` à la main (`workflow_dispatch`) et vérifier le
dashboard des deux onglets : `seq` continus, prix de référence publié.

## 9. Après chaque transaction de preuve

```bash
scripts/archive_proof.sh <d1|d2|d3|d4|d5|d6> <hash>
```

Puis une nouvelle entrée datée dans le journal du livrable. Les entrées
antérieures au reset ne se réécrivent pas : elles décrivent ce qui a été fait
à leur date.

---

## Limites connues

- **Procédure non exécutée de bout en bout.** La rejouer avant un vrai reset
  obligerait à redéployer les instances de preuve actuelles, celles que
  référencent les dossiers déjà soumis. Seule la vérification préalable
  (étape 5) a été testée, le 20/10/2026, en lecture seule.
- **L'étape 2 n'est pas scriptée** : les commandes de l'emprunt Blend du 21/07
  n'ont pas été consignées.
- **Le rejeu dépend de trois tiers** qui redéploient de leur côté : Blend,
  Circle et Aquarius. Si l'un d'eux ne revient pas, la partie qui en dépend ne
  se rejoue pas, et les preuves d'origine restent alors la seule trace, via
  l'archive et les vidéos.

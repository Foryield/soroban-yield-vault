# Rattrapage SCF Build — Roadmap D1→D6, remise le 30 septembre 2026

Référence d'exécution suite aux retours reviewers sur le Deliverable 1.
Fenêtre : mardi 21 juillet → mercredi 30 septembre 2026 (10 semaines).

## 1. État des lieux (21 juillet)

Acquis :

- Contrat `YieldVault` MVP déployé testnet (`CCKW7NFKDCOTOVUODLJ6K734ZEYT4TZLQGLIVFZZR6DLUHO6UOTENWQ6`), deposit/withdraw/pause, 8 tests verts.
- UI démo (SWK + Freighter) en ligne sur vault.for-yield.com.

Écarts vs Deliverable 1 :

1. Allocation Blend v2 USDC : absente (le cœur du livrable).
2. Parts proportionnelles : ratio codé en dur 1:1, pas de math `shares = amount × total_shares / total_assets`.
3. Instance déployée sur XLM natif, pas USDC.
4. 8 tests vs 200+ exigés ; couverture > 90 % non mesurée (aucun outillage).
5. Zéro PR mergée (tout en commits directs sur `main`).
6. Hash de transactions de preuve non consignés.

## 2. Principe de séquencement

Deux chantiers parallèles, imposés par les dépendances (pas un choix entre
deux stratégies) :

- **Piste A — cœur contrat** : D1 → D3 → D4 → D5. L'ordre est technique :
  la math proportionnelle et Blend (D1) conditionnent tout rendement ; l'EURC (D3)
  conditionne le rebalance USDC↔EURC (D4) et les frais en EURC (D6b) ; le routing
  Soroswap (D4) conditionne les positions routées de DeFindex (D5).
- **Piste B — périphérie** : D2 (wallets, aucune dépendance au contrat),
  D6a (schéma d'events), D6b (module de frais, ne dépend que de D1 et D3).

Charges pondérées : D1 = 3,5 sem · D2 = 4,5 sem (1,5 SWK + 3 DFNS) · D3 = 1 sem
(contrat déjà asset-agnostique : déploiement d'instance + preuves) · D4 = 2 sem ·
D5 = 2 sem · D6 = 4 sem (1 events + 2 frais + 1 dashboard). Total ~17 semaines-phase,
tenable en 10 semaines uniquement grâce au parallélisme des deux pistes.

## 3. Calendrier

### Piste A — cœur contrat

**S1 (21-24 juil) — dérisquage + socle process**

- Spikes bloquants (une journée chacun) : pool Blend v2 USDC sur testnet ;
  liquidité EURC/USDC testnet sur Soroswap aggregator et pools Aquarius ;
  DeFindex déployé testnet ; couverture du réseau Stellar par le compte DFNS.
- Socle évidences : CI GitHub Actions (cargo test + cargo-llvm-cov, seuil 90 %),
  règle « tout passe par PR mergée », dossier `docs/evidence/` (chaque tx de
  preuve consignée le jour même), commit de `docs/the-arch.md`.

**S1-S4 — D1, clos vendredi 14 août**

- Math de parts proportionnelles (arrondis au profit du vault, protection
  contre l'inflation de première part).
- Adaptateur Blend v2 : supply/withdraw du pool USDC, `total_assets` intégrant
  la position Blend.
- Redéploiement d'une instance initialisée sur le SAC USDC testnet ;
  tx de dépôt et retrait consignées.
- Campagne de tests vers 200+ : unitaires, cas limites, property-based (proptest)
  sur les invariants (conservation, monotonie, round-trip). Couverture mesurée > 90 %.
- Suivis actés en revue (pass 1, 21/07) : paramètres de slippage
  (`min_shares_out` / `min_amount_out`) à trancher avant le redéploiement
  testnet ; erreurs typées `#[contracterror]` à faire pendant la campagne
  de tests (les panics chaîne sont fragiles pour les intégrateurs).
- Suivis actés en revue (pass 2 Blend, 21/07) : test de retrait en pénurie de
  liquidité pool (`max_util` atteint via un second actif de collatéral, revert
  `InvalidUtilRate` vérifié en sonde par le reviewer) à ajouter à la campagne ;
  chemin de désallocation d'urgence / migration de pool = décision Tranche 2
  (risque accepté D1, documenté dans l'en-tête du contrat).

**S5 — D3, clos vendredi 21 août**

- Instance vault initialisée sur le SAC EURC testnet.
- Tx de dépôt et de rachat EURC, hashes + contract ID + vidéo walkthrough.

**S5-S7 — D4, clos vendredi 4 septembre**

- Soroswap aggregator en venue primaire, fallback Aquarius.
- Protection slippage, comptabilité des frais de swap, sélection best-execution.
- Rebalance USDC↔EURC démontré sur testnet (hashes + vidéo + PR).

**S8-S9 — D5, clos vendredi 18 septembre**

- Allocateur DeFindex sur au moins 2 stratégies (Blend v2 + Aquarius).
- Triggers de rebalance, allocation batch optimisée.
- Adresse montrant la distribution + hashes par allocation + vidéo.

### Piste B — périphérie

**S1-S2 — D2-SWK, clos vendredi 31 juillet**

- Multi-wallet via le kit : xBull, Albedo, Lobstr, Ledger.
- Config réseau mainnet, durcissement signature/session/erreurs.
- Screenshots de connexion par wallet dans `docs/evidence/`.

**S3-S6 — D2-DFNS, clos vendredi 28 août**

- Wallet embarqué compatible Soroban provisionné depuis un login email/social
  (ni extension, ni seed phrase).
- Dépôt complet sur le vault testnet depuis ce wallet.
- Vidéo walkthrough d'onboarding.

**S6-S7 — D6a schéma d'events, clos vendredi 4 septembre**

- Schéma d'events Soroban pour l'audit trail AMF.
- Appliqué au fil de l'eau sur D4/D5 à mesure que les modules émettent
  (pas de rétrofit en fin de chantier).

**S8-S9 — D6b module de frais, clos vendredi 18 septembre**

- High-water mark, split atomique frais de performance / frais de gestion en EURC.
- Ne dépend que de D1 et D3, d'où le parallèle avec D5.

**S10 (21-25 sept) — D6c dashboard + clôture D6, vendredi 25 septembre**

- Accrual et distribution de frais démontrés sur testnet.
- Events consommés par le visualiseur, screenshot.

### Clôture

**S11 (28-30 sept) — buffer + remise mercredi 30 septembre**

- Relecture du pack d'évidences des 6 livrables (PRs, hashes, vidéos, screenshots).
- Tag de release, soumission.

## 4. Jalons récapitulatifs

- ven 31 juil : D2-SWK
- ven 14 août : D1
- ven 21 août : D3
- ven 28 août : D2-DFNS (D2 complet)
- ven 4 sept : D4 + D6a
- ven 18 sept : D5 + D6b
- ven 25 sept : D6 complet
- mer 30 sept : remise

## 5. Risques

1. **Liquidité testnet des protocoles tiers.** Blend v2, Soroswap, Aquarius et
   DeFindex sur testnet avec des paires EURC/USDC utilisables : hypothèse
   fondatrice de D1/D4/D5, non vérifiée. Les spikes de S1 sont bloquants ; toute
   mauvaise surprise doit être renégociée avec le reviewer avant le 31 juillet
   (pool seedé par nous, ou fork local documenté).
2. **Buffer réduit à 3 jours.** Aucun mou entre D5, D6b et D6c : un glissement
   d'une semaine sur D1 se propage jusqu'au 25 septembre sans amortisseur.
3. **DFNS côté Stellar.** Vérifier en S1 que le compte DFNS couvre le réseau
   Stellar (provisioning de wallet + signature Soroban) avant d'engager D2.

## 6. Décision à trancher (avant fin août)

**Dashboard D6c : standalone ou intégré.** Recommandation : visualiseur d'events
standalone dans `web/` de ce repo. Satisfait la mesure du livrable (dashboard
consommant les events de conformité) avec une surface minimale et auditable en
un seul repo. Le branchement au back-office compliance interne de For Yield
reste possible ensuite ; c'est une décision de direction, à prendre avant fin août.

## 7. Discipline d'évidences (cause racine des retours)

Chaque livrable se clôt le vendredi avec ses preuves déjà rangées : PRs mergées,
hashes dans `docs/evidence/`, vidéo uploadée. Le packaging de S11 est une
relecture, pas une production.

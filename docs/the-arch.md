# ForYield × Stellar — Démo Soroban (The Arch / SCF Build)

Doc de référence pour la démo testnet : ce qui tourne, où, et ce qu'on a branché
sur Stellar. Tout est sur **Testnet**, sans valeur, jetable.

---

## 1. En une phrase

**ForYield** est le premier coffre de rendement DeFi régulé MiCA (France), déployé
sur **Stellar / Soroban**. La démo montre le cœur on-chain — un `YieldVault` où un
utilisateur dépose un actif et reçoit des parts — piloté depuis une UI web qui signe
avec un wallet Stellar réel.

Périmètre volontairement minimal (Tranche 1 / MVP) : **dépôt → émission de parts 1:1
→ retrait**, plus une **pause d'urgence** admin. Pas encore de stratégie de rendement
branchée (le ratio parts:actif reste 1:1).

> **Alignement stratégique SDF.** *ForYield aligns with two SDF strategic priorities:
> native EURC settlement and MiCA EU regulated DeFi access.* Le contrat est
> asset-agnostique et cible un SAC **EURC** en production (règlement natif en euro sur
> Stellar) ; l'enveloppe juridique est un coffre DeFi **régulé MiCA** ouvrant l'accès
> au rendement on-chain à des opérateurs et investisseurs européens.

---

## 2. Adresses & réseau (Testnet)

| Élément | Valeur |
|---|---|
| **YieldVault (contrat)** | `CCKW7NFKDCOTOVUODLJ6K734ZEYT4TZLQGLIVFZZR6DLUHO6UOTENWQ6` |
| **Actif déposé — XLM natif (SAC)** | `CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC` |
| **Wallet de démo (public)** | `GB73BVQU3QPX44S7MSHULQLWHKTUC3GZBOEW4IUXHQZRAPYBBUAWCH3G` |
| **Réseau** | Stellar Testnet — passphrase `Test SDF Network ; September 2015` |
| **Décimales** | 7 (montants exprimés en *stroops* : `0.1 XLM = 1 000 000`) |

> La démo utilise le **XLM natif** (via son StellarAssetContract) plutôt qu'un USDC de
> test : ainsi n'importe quel compte financé par Friendbot peut déposer, sans faucet
> spécifique. Le contrat reste *asset-agnostique* — l'actif est fixé une seule fois à
> `initialize`, donc un SAC USDC/EURC se branche sans changer le code.

### Endpoints

| Service | URL |
|---|---|
| Soroban RPC | `https://soroban-testnet.stellar.org` |
| Horizon | `https://horizon-testnet.stellar.org` |
| Friendbot (financement compte) | `https://friendbot.stellar.org/?addr=<G...>` |
| Explorer (contrat) | https://stellar.expert/explorer/testnet/contract/CCKW7NFKDCOTOVUODLJ6K734ZEYT4TZLQGLIVFZZR6DLUHO6UOTENWQ6 |
| Démo web | https://vault.for-yield.com |

---

## 3. Ce qu'on a branché (architecture)

```
 Navigateur (vault.for-yield.com)
        │
        │  1. Connect           ┌────────────────────────┐
        ├──────────────────────▶│  Freighter (extension) │  signature locale
        │                       └────────────────────────┘
        │  2. lecture solde XLM
        ├──────────────────────▶  Horizon (testnet)
        │
        │  3. build deposit() + prepareTransaction (simulation)
        ├──────────────────────▶  Soroban RPC (testnet)
        │
        │  4. submit tx signée + polling du résultat
        ├──────────────────────▶  Soroban RPC ──▶ contrat YieldVault
        │
        │  5. lien preuve
        └──────────────────────▶  Stellar Expert
```

**Briques côté web** (`web/`, Next.js 14 App Router, export statique) :

- **`@creit.tech/stellar-wallets-kit`** — connexion wallet (Freighter sur Testnet ;
  xBull / Albedo dispo via le même kit). Ouvre la modale, récupère l'adresse, route la
  signature vers l'extension.
- **`@stellar/stellar-sdk`** — construit l'invocation `vault.deposit(from, amount)`,
  la prépare via le RPC (simulation + footprint), soumet la tx signée, et lit le
  résultat. Lecture du solde XLM via `Horizon.Server`.
- **Friendbot** — bouton « Fund with Friendbot » pour activer un compte testnet non
  financé (un compte Stellar n'existe on-chain qu'après financement).
- **Stellar Expert** — lien de preuve vers la transaction / l'opération Soroban.

**Brique on-chain** (`contracts/vault/`, Rust + `soroban-sdk`) : le contrat `YieldVault`
décrit en §4.

**Hébergement** : export statique Next.js (`output: export` → `web/out`) déployé sur
**Render** (Static Site, blueprint `render.yaml`). Aucun serveur, aucun secret, free tier
toujours en ligne. Domaine custom `vault.for-yield.com` via CNAME.

---

## 4. Le contrat `YieldVault`

Source : [`contracts/vault/src/lib.rs`](../contracts/vault/src/lib.rs). Interface publique :

| Fonction | Description |
|---|---|
| `initialize(admin, asset)` | Fixe l'admin et l'actif déposé (one-shot ; second appel → panic). |
| `deposit(from, amount) -> shares` | `require_auth(from)`, transfère `amount` de l'actif vers le vault, émet des parts (1:1 au MVP). |
| `withdraw(from, shares) -> amount` | Burn des parts, restitue l'actif (1:1 au MVP). |
| `total_assets() -> i128` | Actif réellement détenu (lecture du solde token on-chain). |
| `shares_of(owner) -> i128` | Parts d'une adresse. |
| `total_shares() -> i128` | Total des parts émises. |
| `pause()` / `unpause()` | Coupe-circuit admin (auth requise). |
| `is_paused() -> bool` | État de pause. |

### Events structurés — la piste d'audit (argument moat)

Chaque `deposit` / `withdraw` (et chaque `pause` / `unpause`) émet un **événement Soroban
structuré** : topics typés (`deposit`, adresse) + payload (`amount`, `shares`). Ces events
sont indexés par le ledger, **immuables et horodatés au bloc** — chaque mouvement de fonds
et chaque action admin laisse une trace cryptographiquement vérifiable, sans dépendre d'une
base off-chain.

C'est précisément ce qu'attend une **piste d'audit conforme AMF** (et l'art. de tenue de
registres MiCA) : reconstituer qui a déposé/retiré quoi, quand, et qui a déclenché un
coupe-circuit, à partir d'une source de vérité on-chain. Un opérateur régulé branche son
reporting directement sur le flux d'events — pas de réconciliation manuelle, pas de registre
falsifiable. **C'est notre différenciateur** face aux vaults non-régulés : la conformité est
native au protocole, pas un sur-couche.

**Build & test :**

```bash
rustup target add wasm32v1-none
cargo test                 # invariants (conservation des parts, pause, garde d'init)
stellar contract build     # -> target/wasm32v1-none/release/yield_vault.wasm
```

---

## 5. Security model

| Sujet | État MVP (testnet) | Cible production |
|---|---|---|
| **Admin** | Clé unique fixée à `initialize` (`require_auth`). | **Multisig** (Stellar native multisig / smart-account) — quorum M-of-N, pas de clé solo. |
| **Changement d'admin** | Aucune rotation exposée au MVP (admin one-shot). | Rotation via fonction `set_admin` gardée par le multisig + **timelock** (délai public avant effet) sur les actions sensibles (rotation, upgrade). |
| **Pause / coupe-circuit** | `pause()` / `unpause()` réservés à l'admin (`require_auth`) ; en pause, `deposit` / `withdraw` rejettent. Limite les dégâts en cas d'anomalie. | Idem, déclenchable par un sous-ensemble du multisig (réaction rapide) ; `unpause` exige le quorum plein. |
| **Surface de confiance** | Pas de stratégie branchée → fonds détenus par le seul contrat vault, ratio 1:1 vérifiable on-chain (`total_assets`). | Allocateurs externes (cf. §8) ajoutés derrière des **caps par protocole** et une logique de retrait d'urgence. |

**Plan d'audit.** Tests d'invariants en CI dès le MVP (conservation des parts, garde
d'init, comportement en pause). Audit formel **Certora** planifié en **Tranche 3**, financé
via un **crédit Stellar LaunchKit** — *pas* prélevé sur le grant SCF. Aucun déploiement
mainnet avant clôture de l'audit.

> Principe : au MVP, la sécurité repose sur une surface minimale (un seul contrat, pas de
> dépendance externe, ratio 1:1 vérifiable) ; la décentralisation de l'admin (multisig +
> timelock) précède l'ajout de toute stratégie qui élargit la surface de confiance.

---

## 6. Le parcours démo (≈30 s)

1. **Connect Wallet** → modale Stellar Wallets Kit → **Freighter** (Testnet) → approuver.
2. La carte affiche l'adresse (`GB73…CH3G`) et le **solde XLM** (lu sur Horizon).
3. Montant par défaut **0.1 XLM** → **Deposit**.
4. Freighter ouvre la popup de signature → **Confirm**.
5. La UI passe en « Confirm in Freighter… » puis **« Deposit confirmed »** + lien
   **View on Stellar Expert**.
6. Sur Stellar Expert : opération Soroban **invoke contract function `deposit`**. Le solde
   XLM baisse de ~0.1 (dépôt + frais réseau négligeables) — preuve que ça a bougé on-chain.

> Compte non financé ? Le bouton **Fund with Friendbot** l'active avant de pouvoir déposer.

---

## 7. Notes techniques (gotchas testnet)

Deux points réglés pendant la mise au point de la démo, utiles à connaître :

- **Autorisation Freighter avant signature.** Le connect initial n'garantit pas que le
  domaine soit encore dans l'allowlist de Freighter au moment de signer (`getAddress`
  peut renvoyer une clé en cache sans autorisation vivante → warning *« … is not currently
  connected »*). On re-demande l'accès juste avant `signTransaction` ; idempotent si déjà
  autorisé.
- **Protocole 23.** Le testnet renvoie désormais un `TransactionMeta` v4. Le `stellar-sdk`
  doit être ≥ 14 (qui embarque `stellar-base` ≥ 14) pour le décoder — sinon erreur
  `Bad union switch: 4` au *parsing* du résultat (la tx, elle, passe quand même on-chain).

---

## 8. Périmètre & suite

| Tranche | Contenu |
|---|---|
| **1 — MVP (ici)** | Contrat dépôt/retrait/pause, onboarding wallet, wrapper SAC EURC. |
| **2 — Testnet** | Allocateur DeFindex, routing DEX Soroswap, LP Aquarius, lending Blend v2, frais de performance (high-water mark), schéma d'événements de conformité. |
| **3 — Mainnet** | Audit Certora (via crédit Stellar LaunchKit), déploiement mainnet, onboarding cross-chain, dashboard investisseur. |

### Integration List — Tranche 2 (quoi brancher, et comment)

Le vault MVP est asset-agnostique et neutre en stratégie ; la Tranche 2 lui ajoute une
couche d'allocation derrière l'interface `deposit`/`withdraw` existante, sans casser le
ratio de parts. Quatre intégrations Stellar/Soroban, chacune derrière un cap par protocole
et un chemin de retrait d'urgence :

- **DeFindex — allocator.** Couche d'orchestration de stratégies. Le vault délègue la
  répartition des fonds déposés à une stratégie DeFindex (allocation cible par protocole,
  rebalancing). ForYield reste le contrat de garde et de parts ; DeFindex porte la logique
  *où placer* le capital. Branchement : le vault appelle l'allocateur à `deposit`, le
  retire à `withdraw`, et lit la valeur de marché pour faire évoluer le ratio parts:actif
  au-delà du 1:1.

- **Soroswap — routing DEX.** Aggrégateur/router de swap on-chain. Sert à entrer/sortir
  d'une position et à convertir entre actifs (ex. EURC ↔ actif de stratégie) au meilleur
  prix via routing multi-hop. Branchement : appelé par l'allocateur lors d'un rebalancing
  ou d'un retrait nécessitant une conversion, avec slippage borné.

- **Aquarius — LP / market-making.** Fourniture de liquidité et incitations (AMM + reward
  pools). Utilisé comme *venue de rendement* : une part de l'allocation est placée en
  liquidité sur des pools Aquarius pour capter frais de swap et émissions. Branchement :
  stratégie « LP » exposée à l'allocateur, avec suivi des positions et des rewards dans la
  valorisation `total_assets`.

- **Blend v2 — lending.** Marché de prêt/emprunt isolé par pools. Utilisé comme brique de
  rendement à faible risque : dépôt de l'actif (ex. EURC) dans un pool de lending Blend
  pour un yield variable. Branchement : stratégie « lend » exposée à l'allocateur ;
  l'intérêt accru augmente `total_assets`, donc la valeur de chaque part.

Hors-scope MVP donc : allocation multi-protocole, frais, parts SEP-41 transférables. Le
ratio parts:actif reste **1:1** tant qu'aucune stratégie n'est branchée.

---

## 9. Liens

- **Démo** : https://vault.for-yield.com
- **Contrat (explorer)** : https://stellar.expert/explorer/testnet/contract/CCKW7NFKDCOTOVUODLJ6K734ZEYT4TZLQGLIVFZZR6DLUHO6UOTENWQ6
- **Code** : `contracts/vault/` (Rust/Soroban) · `web/` (Next.js)
- **Licence** : MIT — tout opérateur EU régulé peut forker et lancer sur Stellar.

# D2-DFNS — Onboarding wallet embarqué : plan d'implémentation

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Prouver sur testnet le parcours d'onboarding complet du livrable D2 : un wallet Stellar provisionné par DFNS depuis un identifiant email (ni extension, ni seed phrase) qui complète un dépôt Soroban sur le vault démo.

**Architecture:** Un package TypeScript `onboarding/` à trois briques à coutures JSON franches, conçues pour être réutilisées telles quelles par l'application For Yield : `provision` (créer + financer le wallet DFNS), `envelope` (construire et simuler l'invocation Soroban), `submit` (soumettre à DFNS et confirmer on-chain). Par-dessus, un orchestrateur CLI `onboard` et une façade web locale minimale pour la vidéo walkthrough. Les credentials DFNS (testnet uniquement) vivent en variables d'environnement, jamais dans le repo.

**Tech Stack:** TypeScript + Node 20, `@dfns/sdk` + `@dfns/sdk-keysigner` (signature User Action du service account), `@stellar/stellar-sdk` (build + simulate Soroban), `vitest` (tests unitaires), `express` (façade vidéo locale). Vault démo XLM natif : `CCKW7NFKDCOTOVUODLJ6K734ZEYT4TZLQGLIVFZZR6DLUHO6UOTENWQ6`.

**Chaîne validée en amont (spike du 21/07, `docs/evidence/d2-wallet-onboarding.md`) :** wallet DFNS `StellarTestnet` → enveloppe simulée → `POST /wallets/{id}/transactions` (`kind: Transaction`, hex) → `Broadcasted` → dépôt réussi on-chain (tx `d5047db5…`). Le plan industrialise cette chaîne.

**Risques connus :**
- Une policy DFNS peut intercepter la transaction (`status` ≠ `Broadcasted`) : `submit` doit exposer le statut brut et échouer clairement sur `PendingApproval`/`Failed`.
- Le numéro de séquence de l'enveloppe se périme : construire l'enveloppe juste avant la soumission (l'orchestrateur enchaîne les deux).
- Les tests unitaires ne touchent ni DFNS ni le réseau (pas de credentials en CI) : les chemins réseau se prouvent par les runs testnet consignés en évidence.

---

### Task 0 : Squelette du package

**Files:**
- Create: `onboarding/package.json`
- Create: `onboarding/tsconfig.json`
- Create: `onboarding/.env.example`
- Create: `onboarding/.gitignore`
- Create: `onboarding/README.md`

**Step 1 : Créer la branche**

```bash
git checkout main && git pull && git checkout -b feat/d2-dfns-onboarding
```

**Step 2 : Initialiser le package**

`onboarding/package.json` :

```json
{
  "name": "@foryield/stellar-onboarding",
  "private": true,
  "type": "module",
  "scripts": {
    "test": "vitest run",
    "provision": "tsx src/cli/provision.ts",
    "envelope": "tsx src/cli/envelope.ts",
    "submit": "tsx src/cli/submit.ts",
    "onboard": "tsx src/cli/onboard.ts",
    "demo": "tsx src/demo/server.ts"
  }
}
```

```bash
cd onboarding
npm install @dfns/sdk @dfns/sdk-keysigner @stellar/stellar-sdk express
npm install -D typescript tsx vitest @types/node @types/express
```

`onboarding/tsconfig.json` : `strict: true`, `module: "NodeNext"`, `target: "ES2022"`.

`onboarding/.gitignore` :

```
node_modules/
.env
```

`onboarding/.env.example` (noms seulement, valeurs fournies hors bande, testnet uniquement) :

```
DFNS_API_URL=https://api.dfns.io
DFNS_AUTH_TOKEN=
DFNS_CRED_ID=
DFNS_PRIVATE_KEY=
STELLAR_RPC_URL=https://soroban-testnet.stellar.org
STELLAR_HORIZON_URL=https://horizon-testnet.stellar.org
VAULT_CONTRACT_ID=CCKW7NFKDCOTOVUODLJ6K734ZEYT4TZLQGLIVFZZR6DLUHO6UOTENWQ6
```

**Step 3 : Vérifier que la CI existante ignore le package (build cargo intact)**

Run: `cargo test --workspace` à la racine.
Expected: vert, inchangé.

**Step 4 : Commit**

```bash
git add onboarding/package.json onboarding/package-lock.json onboarding/tsconfig.json onboarding/.env.example onboarding/.gitignore onboarding/README.md
git commit -m "chore(onboarding): scaffold TypeScript package for D2 DFNS onboarding"
```

---

### Task 1 : Config fail-closed

**Files:**
- Create: `onboarding/src/config.ts`
- Test: `onboarding/test/config.test.ts`

**Step 1 : Test qui échoue**

```ts
import { describe, it, expect } from "vitest";
import { loadConfig } from "../src/config.js";

const FULL_ENV = {
  DFNS_AUTH_TOKEN: "t", DFNS_CRED_ID: "c", DFNS_PRIVATE_KEY: "k",
};

describe("loadConfig", () => {
  it("throws when a required DFNS var is missing", () => {
    expect(() => loadConfig({ ...FULL_ENV, DFNS_AUTH_TOKEN: undefined }))
      .toThrow(/DFNS_AUTH_TOKEN/);
  });

  it("applies public defaults for network endpoints", () => {
    const cfg = loadConfig(FULL_ENV);
    expect(cfg.rpcUrl).toBe("https://soroban-testnet.stellar.org");
    expect(cfg.network).toBe("StellarTestnet");
  });
});
```

Run: `npx vitest run test/config.test.ts` — Expected: FAIL (module absent).

**Step 2 : Implémentation minimale**

```ts
export type Config = {
  dfnsApiUrl: string; dfnsAuthToken: string; dfnsCredId: string; dfnsPrivateKey: string;
  rpcUrl: string; horizonUrl: string; vaultContractId: string;
  network: "StellarTestnet";
};

type Env = Record<string, string | undefined>;

function required(env: Env, key: string): string {
  const value = env[key];
  if (!value) throw new Error(`Missing required env var: ${key}`);
  return value;
}

export function loadConfig(env: Env = process.env): Config {
  return {
    dfnsApiUrl: env.DFNS_API_URL ?? "https://api.dfns.io",
    dfnsAuthToken: required(env, "DFNS_AUTH_TOKEN"),
    dfnsCredId: required(env, "DFNS_CRED_ID"),
    dfnsPrivateKey: required(env, "DFNS_PRIVATE_KEY"),
    rpcUrl: env.STELLAR_RPC_URL ?? "https://soroban-testnet.stellar.org",
    horizonUrl: env.STELLAR_HORIZON_URL ?? "https://horizon-testnet.stellar.org",
    vaultContractId: env.VAULT_CONTRACT_ID ?? "CCKW7NFKDCOTOVUODLJ6K734ZEYT4TZLQGLIVFZZR6DLUHO6UOTENWQ6",
    network: "StellarTestnet",
  };
}
```

**Step 3 : Vert** — `npx vitest run test/config.test.ts` — Expected: PASS.

**Step 4 : Commit** — `git add onboarding/src/config.ts onboarding/test/config.test.ts && git commit -m "feat(onboarding): fail-closed env config"`

---

### Task 2 : Client DFNS

**Files:**
- Create: `onboarding/src/dfns.ts`

Brique fine sans logique propre : pas de test unitaire dédié (la valeur est prouvée par les runs testnet).

**Step 1 : Implémentation**

```ts
import { DfnsApiClient } from "@dfns/sdk";
import { AsymmetricKeySigner } from "@dfns/sdk-keysigner";
import type { Config } from "./config.js";

export function dfnsClient(cfg: Config): DfnsApiClient {
  return new DfnsApiClient({
    baseUrl: cfg.dfnsApiUrl,
    authToken: cfg.dfnsAuthToken,
    signer: new AsymmetricKeySigner({
      credId: cfg.dfnsCredId,
      privateKey: cfg.dfnsPrivateKey,
    }),
  });
}
```

**Step 2 : Type-check** — `npx tsc --noEmit` — Expected: 0 erreur.

**Step 3 : Commit** — `git commit -m "feat(onboarding): DFNS client with user-action signer"`

---

### Task 3 : Brique provision (wallet + Friendbot)

**Files:**
- Create: `onboarding/src/provision.ts`
- Create: `onboarding/src/cli/provision.ts`
- Test: `onboarding/test/provision.test.ts`

**Step 1 : Tests qui échouent**

```ts
import { describe, it, expect, vi } from "vitest";
import { provisionWallet, fundWithFriendbot } from "../src/provision.js";

describe("provisionWallet", () => {
  it("creates a StellarTestnet wallet and returns id + address", async () => {
    const client = { wallets: { createWallet: vi.fn().mockResolvedValue({ id: "wa-1", address: "GABC" }) } };
    const result = await provisionWallet(client as never, "user@example.com");
    expect(client.wallets.createWallet).toHaveBeenCalledWith({
      body: { network: "StellarTestnet", name: "user@example.com" },
    });
    expect(result).toEqual({ walletId: "wa-1", address: "GABC" });
  });
});

describe("fundWithFriendbot", () => {
  it("fails loudly when friendbot rejects", async () => {
    const fetchImpl = vi.fn().mockResolvedValue({ ok: false, status: 400 });
    await expect(fundWithFriendbot("GABC", fetchImpl as never)).rejects.toThrow(/friendbot/i);
  });
});
```

Run: `npx vitest run test/provision.test.ts` — Expected: FAIL.

**Step 2 : Implémentation**

```ts
import type { DfnsApiClient } from "@dfns/sdk";

export type ProvisionedWallet = { walletId: string; address: string };

export async function provisionWallet(client: DfnsApiClient, name: string): Promise<ProvisionedWallet> {
  const wallet = await client.wallets.createWallet({
    body: { network: "StellarTestnet", name },
  });
  return { walletId: wallet.id, address: wallet.address as string };
}

// Testnet only: Friendbot creates and funds the on-chain account (10k test XLM).
export async function fundWithFriendbot(address: string, fetchImpl: typeof fetch = fetch): Promise<void> {
  const response = await fetchImpl(`https://friendbot.stellar.org/?addr=${encodeURIComponent(address)}`);
  if (!response.ok) throw new Error(`friendbot funding failed for ${address}: HTTP ${response.status}`);
}
```

CLI `src/cli/provision.ts` : `loadConfig` → `provisionWallet(client, process.argv[2])` → `fundWithFriendbot` → `console.log(JSON.stringify(result))`.

**Step 3 : Vert** — `npx vitest run test/provision.test.ts` — Expected: PASS.

**Step 4 : Run testnet réel (credentials en `.env`)**

Run: `npm run provision -- demo@example.com`
Expected: JSON `{"walletId":"wa-…","address":"G…"}` ; le compte existe sur Horizon avec 10 000 XLM.

**Step 5 : Commit** — `git commit -m "feat(onboarding): provision brick - DFNS wallet + friendbot funding"`

---

### Task 4 : Brique envelope (build + simulate Soroban)

**Files:**
- Create: `onboarding/src/envelope.ts`
- Create: `onboarding/src/cli/envelope.ts`
- Test: `onboarding/test/envelope.test.ts`

**Step 1 : Tests qui échouent** (le serveur RPC est injecté ; seul le chemin d'erreur se simule à froid, le chemin heureux se prouve sur testnet)

```ts
import { describe, it, expect, vi } from "vitest";
import { depositArgs, buildInvocationHex } from "../src/envelope.js";
import { Address } from "@stellar/stellar-sdk";

describe("depositArgs", () => {
  it("encodes from + stroops as ScVal [Address, i128]", () => {
    const [from, amount] = depositArgs("GCUKCTOCRTLX52H2BWAA4EL5TE5PCECUSKFOG7BALI2TPFZRLIHJC5RS", 1_000_000n);
    expect(Address.fromScVal(from).toString()).toBe("GCUKCTOCRTLX52H2BWAA4EL5TE5PCECUSKFOG7BALI2TPFZRLIHJC5RS");
    expect(amount.switch().name).toBe("scvI128");
  });
});

describe("buildInvocationHex", () => {
  it("throws with the simulation error when simulate fails", async () => {
    const server = {
      getAccount: vi.fn().mockResolvedValue(fakeAccount()),
      simulateTransaction: vi.fn().mockResolvedValue({ error: "host invocation failed" }),
    };
    await expect(buildInvocationHex(opts(), server as never)).rejects.toThrow(/host invocation failed/);
  });
});
```

(`fakeAccount()` : `new Account(G_ADDRESS, "1")` de stellar-sdk ; `opts()` : contractId + method + args + source.)

Run: `npx vitest run test/envelope.test.ts` — Expected: FAIL.

**Step 2 : Implémentation**

```ts
import {
  Account, Address, BASE_FEE, Contract, Networks, TransactionBuilder, nativeToScVal, rpc, xdr,
} from "@stellar/stellar-sdk";

export type InvocationRequest = {
  contractId: string;
  method: string;
  args: xdr.ScVal[];
  source: string;      // G… address; must also be the auth invoker (source-account auth)
  rpcUrl: string;
};

export type BuiltEnvelope = { xdrBase64: string; hex: string };

export function depositArgs(from: string, stroops: bigint): xdr.ScVal[] {
  return [new Address(from).toScVal(), nativeToScVal(stroops, { type: "i128" })];
}

export async function buildInvocationHex(
  request: InvocationRequest,
  server: rpc.Server = new rpc.Server(request.rpcUrl),
): Promise<BuiltEnvelope> {
  const account: Account = await server.getAccount(request.source);
  const tx = new TransactionBuilder(account, { fee: BASE_FEE, networkPassphrase: Networks.TESTNET })
    .addOperation(new Contract(request.contractId).call(request.method, ...request.args))
    .setTimeout(300)
    .build();

  const simulation = await server.simulateTransaction(tx);
  if (!rpc.Api.isSimulationSuccess(simulation)) {
    throw new Error(`simulation failed: ${"error" in simulation ? simulation.error : "unknown"}`);
  }

  const assembled = rpc.assembleTransaction(tx, simulation).build();
  const raw = assembled.toEnvelope().toXDR();
  return { xdrBase64: assembled.toXDR(), hex: `0x${Buffer.from(raw).toString("hex")}` };
}
```

CLI `src/cli/envelope.ts` : args `<method> <source> <amount>` → `deposit`/`withdraw` → JSON `{xdrBase64, hex}` sur stdout (c'est la couture réutilisable en sous-processus par n'importe quel backend).

**Step 3 : Vert** — `npx vitest run test/envelope.test.ts` — Expected: PASS.

**Step 4 : Run testnet réel** — `npm run envelope -- deposit G… 1000000` — Expected: JSON avec `hex` commençant par `0x`.

**Step 5 : Commit** — `git commit -m "feat(onboarding): envelope brick - build + simulate Soroban invocation"`

---

### Task 5 : Brique submit (DFNS broadcast + confirmation Horizon)

**Files:**
- Create: `onboarding/src/submit.ts`
- Create: `onboarding/src/cli/submit.ts`
- Test: `onboarding/test/submit.test.ts`

**Step 1 : Tests qui échouent**

```ts
import { describe, it, expect, vi } from "vitest";
import { submitViaDfns, waitForInclusion } from "../src/submit.js";

describe("submitViaDfns", () => {
  it("returns the tx hash when DFNS broadcasts", async () => {
    const client = { wallets: { broadcastTransaction: vi.fn().mockResolvedValue({
      id: "tx-1", status: "Broadcasted", txHash: "abc123",
    }) } };
    const result = await submitViaDfns(client as never, "wa-1", "0xdead");
    expect(result).toEqual({ requestId: "tx-1", status: "Broadcasted", txHash: "abc123" });
  });

  it("throws a clear error when a policy holds the transaction", async () => {
    const client = { wallets: { broadcastTransaction: vi.fn().mockResolvedValue({
      id: "tx-2", status: "PendingApproval",
    }) } };
    await expect(submitViaDfns(client as never, "wa-1", "0xdead")).rejects.toThrow(/PendingApproval/);
  });
});

describe("waitForInclusion", () => {
  it("resolves once Horizon reports the transaction successful", async () => {
    const fetchImpl = vi.fn()
      .mockResolvedValueOnce({ status: 404 })
      .mockResolvedValueOnce({ status: 200, ok: true, json: async () => ({ successful: true, ledger: 42 }) });
    const result = await waitForInclusion("https://horizon", "abc123", { fetchImpl: fetchImpl as never, delayMs: 0 });
    expect(result).toEqual({ successful: true, ledger: 42 });
  });
});
```

Run: `npx vitest run test/submit.test.ts` — Expected: FAIL.

**Step 2 : Implémentation**

```ts
import type { DfnsApiClient } from "@dfns/sdk";

export type SubmitResult = { requestId: string; status: string; txHash: string };

export async function submitViaDfns(client: DfnsApiClient, walletId: string, hex: string): Promise<SubmitResult> {
  const response = await client.wallets.broadcastTransaction({
    walletId,
    body: { kind: "Transaction", transaction: hex },
  });
  if (response.status !== "Broadcasted" || !response.txHash) {
    throw new Error(`DFNS did not broadcast: status=${response.status} (request ${response.id})`);
  }
  return { requestId: response.id, status: response.status, txHash: response.txHash };
}

export type InclusionResult = { successful: boolean; ledger: number };

export async function waitForInclusion(
  horizonUrl: string,
  txHash: string,
  options: { fetchImpl?: typeof fetch; delayMs?: number; attempts?: number } = {},
): Promise<InclusionResult> {
  const { fetchImpl = fetch, delayMs = 2000, attempts = 15 } = options;
  for (let attempt = 0; attempt < attempts; attempt += 1) {
    const response = await fetchImpl(`${horizonUrl}/transactions/${txHash}`);
    if (response.status === 200) {
      const body = await response.json();
      return { successful: body.successful === true, ledger: body.ledger };
    }
    await new Promise((resolve) => setTimeout(resolve, delayMs));
  }
  throw new Error(`transaction ${txHash} not found on Horizon after ${attempts} attempts`);
}
```

CLI `src/cli/submit.ts` : `<walletId> <hex>` → JSON `{txHash, ledger, successful}`.

**Step 3 : Vert** — `npx vitest run test/submit.test.ts` — Expected: PASS.

**Step 4 : Commit** — `git commit -m "feat(onboarding): submit brick - DFNS broadcast + Horizon confirmation"`

---

### Task 6 : Orchestrateur `onboard`

**Files:**
- Create: `onboarding/src/onboard.ts`
- Create: `onboarding/src/cli/onboard.ts`
- Test: `onboarding/test/onboard.test.ts`

**Step 1 : Test qui échoue** (toutes les briques injectées en fakes ; vérifie l'enchaînement et le JSON final)

```ts
import { describe, it, expect, vi } from "vitest";
import { onboard } from "../src/onboard.js";

it("chains provision -> fund -> envelope -> submit -> inclusion", async () => {
  const deps = {
    provision: vi.fn().mockResolvedValue({ walletId: "wa-1", address: "GABC" }),
    fund: vi.fn().mockResolvedValue(undefined),
    buildEnvelope: vi.fn().mockResolvedValue({ xdrBase64: "AA==", hex: "0xdead" }),
    submit: vi.fn().mockResolvedValue({ requestId: "tx-1", status: "Broadcasted", txHash: "abc" }),
    waitForInclusion: vi.fn().mockResolvedValue({ successful: true, ledger: 42 }),
  };
  const summary = await onboard("user@example.com", 1_000_000n, deps as never);
  expect(deps.buildEnvelope).toHaveBeenCalledAfter(deps.fund);
  expect(summary).toMatchObject({ email: "user@example.com", address: "GABC", txHash: "abc", ledger: 42, successful: true });
});
```

Run: `npx vitest run test/onboard.test.ts` — Expected: FAIL.

**Step 2 : Implémentation** — `onboard(email, stroops, deps)` assemble les briques (l'enveloppe est construite après le financement, juste avant la soumission, pour un numéro de séquence frais) et retourne le résumé JSON. Le CLI câble les vraies briques depuis `loadConfig()`.

**Step 3 : Vert** — `npx vitest run test/onboard.test.ts` — Expected: PASS.

**Step 4 : Run testnet réel de bout en bout**

Run: `npm run onboard -- reviewer-demo@example.com 1000000`
Expected: JSON final avec `successful: true` ; `shares_of(address)` = `1000000` via `stellar contract invoke … -- shares_of`.

**Step 5 : Commit** — `git commit -m "feat(onboarding): onboard orchestrator - email to completed vault deposit"`

---

### Task 7 : Façade web locale pour la vidéo

**Files:**
- Create: `onboarding/src/demo/server.ts`
- Create: `onboarding/src/demo/index.html`
- Test: `onboarding/test/demo.test.ts`

Page unique filmable en localhost : champ email → « Create my wallet » (provision + fund, affiche l'adresse) → « Deposit 0.1 XLM » (envelope + submit, affiche le hash + lien Stellar Expert). Express sert `index.html` et deux routes POST `/api/onboard` et `/api/deposit` qui appellent les briques. Locale uniquement : ne se déploie pas (les credentials restent sur la machine).

**Step 1 : Test qui échoue** — smoke test des deux routes avec briques fakes injectées (supertest ou `fetch` sur un port éphémère) : `POST /api/onboard {email}` → 200 `{address}`.

**Step 2 : Implémentation** — serveur ~60 lignes, injection des briques par paramètre pour rester testable.

**Step 3 : Vert** — `npx vitest run test/demo.test.ts` — Expected: PASS.

**Step 4 : Vérification visuelle** — `npm run demo` → dérouler le parcours complet dans le navigateur sur `http://localhost:4600`. C'est la répétition générale de la vidéo.

**Step 5 : Commit** — `git commit -m "feat(onboarding): local demo page for the walkthrough video"`

---

### Task 8 : CI

**Files:**
- Modify: `.github/workflows/` (workflow existant : ajouter un job)

**Step 1 : Ajouter le job** `onboarding-tests` : Node 20, `working-directory: onboarding`, `npm ci && npx tsc --noEmit && npm test`. Aucun secret requis (les tests unitaires ne touchent pas le réseau).

**Step 2 : Vérifier en local** — `cd onboarding && npm ci && npx tsc --noEmit && npm test` — Expected: tout vert.

**Step 3 : Commit** — `git commit -m "ci(onboarding): unit tests + type-check job"`

---

### Task 9 : Évidence + PR

**Files:**
- Modify: `docs/evidence/d2-wallet-onboarding.md`
- Modify: `onboarding/README.md`

**Step 1 : Run d'évidence** — rejouer `npm run onboard` avec un email de démo ; consigner le jour même : walletId, adresse, hash du dépôt + lien Stellar Expert, `shares_of` observé.

**Step 2 : README** — usage des 4 CLIs, `.env.example`, périmètre testnet only, et la phrase de réutilisation : chaque brique parle JSON sur stdin/stdout pour être appelée en sous-processus par n'importe quel backend.

**Step 3 : Commit + PR**

```bash
git add docs/evidence/d2-wallet-onboarding.md onboarding/README.md
git commit -m "docs(evidence): D2 DFNS onboarding - end-to-end testnet run"
```

PR `feat/d2-dfns-onboarding` → `main` (3 checks CI verts + le nouveau job). La vidéo walkthrough se tourne sur la façade locale après merge, et se référence dans l'évidence à la clôture du jalon.

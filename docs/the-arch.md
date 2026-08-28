# ForYield × Stellar — Soroban Technical Architecture

Reference document for the Stellar integration of ForYield: what runs, where, and
what it is wired to. Everything described here is on **Testnet**, worthless and
disposable.

| Version | Date | Change |
|---|---|---|
| v1 | 2026-06 | Initial MVP demo: single vault on native XLM, shares minted 1:1, no strategy. |
| v2 | 2026-08-04 | Rewritten against the delivered Tranche 1. Proportional shares, Blend v2 allocation, three vault instances, DFNS embedded onboarding, SwapRouter. Language switched to English, which is the language of the rest of the repository and of the readers this document is handed to. |

> **Regulatory status.** ForYield is not an authorised crypto-asset service
> provider at this date. This repository and the demo are not an offer of a
> financial service, and nothing here asserts a status that has not been
> obtained.

---

## 1. In one paragraph

**ForYield** is a DeFi yield vault designed for European regulatory
requirements, built on **Stellar / Soroban**. A user deposits an asset and
receives proportional vault shares; the vault supplies the deposit to a lending
protocol in the same transaction, and the interest accrues into the share price
with no action from the vault and no rebasing. Two onboarding paths reach the
same contract: a browser wallet through Stellar Wallets Kit, or an embedded
wallet provisioned from an email address through DFNS, with no extension and no
seed phrase.

> **SDF strategic alignment.** *ForYield aligns with two SDF strategic
> priorities: native EURC settlement and MiCA EU regulated DeFi access.* The
> contract is asset-agnostic and holds EURC through its StellarAssetContract
> wrapper (native euro settlement on Stellar); the legal envelope targets the
> MiCA regime, to open on-chain yield to European operators and investors.

**Delivered in Tranche 1**: proportional share accounting with first-depositor
inflation protection, allocation to a Blend v2 lending pool, EURC through its
SAC wrapper, both wallet onboarding paths, an admin emergency pause.
**Deferred to Tranches 2 and 3**: multi-protocol allocation through DeFindex,
the performance-fee module with high-water mark, SEP-41 transferable shares, the
admin surface beyond the pause (key rotation, upgrade, emergency divest).

The DEX routing layer scheduled for Tranche 2 is already code-complete and
evidenced on testnet, ahead of its milestone (section 5).

---

## 2. Deployed on testnet

Network: Stellar **Testnet**, passphrase `Test SDF Network ; September 2015`.
Amounts are raw units with 7 decimals (`0.1 XLM = 1000000`).

| Instance | Contract ID | Asset | Strategy |
|---|---|---|---|
| YieldVault, Deliverable 1 | `CCE5ITQQF4GWG5FA47D2XJBKXASWJ2E5V5AWW5U5BBAFWIXA77YYGWNI` | Blend testnet USDC (SAC) | Blend v2 TestnetV2 pool |
| YieldVault, Deliverable 3 | `CDZR2IY4V3GXUONLTVXJNCMTIR2LLFC55ZRPPEHCTI4RM7LVF25UKG5K` | Circle EURC via SAC wrapper | none (pure custody) |
| YieldVault, public demo | `CCP3EJYJ55RLZYCHABIWCTCWRHQN2BYZVXLCHZLPCCKIKA4VNK6TMCHN` | native XLM (SAC) | none (pure custody) |
| SwapRouter, Deliverable 4 | redeployment pending (2026-08-28) | USDC / EURC | Aquarius, single venue |

| Referenced contract | Address |
|---|---|
| Blend testnet USDC (SAC) | `CAQCFVLOBK5GIULPNZRGATJJMIZL5BSP7X5YJVMGCPTUEPFM4AVSRCJU` |
| Blend v2 TestnetV2 pool | `CCEBVDYM32YNYCVNRXQKDFFPISJJCV557CDZEIRBEE4NCV4KHPQ44HGF` |
| EURC SAC wrapper (Circle `EURC:GB3Q6QDZ…ZTVO`) | `CCUUDM434BMZMYWYDITHFXHDMIVTGGD6T2I5UKNX5BSLXLW7HVR4MCGZ` |
| Native XLM (SAC) | `CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC` |

The USDC address and the Blend pool are read at deployment time from the
canonical `blend-capital/blend-utils` registry rather than hard-coded, so the
instance points at the addresses Blend itself publishes.

**All three vault instances run the same bytecode**, wasm hash
`5d5001e32dc23273dff3cc4aa4f10e7fe639fddabfab9d2ea9d9ed93dbb78bba`, built from
`main`. They differ only by the asset held and by whether a Blend pool is
attached. An instance never updates itself, so this parity is a fact to check
rather than to assume:

```bash
stellar contract fetch --id <VAULT_ID> --network testnet --out-file onchain.wasm
shasum -a 256 onchain.wasm
```

### Endpoints

| Service | URL |
|---|---|
| Soroban RPC | `https://soroban-testnet.stellar.org` |
| Horizon | `https://horizon-testnet.stellar.org` |
| Friendbot | `https://friendbot.stellar.org/?addr=<G...>` |
| Web demo | https://vault.for-yield.com |

---

## 3. System architecture

Two onboarding paths, one contract. Which path a user takes changes who holds
the key, not what the vault sees.

```
  Browser wallet path                        Embedded wallet path
  (crypto-native users)                      (institutional and HNW clients)

  vault.for-yield.com                        onboarding/ package
        │                                          │
        │ 1. connect                               │ 1. email address
        ▼                                          ▼
  Stellar Wallets Kit                        DFNS API: provision a
  Freighter · xBull · Albedo                 StellarTestnet MPC wallet
  Lobstr · Ledger                            (no seed phrase, no local key)
        │                                          │
        │ 2. read balance                          │ 2. fund via Friendbot
        ▼                                          ▼
     Horizon                                    Friendbot
        │                                          │
        │ 3. build deposit(), simulate             │ 3. build + simulate,
        ▼                                          ▼   envelope to hex
   Soroban RPC                                 Soroban RPC
        │                                          │
        │ 4. sign in the wallet, submit            │ 4. broadcast through the
        ▼                                          ▼   DFNS transactions API
   ┌──────────────────────────────────────────────────────┐
   │                YieldVault contract                    │
   │   deposit → shares minted → supplied to Blend         │
   └──────────────────────────────────────────────────────┘
        │                                          │
        └────────────▶ Stellar Expert ◀────────────┘
                       (proof link)
```

**Web (`web/`)**, Next.js 14 App Router, static export, no server and no secret:

- `@creit.tech/stellar-wallets-kit` for wallet connection. Five modules are
  registered explicitly rather than through `allowAllModules()`: Freighter,
  xBull, Albedo, Lobstr, Ledger. The catch-all also loaded four wallets we do
  not support and cannot demonstrate, which would have put names in the
  connection modal that we cannot stand behind. Ledger requires an explicit
  module in any case, since it goes through WebUSB.
- `@stellar/stellar-sdk` builds the `deposit` invocation, prepares it through
  the RPC (simulation and footprint), submits the signed transaction and reads
  the result. Balances are read from Horizon.
- Network selection through `NEXT_PUBLIC_STELLAR_NETWORK`, fail-closed on
  mainnet: on mainnet the vault ID and the RPC URL have no default and must come
  from the environment, so no implicit contract can ever be addressed with real
  money.
- Wallet sessions are persisted and restored on reload.

**Onboarding (`onboarding/`)**, TypeScript, four composable bricks plus an
orchestrator: `provision` (DFNS wallet plus Friendbot), `envelope` (build and
simulate a Soroban call, output a hex envelope), `submit` (broadcast through
DFNS, confirm inclusion on Horizon), `onboard` (the chain of the three). A local
demo page drives the same flow for filming. 36 unit tests run with no
credentials, in a dedicated CI job. Credentials live outside the repository, in
`~/.config/foryield/soroban-onboarding.env`, and the loader deliberately has no
fallback to a repository-local file.

**Contracts (`contracts/`)**, Rust with `soroban-sdk` 25 and
`blend-contract-sdk` 2.25, target `wasm32v1-none`.

**Hosting**: static Next.js export deployed on Render as a static site
(`render.yaml` blueprint), custom domain `vault.for-yield.com` through a CNAME.
Security headers are set in the blueprint: `frame-ancestors 'none'` and
`X-Frame-Options: DENY` in particular, since a page whose main action is "sign
this transaction" must not be embeddable.

---

## 4. The YieldVault contract

Source: [`contracts/vault/src/lib.rs`](../contracts/vault/src/lib.rs). Nine
public functions.

| Function | Description |
|---|---|
| `initialize(admin, asset, pool)` | Sets the admin, the deposit asset and an optional Blend v2 pool. One-shot and immutable; a second call fails. |
| `deposit(from, amount) -> shares` | Pulls the asset and mints proportional shares. Requires `from`'s authorisation. |
| `withdraw(from, shares) -> amount` | Burns shares and returns the asset pro-rata. |
| `total_assets() -> i128` | Idle token balance plus the Blend position valued at bTokens × b_rate. |
| `shares_of(owner) -> i128` | Shares held by an address. |
| `total_shares() -> i128` | Total shares issued. |
| `pause()` / `unpause()` | Admin-only emergency switch. |
| `is_paused() -> bool` | Pause state. |

**Share accounting.** Shares are minted at `amount × total_shares /
assets_before`, truncated, and redeemed at `shares × assets / total_shares`,
truncated. Both roundings fall to the vault, so no sequence of deposits and
withdrawals extracts value from the holders who stay. The first deposit locks
1,000 dead shares, counted in the total and owned by nobody, which bounds the
cost of a first-share price inflation attack (Uniswap V2 and DeFindex model).
Any asset donated to the contract before that first deposit is absorbed into the
genesis total rather than handed to the first entrant.

**Allocation.** When a pool is attached, every deposit is supplied to it in the
same transaction and every withdrawal is served from it when the idle balance
falls short, so the vault holds no idle assets. The Blend position is valued
conservatively, truncated downwards. `initialize` reads the pool's reserve for
the deposit asset before accepting it and fails with `PoolReserveMissing`: on a
one-shot immutable setter, a wrong pool would otherwise leave the vault
permanently unusable.

**Persistence.** `deposit` and `withdraw` extend the lifetime of the contract
instance, its code and the caller's share entry to 120 days whenever less than
30 days remain. Without this, entries fall back on the network default of about
seven days and are archived silently; restoration is permissionless and loses
nothing, but it costs the person who triggers it roughly 240 times the nominal
resource fee. A compile-time assertion keeps the target under the network cap,
so raising the constants breaks the build instead of the network call.

**Errors.** Ten typed `#[contracterror]` codes rather than string panics, so an
off-chain integrator tests a number instead of matching a message.

### Structured events, the audit trail

Every `deposit` and every `withdraw` emits a structured Soroban event: typed
topics (the operation, the address) and a payload (amount, shares). These events
are indexed by the ledger, immutable and timestamped at the block, so every
movement of funds leaves a cryptographically verifiable trace with no dependency
on an off-chain database.

That is what the record-keeping obligations of a European operator call for:
reconstructing who deposited or withdrew what and when, from an on-chain source
of truth. A regulated operator wires its reporting straight onto the event
stream, with no manual reconciliation and no falsifiable ledger.

Two limits, stated plainly. `pause` and `unpause` do **not** emit events today;
an admin action is visible as an invocation on the ledger, not as a structured
event, and closing that gap belongs to the compliance event schema of
Deliverable 6a. The event shape is deliberately frozen in its current form until
that schema lands across all modules at once, which is why the vault still uses
the deprecated publishing API while the router already uses `#[contractevent]`.

### Build and test

```bash
rustup target add wasm32v1-none
cargo test --workspace     # 237 vault tests, 46 router tests
cargo llvm-cov --workspace --summary-only \
  --ignore-filename-regex '(^|/)test[^/]*\.rs$' --fail-under-lines 90
stellar contract build     # -> target/wasm32v1-none/release/yield_vault.wasm
```

283 tests pass with zero failures, and line coverage is 95.77% on the vault
contract, 96.05% across the workspace. The exclusion regex matters: Soroban
keeps test modules inside `src/`, so an unfiltered run counts test code as
covered code and inflates the figure by about four points.

The suite exercises the share arithmetic and the guards, integrates against the
**real Blend v2 wasm stack** (a real borrower with a one-year ledger jump, a
liquidity crunch at maximum utilisation, a frozen reserve, a supply-cap
rejection, each checked for atomic revert with no share loss), sweeps 200
oracle-generated matrix cases, and checks three property-based invariants over
256 random cases each: no free lunch, two-user solvency, share-price
monotonicity.

---

## 5. The SwapRouter contract (Tranche 2, already delivered)

Source: [`contracts/router/src/lib.rs`](../contracts/router/src/lib.rs).
Rebalancing between vault assets goes through a **separate** contract rather
than a swap module inside the vault: the vault stays free of any on-chain price
source, which matches how ForYield already values positions and selects venues
off-chain.

What the contract guarantees on-chain is a mandatory `min_out`, re-checked on
the way back and judged on the router's own balance delta rather than on what
the venue claims to have served: a venue that lies about what it served does
not fool the router. Quoting stays off-chain (`scripts/quote_aqua.sh`) and
calibrates `min_out`. Swap fees are accumulated per pair, which is the raw
material of the Deliverable 6c dashboard. Events are emitted in the target
`#[contractevent]` style.

Aquarius is the only venue. Soroswap was removed on 2026-08-28 after being
reported compromised, and no replacement was adopted: Phoenix, the obvious
candidate, restricts pool creation to accounts whitelisted by its factory, so
the USDC/EURC pair the router needs cannot be seeded there permissionlessly the
way it was on Soroswap and Aquarius.

The consequence is stated plainly rather than glossed over: the atomic fallback
is gone, and Aquarius is now a single point of failure. If its router is
unavailable, its pool empty, or the pool absent from the admin registry, the
swap fails and everything reverts. The failure is typed (`AquaPoolNotSet`,
`VenueFailed`) and the caller keeps the funds, but the swap does not happen.
The empty-pool case is not theoretical: third parties drained the EURC side of
our testnet Aquarius pool between July and August 2026.

What survives from the two-venue design is what protects funds rather than what
routed them: the min-out re-check, the balance-delta judgement, the atomic
revert, and the narrow, transaction-scoped pre-authorisation of the venue's
token pull. The venue was integrated against its real deployed bytecode, not a
mock.

---

## 6. Security model

| Topic | Tranche 1 state (testnet) | Production target |
|---|---|---|
| **Admin** | Single key fixed at `initialize`, `require_auth`. | **Multisig** (Stellar native multisig or smart account), M-of-N quorum, no solo key. |
| **Admin rotation** | None exposed; the admin is one-shot. | `set_admin` guarded by the multisig, plus a **timelock** on sensitive actions. |
| **Pause** | `pause` / `unpause` are admin-only; while paused, `deposit` and `withdraw` are rejected. | Same, triggerable by a subset of the multisig for speed, with `unpause` requiring the full quorum. |
| **Strategy exposure** | One Blend v2 pool, immutable, set at `initialize`. | Several allocators behind per-protocol caps and an emergency withdrawal path. |
| **Shares** | Non-transferable; ownership is an entry in contract storage. | SEP-41 transferable shares. |

**Accepted risks, documented in the contract header rather than discovered.**
The pool is immutable and there is no emergency divestment: `pause()` stops new
operations but does not repatriate funds already supplied to Blend, and if Blend
freezes the reserve, withdrawals fail atomically, with no shares lost, until it
thaws. The migration and divest path is Tranche 2 work. Slippage parameters were
deferred to Deliverable 4 in a dated decision: with share-price monotonicity
holding, Tranche 1 exposure is bounded by interest dust, and slippage only
becomes material once swaps enter the picture.

**Deployment front-run window.** The contract has no `__constructor`, so
`initialize` is a separate transaction from the deployment. The redeployment
script chains the two with nothing in between, which brings the window down to
roughly two ledgers, and the hardening above means an adversarial `initialize`
on a pool with no reserve for the asset now fails instead of bricking the
address. The window is narrowed, not closed; it is accepted on testnet and
recorded in the evidence log.

**Audit plan.** Invariant tests run in CI from the MVP onwards. A formal audit is
planned for **Tranche 3**, funded through a **Stellar LaunchKit credit** and not
taken from the SCF grant. No mainnet deployment before that audit closes.

---

## 7. Engineering process

Reviewer-facing claims are only worth the process that produces them.

- **Everything reaches `main` through a merged pull request.** Branch protection
  refuses direct pushes and requires five CI checks: Unit tests, Wasm build,
  Coverage, Web build, Onboarding tests.
- **The coverage gate is armed**, `--fail-under-lines 90` on every pull request,
  measured on production code only.
- **`docs/evidence/` holds one file per deliverable**, and every proof is
  recorded the day it is produced rather than reconstructed at submission time.
  Runs that went wrong are recorded too: an evidence log that keeps only the
  successful attempts teaches nobody anything.
- **Evidence instances are redeployed from `main`** with
  [`scripts/redeploy_vault.sh`](../scripts/redeploy_vault.sh), one profile per
  instance (`VAULT_PROFILE=d1|eurc|demo`). The same script is the runbook for
  the next SDF testnet reset.

---

## 8. Demo walkthrough (about 30 seconds)

1. **Connect Wallet** opens the Stellar Wallets Kit modal. Pick Freighter,
   xBull, Albedo or Lobstr on Testnet, and approve.
2. The card shows the address and the XLM balance read from Horizon. An
   unfunded account gets a **Fund with Friendbot** button, since a Stellar
   account only exists on-chain once funded.
3. Default amount **0.1 XLM**, then **Deposit**.
4. The wallet opens its signing prompt. Confirm.
5. The UI moves to a confirming state, then to **Deposit confirmed**, and shows
   **your vault position** (your share of the assets, and the vault's total
   assets) plus a **View on Stellar Expert** link.
6. On Stellar Expert: a Soroban `invoke contract function deposit` operation,
   and an XLM balance down by about 0.1 plus negligible network fees.

The demo covers connection, funding, deposit and position reading. Withdrawal is
exercised on every redeployment of the evidence instances, and its transaction
hashes are in `docs/evidence/`.

---

## 9. Testnet notes

Points settled while building, worth knowing before repeating them.

- **Freighter authorisation before signing.** The initial connect does not
  guarantee the domain is still in Freighter's allowlist at signing time
  (`getAddress` can return a cached key with no live authorisation). Access is
  requested again just before `signTransaction`, which is idempotent when
  already granted.
- **Protocol 23.** Testnet returns a `TransactionMeta` v4. The `stellar-sdk`
  must be at least version 14 to decode it, otherwise the result parsing fails
  with `Bad union switch: 4` while the transaction itself goes through on-chain.
- **Optional arguments on the CLI, version 27.** An `Option<Address>` is passed
  as JSON, so the value carries its own quotes: `--pool "\"C...\""`. The raw
  form, accepted by version 26, fails with `Invalid JSON in argument` and leaves
  a deployed contract with no admin, front-run window wide open. Absence is
  expressed by omitting the argument.
- **A deployed instance does not follow the repository.** It keeps the bytecode
  it was deployed with while `main` moves on. On 2026-08-04 two of the three
  instances were found running superseded code, one of them the public demo, and
  the pointer to that demo turned out to live in five places, one of which sits
  outside the repository in a local credential file. Check the deployed wasm
  hash, not the source.

---

## 10. Scope and roadmap

| Tranche | Content | State |
|---|---|---|
| **1, MVP** | Vault with proportional shares and Blend v2 allocation, wallet onboarding (Stellar Wallets Kit and DFNS), EURC SAC wrapper. | Delivered, evidenced on testnet. |
| **2, Testnet** | DEX routing (Aquarius), DeFindex allocator, performance-fee module with high-water mark, compliance event schema, dashboard. | DEX routing delivered ahead of schedule, then reduced to a single venue on 2026-08-28 after Soroswap was reported compromised; the rest in progress. |
| **3, Mainnet** | Formal audit, mainnet deployment, cross-chain onboarding, investor dashboard. | Not started. |

### Integration list, and what testnet actually showed

The vault is asset-agnostic and, beyond its single pool, strategy-neutral.
Tranche 2 adds an allocation layer behind the existing `deposit` and `withdraw`
interface, without touching the share ratio. Each integration sits behind a
per-protocol cap and an emergency withdrawal path.

- **Blend v2, lending.** Already wired in Tranche 1: the asset is supplied to a
  lending pool and accrued interest raises `total_assets`, therefore the value
  of every share. Integrated against the real Blend wasm stack, not a mock.
- **Aquarius, DEX routing.** The SwapRouter's only venue, used to convert
  between vault assets during a rebalance, with a bounded slippage floor. Its
  fee semantics (fee on the output, rounded up) are handled in the venue
  adapter rather than left to the caller.
- **Soroswap, removed.** Delivered in July 2026 as the primary venue, removed
  on 2026-08-28 after being reported compromised. A second venue remains
  desirable, and the search for one is open: it must allow permissionless pool
  creation for our pair, which is what ruled Phoenix out.
- **DeFindex, allocator.** Tranche 2. The open decision is whether ForYield
  routes through DeFindex vaults or calls the underlying strategies directly;
  our testnet survey found DeFindex with Blend strategies deployed but not the
  others we need, which is why this is scoped as a decision rather than a
  wiring task.

---

## 11. Links

- **Demo**: https://vault.for-yield.com
- **Deliverable 1 vault**: https://stellar.expert/explorer/testnet/contract/CCE5ITQQF4GWG5FA47D2XJBKXASWJ2E5V5AWW5U5BBAFWIXA77YYGWNI
- **Reviewer evidence**: [`docs/evidence/`](./evidence/)
- **Code**: `contracts/vault/` and `contracts/router/` (Rust and Soroban),
  `web/` (Next.js), `onboarding/` (TypeScript and DFNS)
- **Licence**: MIT, so any regulated EU operator may fork it and launch on
  Stellar.

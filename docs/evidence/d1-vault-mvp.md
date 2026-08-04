# D1 — Soroban YieldVault (USDC, Blend v2, 200+ tests)

## 2026-07-21 — Proportional share math + inflation protection

- **What it proves**: "mints proportional vault shares" (D1 Measure) — 1:1
  minting replaced by `shares = amount × total_shares / total_assets`,
  pro-rata withdrawals, 1,000 dead-share first-deposit inflation lock,
  insolvency and zero-rounding guards. Adversarially reviewed (1 critical
  + 3 warnings found and fixed before merge).
- **Pull request**: https://github.com/Foryield/soroban-yield-vault/pull/1
  (merged 2026-07-21, CI green: Unit tests / Wasm build / Coverage)
- **Tests**: 21 unit tests passing at merge.

## 2026-07-21 — Blend v2 allocation adapter

- **What it proves**: "supports a single initial allocation target (Blend v2
  USDC pool)" (D1 Measure) — deposits supplied to the pool, withdrawals
  served from it, `total_assets` valuing the position at bTokens × b_rate.
  Integration-tested against the real Blend WASM stack, including a real
  borrower + 1-year jump and atomic pool-failure modes. Adversarially
  reviewed (PASS WITH WARNINGS, both warnings addressed).
- **Pull request**: https://github.com/Foryield/soroban-yield-vault/pull/3
  (merged 2026-07-21, CI green).

## 2026-07-21 — Test campaign (231 tests, >90% coverage)

- **What it proves**: "full test suite passing (200+ tests), unit-tested
  coverage above 90 percent" (D1 Measure) — 231 tests (unit, Blend
  integration incl. max-util liquidity crunch, 200 oracle-generated matrix
  cases, 3 proptest properties x 256 random cases each), 92.4% line coverage
  on the contract source, 99.5% workspace-wide, CI gate
  `--fail-under-lines 90` active. Typed `VaultError` codes replace string
  panics.
- **Pull request**: https://github.com/Foryield/soroban-yield-vault/pull/5
- Slippage decision (2026-07-21): min-out parameters deferred to D4/Tranche 2
  (share-price monotonicity bounds D1 exposure to interest dust; slippage
  becomes material with swaps).

## 2026-07-21 — Testnet deployment on Blend USDC (evidence instance)

- **What it proves**: "contract deployed to Stellar testnet with verifiable
  address" + "deposit and withdraw transaction hashes on testnet"
  (D1 Measure + Reviewer evidence), against the real Blend v2 TestnetV2 pool.
- **Contract ID**: `CC3AEKESVOYLHAEBV3F3WOJP3JHF754ZEEXYG6XD3VQGI5YZEV2OEC6C`
  ([explorer](https://stellar.expert/explorer/testnet/contract/CC3AEKESVOYLHAEBV3F3WOJP3JHF754ZEEXYG6XD3VQGI5YZEV2OEC6C)),
  built from commit 7356136 (PR #5 branch).
- **Initialize** (asset = Blend testnet USDC SAC, pool = TestnetV2):
  [c637e8a8…d3e0](https://stellar.expert/explorer/testnet/tx/c637e8a8115d6a9243a7f2039c6006590202ce4777f37020264ab58aad63d3e0)
- **Deposit 100 USDC** (999,999,000 shares minted — 1,000 dead shares locked;
  funds supplied to Blend in the same transaction):
  [300820e4…bba8](https://stellar.expert/explorer/testnet/tx/300820e4a7afd0e09683c544de4f61b15b70e1972c02cc487a6c83daa7a7bba8)
- **Withdraw 400,000,000 shares** (399,999,999 units returned — truncation in
  the vault's favor; shortfall pulled back from Blend):
  [552812fc…e8a5](https://stellar.expert/explorer/testnet/tx/552812fc218831b8ef25a33e305e3cae5f123bfc590a86a904362643626ea8a5)
- Post-state read on-chain: `total_assets = 600000000` (60 USDC), Blend
  position `supply[3] = 568268900` bTokens, zero idle balance on the vault.
- The test USDC was borrowed from the TestnetV2 pool itself by the ops
  account (XLM collateral, USDC borrow) — no faucet dependency.

## 2026-08-03 — Re-verification against the Measure, current figures

Everything above was recorded the day it was produced. This entry states what
the repository proves today, so a reviewer re-running the commands gets the
numbers they read here.

- **Tests**: 283 workspace tests passing (237 vault, 46 router), up from 231 at
  the D1 test campaign.
- **Coverage**: 95.77% lines on `contracts/vault/src/lib.rs`, 96.05% workspace,
  test modules excluded from the measure. CI gate `--fail-under-lines 90`
  active on every pull request. Reproduce with:

  ```bash
  cargo llvm-cov --workspace --summary-only \
    --ignore-filename-regex '(^|/)test[^/]*\.rs$' --fail-under-lines 90
  ```

- **Hashes still resolve**: init, deposit and withdraw above re-queried on
  Horizon, all three `successful: true` (ledgers 3725768, 3725777, 3725780).
- **Deployed instance at the time of this entry**: `CC3AEKES…EC6C`, on-chain
  wasm hash `3a868b71…`, built from commit `7356136`. Superseded the same day,
  see the redeployment entry below.

Two hardening changes to the contract source, deployed the same day:

- **Durability**: `deposit` and `withdraw` extend the TTL of the contract
  instance, its code and the caller's share entry to 120 days whenever less than
  30 days remain. Without it, entries fall back on the network default of about
  seven days, after which the protocol restores them automatically at roughly
  240 times the nominal resource fee. A compile-time assertion keeps the target
  under the network cap of 3,110,400 ledgers.
- **Misconfiguration**: `initialize` reads the reserve for the deposit asset
  before accepting a Blend pool, and fails with `PoolReserveMissing` (#10) if
  there is none. The pool being immutable and `initialize` one-shot, a wrong
  pool used to leave the vault permanently unusable.

## 2026-08-03 — Evidence instance redeployed on the hardened contract

- **What it proves**: the whole D1 Measure again, on a contract whose on-chain
  wasm matches the repository. The previous instance was built from `7356136`,
  before the vault learned to extend its own TTL and to reject a Blend pool with
  no reserve for the deposit asset; publishing a contract whose code no longer
  matched the repository would have turned a ten-second check by a reviewer into
  a paragraph of explanation.
- **Contract ID**: `CCE5ITQQF4GWG5FA47D2XJBKXASWJ2E5V5AWW5U5BBAFWIXA77YYGWNI`
  ([explorer](https://stellar.expert/explorer/testnet/contract/CCE5ITQQF4GWG5FA47D2XJBKXASWJ2E5V5AWW5U5BBAFWIXA77YYGWNI)),
  on-chain wasm hash
  `5d5001e32dc23273dff3cc4aa4f10e7fe639fddabfab9d2ea9d9ed93dbb78bba`, built from
  `main` at `6c3dc88`. Same deposit asset and same Blend v2 TestnetV2 pool as
  before, both read from the canonical `blend-capital/blend-utils` registry at
  run time rather than hard-coded.
- **Deploy**:
  [ccfe40ee…9fcb4](https://stellar.expert/explorer/testnet/tx/ccfe40eefd64ba244978c4f7d9d059e51915c02bf7c026941d5d31bf0a19fcb4)
  (ledger 3954411)
- **Initialize**:
  [dec088c4…15c0](https://stellar.expert/explorer/testnet/tx/dec088c4952d2dbd42a87053e4a1662109f0e230fd98303f578a921b99d015c0)
  (ledger 3954850)
- **Deposit 100 USDC** (999,999,000 shares minted, 1,000 dead shares locked;
  supplied to Blend in the same transaction):
  [301cca6e…a705](https://stellar.expert/explorer/testnet/tx/301cca6e9a62aa6dd3f4349ae83e072bbafa51a00d9938cc7982c5dcd8aba705)
  (ledger 3954851)
- **Withdraw 399,999,600 shares** (399,999,599 units returned, truncation in the
  vault's favor; shortfall pulled back from Blend):
  [a57b0414…0770](https://stellar.expert/explorer/testnet/tx/a57b04147c4b7a99e9aa800a936085527fc89c6a327867673ba2ec028d4e0770)
  (ledger 3954852)
- **Post-state read on-chain**: `total_assets = 600000403` for
  `total_shares = 600000400`, zero idle balance on the vault. Assets already sit
  above shares: Blend interest accrues into the share price with no action from
  the vault.
- **TTL set by the contract itself, measured not assumed**: the instance entry
  and the holder's share entry both live until ledger 6,028,451, that is 120
  days out, written by `deposit` and `withdraw` themselves. A simulated deposit
  reports `ext: "v0"` (no archived entry) and a `minResourceFee` of 49,908
  stroops.
- **Front-run window**: `deploy` and `initialize` were 37 minutes apart on this
  run, a tooling failure between the two steps, instead of the intended few
  seconds. The contract was verified uninitialised by simulation before
  `initialize` was submitted. The window exists at all because the contract has
  no `__constructor`; it is accepted on testnet and recorded here.
- The predecessor instance `CC3AEKES…EC6C` stays online with its 1,000 dead
  shares and their backing. The July entries above remain accurate as dated
  records of that instance.
- Reproducible with `scripts/redeploy_vault.sh` (profile `d1`, the default),
  which is also the runbook for the next SDF testnet reset.

D1 status: all Measures met (verifiable testnet address, 283 tests passing,
95.77% coverage on the vault contract, merged PRs, deposit/withdraw hashes), on
an instance running the current published code. Remaining before closing the
deliverable: walkthrough/video packaging at reviewer submission.

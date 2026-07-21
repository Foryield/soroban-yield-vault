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

D1 status: all Measures met (verifiable testnet address, 231 tests passing,
92.4% coverage, merged PRs, deposit/withdraw hashes). Remaining before
closing the deliverable: walkthrough/video packaging at reviewer submission.

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

Still open for D1: Blend v2 USDC allocation adapter, redeploy on the Blend
testnet USDC SAC (with deposit/withdraw tx hashes), 200+ test campaign with
coverage > 90%.

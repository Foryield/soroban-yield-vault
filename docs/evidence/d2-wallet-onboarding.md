# D2 — Wallet onboarding (SWK production hardening + DFNS embedded)

## 2026-07-21 — SWK production hardening

- **What it proves**: "SWK signing live on mainnet config across the listed
  wallets" (D2 Measure, config side) — Ledger module wired into the kit
  modal, `NEXT_PUBLIC_STELLAR_NETWORK=testnet|mainnet` network switch
  (fail-closed on mainnet), persisted/restored wallet sessions,
  normalized signing/session error handling.
- **Pull request**: https://github.com/Foryield/soroban-yield-vault/pull/2
  (merged 2026-07-21, CI green: Unit tests / Wasm build / Coverage)

Still open for D2-SWK: per-wallet connection screenshots (Freighter, xBull,
Albedo, Lobstr, Ledger) recorded here. DFNS embedded onboarding is tracked
separately (walkthrough video at closure).

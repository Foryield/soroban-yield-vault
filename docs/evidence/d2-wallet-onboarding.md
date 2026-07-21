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

## 2026-07-21 — DFNS wallet signs a Soroban invocation (D2-DFNS opening spike)

- **What it proves**: the full signing chain for the embedded-wallet track —
  a DFNS-provisioned `StellarTestnet` wallet (MPC, no local key) signs and
  broadcasts a Soroban `InvokeHostFunction` transaction through the DFNS
  Broadcast Transaction API (`kind: Transaction`, hex-encoded envelope), and
  the vault credits the deposit. No Soroban-specific limitation on the DFNS side.
- **Wallet**: `wa-01ju1-vs7fs-ec6989kdio8bsm1u`
  (`GCUKCTOCRTLX52H2BWAA4EL5TE5PCECUSKFOG7BALI2TPFZRLIHJC5RS`, funded by Friendbot)
- **Transaction**: `deposit(from: GCUK…C5RS, amount: 1000000)` (0.1 XLM) on the
  demo vault `CCKW7NFKDCOTOVUODLJ6K734ZEYT4TZLQGLIVFZZR6DLUHO6UOTENWQ6` —
  hash [`d5047db5a17d98641cb4baa39c5842e1573389c6485f310143f05ea3aae325c9`](https://stellar.expert/explorer/testnet/tx/d5047db5a17d98641cb4baa39c5842e1573389c6485f310143f05ea3aae325c9),
  ledger 3728425, successful. `shares_of(GCUK…C5RS)` reads `1000000` after the call.
- **Method**: envelope built and simulated locally
  (`stellar contract invoke --build-only` piped into `stellar tx simulate`,
  auth via source-account credentials since invoker == tx source), then
  submitted unsigned to `POST /wallets/{walletId}/transactions`; DFNS returned
  `status: Broadcasted` with the tx hash in under a second.

Still open for D2-DFNS: onboarding path from an email/social login (no browser
extension, no seed phrase), a full deposit on the testnet vault from the
provisioned wallet, onboarding walkthrough video.

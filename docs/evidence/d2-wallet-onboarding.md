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

## 2026-07-21 — DFNS onboarding end to end: email in, confirmed deposit out

- **What it proves**: the D2 Measure ("a DFNS-provisioned wallet completing a
  deposit on testnet") through the packaged onboarding flow — a single
  `npm run onboard -- <email> <stroops>` provisions a fresh DFNS
  `StellarTestnet` wallet from an email identifier (no extension, no seed
  phrase), funds it, builds and simulates the Soroban deposit, broadcasts it
  through DFNS, and confirms inclusion on Horizon.
- **Wallet**: `wa-01ju3-b2a9o-e84rqvita01ljtbh`
  (`GASMKUYUXYLX4FOUB7IK2RQFPBHGUJDCGTMG56NNVHCR7SDEWPW6GKFI`, named from the
  demo email, funded by Friendbot)
- **Transaction**: `deposit` of 0.1 XLM on the demo vault
  `CCKW7NFKDCOTOVUODLJ6K734ZEYT4TZLQGLIVFZZR6DLUHO6UOTENWQ6` —
  hash [`733845a2a537a30efaef3f48c568a390b0cb7ae30cb29fb4eab570f9d6370b26`](https://stellar.expert/explorer/testnet/tx/733845a2a537a30efaef3f48c568a390b0cb7ae30cb29fb4eab570f9d6370b26),
  ledger 3730705, successful. `shares_of(GASM…GKFI)` reads `1000000` after the
  call.
- **Code**: the `onboarding/` package (provision / envelope / submit bricks +
  orchestrator + local demo page), delivered on this branch with 33
  credential-free unit tests and its own CI job.

Still open for D2-DFNS: onboarding walkthrough video (filmed on the local demo
page, `npm run demo`), recorded here at closure.

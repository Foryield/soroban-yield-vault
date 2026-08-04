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

## 2026-08-04 — Demo instance redeployed on the published contract

- **What it proves**: that the vault a reviewer actually deposits into, when
  following either wallet path, is the contract this repository publishes. Until
  today it was not. The instance behind vault.for-yield.com dated from June:
  its `initialize` took `(admin, asset)` with no pool, it exposed no typed
  errors, and it minted shares 1:1 — it predated the proportional share math,
  the first-deposit inflation lock and the Blend allocation. A reviewer
  exercising Deliverable 2 was depositing into a contract that did not do what
  Deliverable 1 claims. Found by fetching the deployed bytecode and comparing it
  to `main`, not by reading the code.
- **Contract ID**: `CCP3EJYJ55RLZYCHABIWCTCWRHQN2BYZVXLCHZLPCCKIKA4VNK6TMCHN`
  ([explorer](https://stellar.expert/explorer/testnet/contract/CCP3EJYJ55RLZYCHABIWCTCWRHQN2BYZVXLCHZLPCCKIKA4VNK6TMCHN)),
  on-chain wasm hash
  `5d5001e32dc23273dff3cc4aa4f10e7fe639fddabfab9d2ea9d9ed93dbb78bba`, built from
  `main` at `e6e34fc` — the same bytecode as the D1 and D3 instances. Deposit
  asset: native XLM through its SAC
  (`CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC`), `pool: None`, so
  any Friendbot-funded account can still deposit with no faucet.
- **Deploy**:
  [`d8e7c74a…bb32`](https://stellar.expert/explorer/testnet/tx/d8e7c74a7464b7e9d529b41b0732a2093783e8d5fe3aa11c54ec6e8f4e70bb32)
  (ledger 3965145)
- **Initialize**:
  [`9816d172…b7c8`](https://stellar.expert/explorer/testnet/tx/9816d1726c0b6eeeef6e503542414c84fa37297264bdc26db887623a87dfb7c8)
  (ledger 3965146)
- **Deposit 1 XLM** (9,999,000 shares minted, 1,000 dead shares locked — the
  proportional math the June instance did not have):
  [`92302abb…a1fa`](https://stellar.expert/explorer/testnet/tx/92302abbdd7dcb71b83f00eb92be14708ccaa76b270a258f3b64e279a50aa1fa)
  (ledger 3965147)
- **Withdraw 3,999,600 shares** (3,999,600 units returned):
  [`625a1351…ad4a`](https://stellar.expert/explorer/testnet/tx/625a13516adf9bf5151f5c3658129a81c23416e199d5079dce2ea106e681ad4a)
  (ledger 3965148)
- **Post-state read on chain**: `total_assets = 6000400` for
  `total_shares = 6000400`, all idle.
- **Frontend repointed**: `NEXT_PUBLIC_VAULT_ID` in `render.yaml` and the
  testnet default in `web/lib/stellar.ts`. Both had to move: the environment
  variable drives the deployed site, the default drives any local run without an
  environment, and leaving them apart is how a stale contract ID survives.
- Positions opened by third parties on the June instance stay on it; it remains
  online for them. That is the accepted cost of redeploying a public demo.
- Reproducible with `VAULT_PROFILE=demo scripts/redeploy_vault.sh`.

## 2026-08-04 — DFNS onboarding replayed against the redeployed demo vault

- **What it proves**: the D2 Measure on the *current* published contract. The
  July run proved the signing chain, but against the June instance; this one
  lands on `CCP3EJYJ…MCHN`, whose bytecode is the repository's.
- **Wallet**: `wa-01jv6-ctatu-e85q09ak5h87j0m8`
  (`GBWG3X6DEJLV3D33MITVYIHJ7AAM47PBRGG64BDGSY3XEW37JML3YAWV`), provisioned from
  an email address, funded by Friendbot.
- **Transaction**: `deposit` of 0.1 XLM —
  [`7594a12c…635c`](https://stellar.expert/explorer/testnet/tx/7594a12c7894ba5eb8395a07284e5fc9f9c5c0baca5e2204dcdc3668d242635c),
  ledger 3965499, successful. `shares_of(GBWG…3YAWV)` reads `1000000` after the
  call, and the contract address decoded from the transaction is
  `CCP3EJYJ55RLZYCHABIWCTCWRHQN2BYZVXLCHZLPCCKIKA4VNK6TMCHN`.
- **A first attempt landed on the old instance** (`e9472c9e…c203`, ledger
  3965435). It reported `successful: true` like any other run: the deposit was
  valid, only the target was stale. The pointer came from a local credential
  file outside the repository, which still carried `VAULT_CONTRACT_ID` for the
  June vault and won over the corrected default in `src/config.ts`. Two fixes
  followed, both in this commit: `VAULT_CONTRACT_ID` is gone from
  `.env.example`, so the code default is the single source; and the onboarding
  result now prints `vaultContractId`, so a run against the wrong vault is
  visible in its own output instead of requiring the transaction to be decoded.

Still open for D2-SWK: per-wallet connection screenshots, and the walkthrough
videos for both paths.

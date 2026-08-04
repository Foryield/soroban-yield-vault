# D3 — EURC SAC Wrapper Integration

## 2026-07-21 — EURC vault instance, deposit and redemption on testnet

- **What it proves**: "EURC deposit and redemption transactions on testnet,
  with the SAC wrapper invoked" (D3 Measure). EURC is a Classic Stellar
  asset; the vault holds it through its StellarAssetContract wrapper.
- **EURC SAC wrapper**: `CCUUDM434BMZMYWYDITHFXHDMIVTGGD6T2I5UKNX5BSLXLW7HVR4MCGZ`
  — deterministic SAC for Circle's official testnet EURC
  (`EURC:GB3Q6QDZYTHWT7E5PVS3W7FUT5GVAFC5KSZFFLPU25GO7VTC3NM2ZTVO`),
  deployed/derived via `stellar contract asset deploy`.
- **Contract ID (EURC vault)**: `CAA4MCRSKZ53KUE6L4SIWWRWRF3BGCSFKQKZJVEZSDPXTHYPGHUCMM7H`
  ([explorer](https://stellar.expert/explorer/testnet/contract/CAA4MCRSKZ53KUE6L4SIWWRWRF3BGCSFKQKZJVEZSDPXTHYPGHUCMM7H)),
  same wasm as the D1 instance, initialized with `pool: None`
  (pure holding vault — no EURC lending pool exists on testnet).
- **Initialize**:
  [f0355ce2…c021](https://stellar.expert/explorer/testnet/tx/f0355ce2543c6b1a16f31f60d3f7a9d4558c2b28b492b4c2012ae5006123c021)
- **Deposit 5 EURC** (49,999,000 shares minted — 1,000 dead shares locked;
  the SAC wrapper `transfer` moves the Classic asset into the contract):
  [91a64549…19d5](https://stellar.expert/explorer/testnet/tx/91a645497e7ad3370abfee3e982fdcf9fe176b777cefc7dadf5fad926c1919d5)
- **Redeem 20,000,000 shares → 2 EURC** (SAC wrapper emits the Classic
  asset back to the holder's trustline):
  [33aa3326…c986](https://stellar.expert/explorer/testnet/tx/33aa33269ec2943a03f3595e475aa92f10807858a849bb423195289e058cc986)
- Post-state read on-chain: vault `total_assets = 30000000` (3 EURC);
  holder trustline back to 17 EURC (20 faucet − 5 deposited + 2 redeemed).
- Test EURC obtained from Circle's official faucet (faucet.circle.com).

## 2026-08-04 — Instance redeployed on the published contract

- **What it proves**: the whole D3 Measure again, on an instance whose on-chain
  wasm matches the repository. The July instance was built before the vault
  learned to extend its own lifetime and to reject a Blend pool with no reserve
  (nine error codes instead of ten); a reviewer hashing its bytecode would have
  found code that no longer exists in `main`. Left alone, it would also have
  archived itself silently after about seven days.
- **Contract ID**: `CDZR2IY4V3GXUONLTVXJNCMTIR2LLFC55ZRPPEHCTI4RM7LVF25UKG5K`
  ([explorer](https://stellar.expert/explorer/testnet/contract/CDZR2IY4V3GXUONLTVXJNCMTIR2LLFC55ZRPPEHCTI4RM7LVF25UKG5K)),
  on-chain wasm hash
  `5d5001e32dc23273dff3cc4aa4f10e7fe639fddabfab9d2ea9d9ed93dbb78bba`, built from
  `main` at `e6e34fc` — the same bytecode as the D1 and demo instances. Same
  EURC SAC wrapper as before, `pool: None`.
- **Drain of the predecessor** (3.8797634 EURC returned to the ops account, the
  1,000 dead shares and their backing staying locked in the old instance
  forever, as designed):
  [`102be116…5945`](https://stellar.expert/explorer/testnet/tx/102be11641b2cbec7c045ae81936232f55b90dce271b4cbc7ed34425aa3b5945)
  (ledger 3965154)
- **Deploy**:
  [`aee03de6…2882`](https://stellar.expert/explorer/testnet/tx/aee03de63020e0773824cad37ed8c4592de0098e343b835b1832e33299652882)
  (ledger 3965156)
- **Initialize**:
  [`fd508914…3185`](https://stellar.expert/explorer/testnet/tx/fd5089140589bdce18eb933087b7827a99984f875eb0fa4c4cf7185d904d3185)
  (ledger 3965158)
- **Deposit 3 EURC** (29,999,000 shares minted, 1,000 dead shares locked; the
  SAC wrapper `transfer` moving the Classic asset into the contract):
  [`b1b64ade…54ca`](https://stellar.expert/explorer/testnet/tx/b1b64ade8867556ea1300d8b2b2e505e96c1b2ec8c0a2d2f30bb4ed2132a54ca)
  (ledger 3965159)
- **Redeem 11,999,600 shares into 1.19996 EURC** (SAC wrapper emitting the
  Classic asset back to the holder's trustline):
  [`abc06ea0…7c26`](https://stellar.expert/explorer/testnet/tx/abc06ea0843ce132461f9225a954d59a8e7617a3e5309a22a85f68af2baf7c26)
  (ledger 3965160)
- **Post-state read on chain**: `total_assets = 18000400` for
  `total_shares = 18000400`, all of it idle (no pool attached, so no strategy
  position to value).
- **Front-run window**: deploy and initialize were two ledgers apart, roughly
  ten seconds, against the 37 minutes of the D1 redeployment on 2026-08-03. The
  window is not closed — the contract has no `__constructor` — but the
  redeployment script now chains the two steps with nothing between them.
- The predecessor instance `CAA4MCRS…MM7H` stays online with its dead shares.
  The July entries above remain accurate as dated records of that instance.
- Reproducible with `VAULT_PROFILE=eurc scripts/redeploy_vault.sh`.

D3 status: Measure met (deposit + redemption with the SAC wrapper invoked,
verifiable contract ID), on an instance running the current published code.
Remaining: walkthrough video at reviewer submission.

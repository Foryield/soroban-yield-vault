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

D3 status: Measure met (deposit + redemption with the SAC wrapper invoked,
verifiable contract ID). Remaining: walkthrough video at reviewer submission.

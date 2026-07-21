# D4 — DEX Routing (Soroswap + Aquarius)

## 2026-07-21 — Soroswap pair seeded (groundwork)

- **What it proves**: the D1/D4 token chain has real testnet liquidity —
  a Soroswap pair pairing the Blend testnet USDC (the D1 vault asset) with
  the Circle EURC SAC (the D3 asset), created and seeded permissionlessly.
- **Pair contract**: `CAKF65K72WQ5N3LOSDX3GRLZQPN5D2MJVTXJBF4J3EZNLH4GTX4PKEIW`
  (token_a = Blend USDC `CAQCFVLOBK5GIULPNZRGATJJMIZL5BSP7X5YJVMGCPTUEPFM4AVSRCJU`,
  token_b = EURC SAC `CCUUDM434BMZMYWYDITHFXHDMIVTGGD6T2I5UKNX5BSLXLW7HVR4MCGZ`),
  reserves 16 USDC / 15 EURC, LP tokens minted to the ops account.
- **Seed transaction**:
  [5fa01cea97e2809a6c69c1aef988579ec6572691a87d90d9617c959c586c608d](https://stellar.expert/explorer/testnet/tx/5fa01cea97e2809a6c69c1aef988579ec6572691a87d90d9617c959c586c608d)
- **Reseed script** (testnet resets 2-4x/year): `scripts/seed_soroswap_pool.sh`
  — re-reads the Soroswap router and Blend USDC addresses from their
  canonical registries on every run, nothing hardcoded.

Still open for D4: swap routing layer in the vault (Soroswap aggregator
primary, Aquarius router fallback), slippage protection (min-out params,
decision of 2026-07-21), swap-fee accounting, best-execution selection,
USDC<->EURC rebalance demonstration + hashes + video.

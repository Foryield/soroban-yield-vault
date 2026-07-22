# ForYield Soroban YieldVault

Open-source Soroban smart contract for **ForYield**, the first French MiCA-regulated
DeFi yield vault on Stellar. This repository contains the core `YieldVault`
contract submitted for the Stellar Community Fund (SCF) Build Award.

> **Scope (Tranche 1 / Deliverable 1).** Asset deposit with proportional share
> minting (`shares = amount × total_shares / total_assets`, rounded in the vault's
> favor), pro-rata withdrawal, a 1,000 dead-share lock on the first deposit
> (first-depositor inflation protection, Uniswap V2 / DeFindex model), and an admin
> emergency pause. The vault is asset-agnostic - the deposit asset is set once at
> `initialize`, so USDC/EURC StellarAssetContracts plug in unchanged. In progress
> for Deliverable 1: Blend v2 USDC allocation. Multi-protocol allocation, DeFindex
> routing, the performance-fee module with high-water mark, and transferable SEP-41
> shares ship in Tranches 2 and 3.

## Testnet deployments

**Deliverable 1 instance — USDC, allocated to Blend v2** (the reviewer-evidence
instance):

| Component | Contract ID |
|---|---|
| YieldVault (D1) | `CC3AEKESVOYLHAEBV3F3WOJP3JHF754ZEEXYG6XD3VQGI5YZEV2OEC6C` |
| Deposit asset - Blend testnet USDC (SAC) | `CAQCFVLOBK5GIULPNZRGATJJMIZL5BSP7X5YJVMGCPTUEPFM4AVSRCJU` |
| Allocation target - Blend v2 TestnetV2 pool | `CCEBVDYM32YNYCVNRXQKDFFPISJJCV557CDZEIRBEE4NCV4KHPQ44HGF` |

Every deposit is supplied to the Blend pool in the same transaction; the vault
holds no idle assets. Evidence transactions (init, deposit, withdraw) are logged
in [docs/evidence/d1-vault-mvp.md](./docs/evidence/d1-vault-mvp.md).
[Explore the D1 vault](https://stellar.expert/explorer/testnet/contract/CC3AEKESVOYLHAEBV3F3WOJP3JHF754ZEEXYG6XD3VQGI5YZEV2OEC6C).

**Deliverable 3 instance — EURC via its SAC wrapper** (pure holding,
`pool: None`; Circle's official testnet EURC):

| Component | Contract ID |
|---|---|
| YieldVault (D3) | `CAA4MCRSKZ53KUE6L4SIWWRWRF3BGCSFKQKZJVEZSDPXTHYPGHUCMM7H` |
| Deposit asset - EURC SAC wrapper | `CCUUDM434BMZMYWYDITHFXHDMIVTGGD6T2I5UKNX5BSLXLW7HVR4MCGZ` |

Evidence transactions in [docs/evidence/d3-eurc-sac.md](./docs/evidence/d3-eurc-sac.md).

**Deliverable 4 instance — SwapRouter, DEX routing (Soroswap + Aquarius)**:

| Component | Contract ID |
|---|---|
| SwapRouter (D4) | `CC25CDFP3L65HHHTTFTEYOCXAVQRDVXGG7RWN7EGYB3JMWTTXB2PDAKK` |

Routes USDC<->EURC through the Soroswap aggregator (primary) with an atomic
Aquarius fallback, min-out slippage protection and per-pair swap-fee accounting.
Evidence (venue seeds, deployment, quotes, 3-hash rebalance, on-chain fallback
proof) in [docs/evidence/d4-dex-routing.md](./docs/evidence/d4-dex-routing.md).
[Explore the D4 router](https://stellar.expert/explorer/testnet/contract/CC25CDFP3L65HHHTTFTEYOCXAVQRDVXGG7RWN7EGYB3JMWTTXB2PDAKK).

**Demo instance — native XLM, no strategy** (behind vault.for-yield.com, so any
Friendbot-funded account can deposit with no faucet):

| Component | Contract ID |
|---|---|
| YieldVault (demo) | `CCKW7NFKDCOTOVUODLJ6K734ZEYT4TZLQGLIVFZZR6DLUHO6UOTENWQ6` |
| Deposit asset - native XLM (SAC) | `CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC` |

Network: Stellar **Testnet** (`Test SDF Network ; September 2015`).

**Deliverable 2 — wallet onboarding**: the [`onboarding/`](./onboarding/)
package provisions a Soroban-compatible wallet through the DFNS API from an
email identifier (no extension, no seed phrase) and completes a deposit on the
demo vault. Evidence in
[docs/evidence/d2-wallet-onboarding.md](./docs/evidence/d2-wallet-onboarding.md).

## Contract interface

| Function | Description |
|---|---|
| `initialize(admin, asset, pool)` | Set the admin, deposit asset and optional Blend pool (one-shot, immutable). |
| `deposit(from, amount) -> shares` | Pull `amount` of the asset and mint proportional shares. |
| `withdraw(from, shares) -> amount` | Burn shares and return the asset pro-rata. |
| `total_assets() -> i128` | Asset held by the vault (on-chain token balance). |
| `shares_of(owner) -> i128` | Shares held by an address. |
| `total_shares() -> i128` | Total shares issued. |
| `pause()` / `unpause()` | Admin-only emergency switch. |
| `is_paused() -> bool` | Pause state. |

Every deposit and withdrawal emits a structured Soroban event (`deposit` / `withdraw`)
for the AMF-compliant audit trail.

## Build & test

```bash
rustup target add wasm32v1-none
cargo test                 # unit tests (math invariants, pause, init guard)
stellar contract build     # -> target/wasm32v1-none/release/yield_vault.wasm
```

## Deploy (testnet)

```bash
stellar keys generate deployer --fund
stellar contract deploy \
  --wasm target/wasm32v1-none/release/yield_vault.wasm \
  --source deployer --network testnet
# Asset = native XLM SAC on testnet:
#   stellar contract id asset --asset native --network testnet
stellar contract invoke --id <VAULT_ID> --source deployer --network testnet \
  -- initialize --admin <ADMIN_G_ADDR> --asset <NATIVE_SAC_ID>
```

## Roadmap

- **Tranche 1 (MVP)** - this contract, wallet onboarding, EURC SAC wrapper.
- **Tranche 2 (Testnet)** - DEX routing (Soroswap + Aquarius), DeFindex allocator,
  performance-fee module + compliance event schema.
- **Tranche 3 (Mainnet)** - Certora audit, mainnet deployment, cross-chain onboarding,
  investor dashboard.

## License

[MIT](./LICENSE) - any regulated EU operator may fork and launch on Stellar.

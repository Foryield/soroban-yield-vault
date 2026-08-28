# ForYield Soroban YieldVault

Open-source Soroban smart contract for **ForYield**, a DeFi yield vault built for
EU regulatory requirements, on Stellar. This repository contains the core
`YieldVault` contract submitted for the Stellar Community Fund (SCF) Build Award.

ForYield is not an authorised crypto-asset service provider. Nothing here is an
offer of a financial service; the deployments below are testnet only.

> **Scope (Tranche 1 / Deliverable 1).** Asset deposit with proportional share
> minting (`shares = amount × total_shares / total_assets`, rounded in the vault's
> favor), pro-rata withdrawal, a 1,000 dead-share lock on the first deposit
> (first-depositor inflation protection, Uniswap V2 / DeFindex model), and an admin
> emergency pause. The vault is asset-agnostic - the deposit asset is set once at
> `initialize`, so USDC/EURC StellarAssetContracts plug in unchanged. Allocation to
> a single Blend v2 lending pool is delivered: the pool is set once at `initialize`
> and every deposit is supplied to it in the same transaction. Multi-protocol
> allocation, DeFindex routing, the performance-fee module with high-water mark,
> transferable SEP-41 shares, and the admin surface beyond the pause (key rotation,
> contract upgrade, emergency divest) ship in Tranches 2 and 3.

## Testnet deployments

**Deliverable 1 instance — USDC, allocated to Blend v2** (the reviewer-evidence
instance):

| Component | Contract ID |
|---|---|
| YieldVault (D1) | `CCE5ITQQF4GWG5FA47D2XJBKXASWJ2E5V5AWW5U5BBAFWIXA77YYGWNI` |
| Deposit asset - Blend testnet USDC (SAC) | `CAQCFVLOBK5GIULPNZRGATJJMIZL5BSP7X5YJVMGCPTUEPFM4AVSRCJU` |
| Allocation target - Blend v2 TestnetV2 pool | `CCEBVDYM32YNYCVNRXQKDFFPISJJCV557CDZEIRBEE4NCV4KHPQ44HGF` |

Every deposit is supplied to the Blend pool in the same transaction, so the vault
holds no idle assets beyond what a direct donation would leave. Evidence
transactions (deploy, init, deposit, withdraw) are logged in
[docs/evidence/d1-vault-mvp.md](./docs/evidence/d1-vault-mvp.md).
[Explore the D1 vault](https://stellar.expert/explorer/testnet/contract/CCE5ITQQF4GWG5FA47D2XJBKXASWJ2E5V5AWW5U5BBAFWIXA77YYGWNI).
The instance is redeployed from `main` with
[`scripts/redeploy_vault.sh`](./scripts/redeploy_vault.sh), which carries one
profile per instance (`VAULT_PROFILE=d1|eurc|demo`) and is also the runbook for
SDF testnet resets; the predecessor instance
`CC3AEKES…EC6C` stays online and its July evidence remains valid as a dated
record.

**Deliverable 3 instance — EURC via its SAC wrapper** (pure holding,
`pool: None`; Circle's official testnet EURC):

| Component | Contract ID |
|---|---|
| YieldVault (D3) | `CDZR2IY4V3GXUONLTVXJNCMTIR2LLFC55ZRPPEHCTI4RM7LVF25UKG5K` |
| Deposit asset - EURC SAC wrapper | `CCUUDM434BMZMYWYDITHFXHDMIVTGGD6T2I5UKNX5BSLXLW7HVR4MCGZ` |

Evidence transactions in [docs/evidence/d3-eurc-sac.md](./docs/evidence/d3-eurc-sac.md).

**Deliverable 4 instance — SwapRouter, DEX routing (Aquarius)**:

| Component | Contract ID |
|---|---|
| SwapRouter (D4) | redeployment pending, see below |

Routes USDC<->EURC through the Aquarius router, with min-out slippage
protection and per-pair swap-fee accounting.

The Soroswap venue was **removed on 2026-08-28** after Soroswap was reported
compromised. The router is now single-venue: the atomic fallback is gone, and
Aquarius is a single point of failure. That trade-off is deliberate and
documented, along with the reasons Phoenix was not adopted as a replacement, in
[the removal plan](./docs/plans/2026-08-28-retrait-soroswap-aquarius-seul.md).

The previous instance
[`CC25CDFP…DAKK`](https://stellar.expert/explorer/testnet/contract/CC25CDFP3L65HHHTTFTEYOCXAVQRDVXGG7RWN7EGYB3JMWTTXB2PDAKK)
is **deprecated and must not be called**: it still routes to the compromised
aggregator, and it cannot be neutralised on-chain (venues are immutable, there
is no pause). Its July evidence remains valid as a dated record.

Evidence in [docs/evidence/d4-dex-routing.md](./docs/evidence/d4-dex-routing.md).

**Demo instance — native XLM, no strategy** (behind vault.for-yield.com, so any
Friendbot-funded account can deposit with no faucet):

| Component | Contract ID |
|---|---|
| YieldVault (demo) | `CCP3EJYJ55RLZYCHABIWCTCWRHQN2BYZVXLCHZLPCCKIKA4VNK6TMCHN` |
| Deposit asset - native XLM (SAC) | `CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC` |

Network: Stellar **Testnet** (`Test SDF Network ; September 2015`).

All three YieldVault instances run the same published bytecode, wasm hash
`5d5001e32dc23273dff3cc4aa4f10e7fe639fddabfab9d2ea9d9ed93dbb78bba`, built from
`main`. They differ only by the asset they hold and by whether a Blend pool is
attached. Check any of them without trusting this file:

```bash
stellar contract fetch --id <VAULT_ID> --network testnet --out-file onchain.wasm
shasum -a 256 onchain.wasm
```

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
| `total_assets() -> i128` | Asset under management: idle token balance plus the Blend position valued at bTokens x b_rate. |
| `shares_of(owner) -> i128` | Shares held by an address. |
| `total_shares() -> i128` | Total shares issued. |
| `pause()` / `unpause()` | Admin-only emergency switch. |
| `is_paused() -> bool` | Pause state. |

Every deposit and withdrawal emits a structured Soroban event (`deposit` / `withdraw`),
so the audit trail an EU operator has to keep is reconstructible from the ledger.

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

`--pool` is optional and immutable once set. Omitted, as above, the vault is pure
custody with no strategy (the demo and D3 instances). The Deliverable 1 instance is
initialised with it, which is what routes every deposit to Blend:

```bash
stellar contract invoke --id <VAULT_ID> --source deployer --network testnet \
  -- initialize --admin <ADMIN_G_ADDR> \
  --asset CAQCFVLOBK5GIULPNZRGATJJMIZL5BSP7X5YJVMGCPTUEPFM4AVSRCJU \
  --pool CCEBVDYM32YNYCVNRXQKDFFPISJJCV557CDZEIRBEE4NCV4KHPQ44HGF
```

## Roadmap

- **Tranche 1 (MVP)** - this contract, wallet onboarding, EURC SAC wrapper.
- **Tranche 2 (Testnet)** - DEX routing (Aquarius), DeFindex allocator,
  performance-fee module + compliance event schema.
- **Tranche 3 (Mainnet)** - Certora audit, mainnet deployment, cross-chain onboarding,
  investor dashboard.

## License

[MIT](./LICENSE) - any regulated EU operator may fork and launch on Stellar.

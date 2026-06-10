# ForYield Soroban YieldVault

Open-source Soroban smart contract for **ForYield**, the first French MiCA-regulated
DeFi yield vault on Stellar. This repository contains the core `YieldVault`
contract submitted for the Stellar Community Fund (SCF) Build Award.

> **Scope (Tranche 1 / Deliverable 1 - MVP).** This contract is deliberately minimal:
> asset deposit (native XLM on the testnet demo), proportional share minting (1:1 until
> a strategy is wired), withdrawal, and an admin emergency pause. The vault is
> asset-agnostic - the deposit asset is set once at `initialize`, so USDC/EURC StellarAssetContracts
> plug in unchanged. Multi-protocol allocation (Blend v2, Aquarius), DeFindex routing,
> the performance-fee module with high-water mark, and transferable SEP-41 shares ship
> in Tranches 2 and 3.

## Testnet deployment

| Component | Contract ID |
|---|---|
| YieldVault | `CCKW7NFKDCOTOVUODLJ6K734ZEYT4TZLQGLIVFZZR6DLUHO6UOTENWQ6` |
| Deposit asset - native XLM (SAC) | `CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC` |

Network: Stellar **Testnet** (`Test SDF Network ; September 2015`).
The demo vault uses native XLM so any Friendbot-funded account can deposit with no faucet.
Explore on [Stellar Expert](https://stellar.expert/explorer/testnet/contract/CCKW7NFKDCOTOVUODLJ6K734ZEYT4TZLQGLIVFZZR6DLUHO6UOTENWQ6).

## Contract interface

| Function | Description |
|---|---|
| `initialize(admin, asset)` | Set the admin and the deposit asset (one-shot). |
| `deposit(from, amount) -> shares` | Pull `amount` of the asset and mint shares (1:1 at MVP). |
| `withdraw(from, shares) -> amount` | Burn shares and return the asset (1:1 at MVP). |
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

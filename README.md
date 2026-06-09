# ForYield Soroban YieldVault

Open-source Soroban smart contract for **ForYield**, the first French MiCA-regulated
DeFi yield vault on Stellar. This repository contains the core `YieldVault`
contract submitted for the Stellar Community Fund (SCF) Build Award.

> **Scope (Tranche 1 / Deliverable 1 - MVP).** This contract is deliberately minimal:
> USDC deposit, proportional share minting (1:1 until a strategy is wired), withdrawal,
> and an admin emergency pause. Multi-protocol allocation (Blend v2, Aquarius),
> DeFindex routing, the performance-fee module with high-water mark, transferable
> SEP-41 shares, and the EURC StellarAssetContract wrapper ship in Tranches 2 and 3.

## Testnet deployment

| Component | Contract ID |
|---|---|
| YieldVault | `CDPZCITOBYAO4SHLGMLDSK7Y7NFR4GWXCTSRKI6ZHMPHTCFVWCPADIHJ` |
| USDC (test StellarAssetContract) | `CAOVR32GS72FJKWOF3IM3SQJOBUHDKYRDMCHGVQMT742UM3LGWNO7O7G` |

Network: Stellar **Testnet** (`Test SDF Network ; September 2015`).
Explore on [Stellar Expert](https://stellar.expert/explorer/testnet/contract/CDPZCITOBYAO4SHLGMLDSK7Y7NFR4GWXCTSRKI6ZHMPHTCFVWCPADIHJ).

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
stellar contract invoke --id <VAULT_ID> --source deployer --network testnet \
  -- initialize --admin <ADMIN_G_ADDR> --asset <USDC_SAC_ID>
```

## Roadmap

- **Tranche 1 (MVP)** - this contract, wallet onboarding, EURC SAC wrapper.
- **Tranche 2 (Testnet)** - DEX routing (Soroswap + Aquarius), DeFindex allocator,
  performance-fee module + compliance event schema.
- **Tranche 3 (Mainnet)** - Certora audit, mainnet deployment, cross-chain onboarding,
  investor dashboard.

## License

[MIT](./LICENSE) - any regulated EU operator may fork and launch on Stellar.

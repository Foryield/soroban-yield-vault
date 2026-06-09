# ForYield x Stellar - Testnet Demo UI

Minimal Next.js front-end for the [ForYield Soroban YieldVault](https://github.com/foryield/soroban-yield-vault)
testnet demo (Stellar Community Fund Build). Connect a Stellar wallet, deposit USDC
into the Soroban vault, and view the transaction on Stellar Expert.

Standalone by design: no dependency on the ForYield production codebase. Disposable.

## Stack

- Next.js 14 (App Router)
- [`@creit.tech/stellar-wallets-kit`](https://github.com/Creit-Tech/Stellar-Wallets-Kit) - wallet connect (Freighter, xBull, Albedo...)
- [`@stellar/stellar-sdk`](https://github.com/stellar/js-stellar-sdk) - build / sign / submit the contract invocation

## Run locally

```bash
cp .env.example .env.local   # fill in the testnet IDs
npm install
npm run dev                  # http://localhost:3000
```

## Environment variables

| Variable | Meaning |
|---|---|
| `NEXT_PUBLIC_VAULT_ID` | YieldVault contract ID (testnet) |
| `NEXT_PUBLIC_USDC_SAC` | USDC StellarAssetContract ID |
| `NEXT_PUBLIC_USDC_ISSUER` | USDC classic asset issuer (for balance lookup) |
| `NEXT_PUBLIC_RPC_URL` | Soroban RPC endpoint |
| `NEXT_PUBLIC_HORIZON_URL` | Horizon endpoint |

## Flow

1. Connect wallet (Stellar Wallets Kit -> Freighter on Testnet).
2. The app reads the USDC balance from Horizon.
3. Deposit builds a `vault.deposit(from, amount)` invocation, prepares it via RPC,
   signs it with the wallet, submits it, and links to Stellar Expert.

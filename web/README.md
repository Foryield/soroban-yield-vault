# ForYield x Stellar - Testnet Demo UI

Next.js front-end for the [ForYield Soroban YieldVault](https://github.com/foryield/soroban-yield-vault)
testnet demo (Stellar Community Fund Build). Connect a Stellar wallet, deposit into
one of the vault instances, and follow what the vault does with the funds.

Standalone by design: no dependency on the ForYield production codebase. Disposable.
Exported as a static site (`output: "export"`): no server.

## Pages

- `/` - deposit and redeem, one tab per vault instance: XLM, EURC (the fee instance
  of deliverable 6b) and USDC, the multi-strategy allocator (deliverable 5). The USDC
  tab also shows the live allocation across Blend, Aquarius and the idle pocket,
  against the targets, and whether the drift trigger would let the keeper rebalance.
  The EURC tab shows the fee settings, the NAV per share against the high-water mark
  and what the treasury holds.
- `/dashboard/` - compliance dashboard (deliverable 6c): the AMF audit trail emitted
  by the allocator and the fee instance (deliverable 6a schema), with a record-number
  gap check, the fees charged (deliverable 6b),
  allocations per batch, NAV per share and a monthly statement, governance events,
  the decision-algorithm register, and CSV / JSON export.

## Where the audit trail comes from

The Soroban RPC keeps events for about 7 days. A daily GitHub Actions job
(`.github/workflows/ops-daily.yml`) copies them into one JSON archive per contract on
the `data/events` branch (`scripts/ingest-events.mjs`). The dashboard reads that
archive, then reads the RPC for anything newer, and refreshes every 15 seconds.

The same job publishes the daily `nav` event (`checkpoint`) and the Aquarius
strategy's reference price, signed by the keeper key `fy-ops-bot` (repository
secret `FY_OPS_BOT_SECRET`).

## Stack

- Next.js 14 (App Router), static export
- [`@creit.tech/stellar-wallets-kit`](https://github.com/Creit-Tech/Stellar-Wallets-Kit) - wallet connect (Freighter, xBull, Albedo, Lobstr, Ledger)
- [`@stellar/stellar-sdk`](https://github.com/stellar/js-stellar-sdk) - contract calls, simulation, events

## Run locally

```bash
cp .env.example .env.local   # optional on testnet: every value has a public default
npm install
npm run dev                  # http://localhost:3000
```

Ingest events into a local folder:

```bash
node scripts/ingest-events.mjs ./archive
```

## Environment variables

| Variable | Meaning |
|---|---|
| `NEXT_PUBLIC_STELLAR_NETWORK` | `testnet` (default) or `mainnet` |
| `NEXT_PUBLIC_VAULT_ID` | XLM vault instance |
| `NEXT_PUBLIC_EURC_VAULT_ID` | EURC vault instance |
| `NEXT_PUBLIC_USDC_VAULT_ID` | USDC multi-strategy vault (allocator) |
| `NEXT_PUBLIC_EVENTS_ARCHIVE_URL` | Base URL of the event archive (`<base>/<contract>.json`) |
| `NEXT_PUBLIC_RPC_URL` | Soroban RPC endpoint |
| `NEXT_PUBLIC_HORIZON_URL` | Horizon endpoint |

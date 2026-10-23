# D6: Performance fees and compliance audit trail

Three parts: the compliance event schema (D6a), the fee module (D6b) and the
compliance dashboard (D6c).

- D6a, event schema: `docs/plans/2026-10-20-d6a-schema-events-conformite.md`,
  answers of the project lead in its section 8. Implemented in the D5 vault
  ([PR #34](https://github.com/Foryield/soroban-yield-vault/pull/34)); its
  events are recorded in `d5-defindex-allocator.md`.
- D6b, fee module: `docs/plans/2026-10-23-d6b-frais.md` (design, security
  review in its section 12).
- D6c, dashboard: `docs/plans/2026-10-23-d6c-dashboard.md`,
  [PR #38](https://github.com/Foryield/soroban-yield-vault/pull/38).

## 2026-10-23: compliance dashboard live, event archive started (D6c)

- **What it proves**: the structured events are consumed by the compliance
  dashboard, and kept beyond the RPC's seven-day window.
- **Dashboard**: https://vault.for-yield.com/dashboard/?vault=usdc, reading
  the D5 vault `CBESWJ4CXLWEFU33HEEMJFT4SAYV5PW6LPEBAYRQBAPKIGRFSVBWXVOV` and
  its two strategies: audit trail with completeness check on `seq`,
  allocations by batch, NAV and monthly statement, governance, algorithm
  registry, CSV and JSON export.
- **Daily ops task** (`.github/workflows/ops-daily.yml`), first run by hand:
  [run 39972020429](https://github.com/Foryield/soroban-yield-vault/actions/runs/39972020429).
  It republished the Aquarius reference price, found the day's `nav` already
  published, and created the archive branch `data/events` with 12 events
  (vault seq 1 to 5 without a gap, no missed ledger).

## 2026-10-23: EURC fee instance deployed, both fee legs set (D6b)

- **What it proves**: a vault instance charges a performance fee above a
  high-water mark and a pro rata management fee, paid in shares minted to a
  treasury, every change recorded with its previous value.
- **Vault**: `CCXPS66KJHVMXRFHYXVPGOJKPH3LWBIAA4WVNV6BYJXFYKKEBCCLYVMP`,
  custody of Circle's testnet EURC, no strategy (no EURC lending pool exists
  on testnet). Wasm sha256
  `e6932f6a72cb77aa540b6611ad969d5367e0a61af5714d67d20b40c5ab9e944e`, built
  from commit `aeec166`.
- **Roles**: admin `GCC4ZBLBYJJD33WOX4EJKDRQSZJMTX7CGBFJWDUH4CDUX2CETUHWGCPG`,
  keeper `GCYKCA4FMM4GQKAIUQ2ZNOKY2B74XMWXN55MVMTE7YDQAHUJOSYXSTTR` (the
  daily ops key), treasury
  `GATESM23W2GOANZOWTSGSLR3HN4XN55JPY4NGMFX5ESDXJUA4QN7DLZL`, a dedicated
  testnet account.
- **Wasm upload** (15:19:07 UTC):
  [`c70ceb17…debe`](https://stellar.expert/explorer/testnet/tx/c70ceb1777a8d5a7f0ea4c82c0eb2bf11ab6fa47bb7b1e365ca05236829bdebe);
  **deployment with constructor** (15:19:12 UTC):
  [`41232278…b6ea`](https://stellar.expert/explorer/testnet/tx/4123227813fae6c0478cd95785a7a2c260dd75c2188e0d9265a77e1f3a88b6ea).
- **Fees set** by the admin, performance 20 %, management 2 % a year
  (15:19:17 UTC):
  [`a5c756e3…08cd`](https://stellar.expert/explorer/testnet/tx/a5c756e39a103f5e367108954e8b0cc9d7d9a06ecb2d459506822698654708cd).
  Events `config_set` (`perf_bps` 0 to 2000, seq 1), `config_set`
  (`mgmt_bps` 0 to 200, seq 2), `treasury_set` (none to the treasury, seq 3).
  Starting high-water mark 1.000000000 per share.

## 2026-10-23: performance fee charged and paid out (D6b)

- **What it proves**: the fee is accrued on a gain above the high-water mark,
  paid in shares, and the treasury redeems them like any holder.
- **Deposit** of 5 EURC by the ops account (15:19:32 UTC):
  [`ed58fb0a…1291`](https://stellar.expert/explorer/testnet/tx/ed58fb0a197678b2b078ed8d5d82a8b485b458fd516c6fbcb81e27a9d0811291).
  `deposit`, seq 4: 49,999,000 shares, 1,000 dead shares at genesis.
- **Simulated yield, stated as such**: the instance holds no strategy, so the
  ops account transferred 0.5 EURC straight to the vault (15:19:37 UTC), as
  interest would arrive:
  [`a711d9f6…a0a1`](https://stellar.expert/explorer/testnet/tx/a711d9f601f39e05231ba0e158a6afe3602b3fae51f087fb5a556b9a9897a0a1).
  NAV per share 1.0 to 1.1.
- **Daily checkpoint** by the keeper (15:19:47 UTC):
  [`1135c165…01a9`](https://stellar.expert/explorer/testnet/tx/1135c1653302242c62d5d0b6c9fd8751d39324d205c4390be29c7b73c16a01a9).
  `fee_minted`, kind `perf`, seq 5: gain 0.5 EURC, fee 0.1 EURC (20 %),
  925,925 shares to the treasury, high-water mark 1.000000000 to
  1.080000019 (the price after the fee), period 15:19:17 to 15:19:47 UTC.
  Then `nav`, seq 6: 5.5 EURC for 50,925,925 shares. The management leg minted
  nothing: 30 seconds at 2 % a year on 5.5 EURC is one unit, worth less than a
  share. Its clock did not move, so that time is charged at the next accrual.
- **Treasury redemption** of its 925,925 fee shares (15:20:07 UTC):
  [`520fae66…cfdd`](https://stellar.expert/explorer/testnet/tx/520fae66b1cb98a13db56d51cec5af4f73465fd12f4480b912c537c14ec5cfdd).
  `withdraw`, seq 7: 0.0999999 EURC received, the 0.1 EURC fee minus one unit
  of truncation, borne by the treasury and never by the holders.
- **Archive**: the seven transactions above are under `archive/d6/`.

### Still open

- The management leg on-chain, from the first daily checkpoint on this
  instance once the ops task covers it.
- Both legs in one transaction on-chain (proven in
  `contracts/vault/src/test_fees.rs`, `both_fees_are_split_in_one_transaction`).
- Dashboard screenshot and the video walkthrough.

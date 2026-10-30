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

## 2026-10-24 and 25: management fee charged by the daily task (D6b)

- **What it proves**: the management fee accrues pro rata temporis without
  any operator action: the scheduled ops task calls `checkpoint()`, and the
  fee is minted in shares to the treasury before the day's `nav`.
- **First scheduled run on this instance**, 2026-10-23 20:36 UTC
  ([run 40020519009](https://github.com/Foryield/soroban-yield-vault/actions/runs/40020519009)):
  the day's `nav` was already published at 15:19 (error 22, expected), and
  the archive ingested this instance's events seq 1 to 7.
- **Checkpoint of 2026-10-24** (19:42:42 UTC,
  [run 40126492347](https://github.com/Foryield/soroban-yield-vault/actions/runs/40126492347)):
  [`c8ad10d4…7fd8`](https://stellar.expert/explorer/testnet/tx/c8ad10d41bc60d3908812057484931fa0579248b9be5408038b3fe74d07b9fd8).
  `fee_minted`, kind `mgmt`, seq 8: period 2026-10-23 15:19:32 to
  2026-10-24 19:42:42 UTC (102,190 seconds), rate 2 % a year on 5.4000001
  EURC, fee 3,499 units (0.0003499 EURC), 3,240 shares to the treasury.
  The period starts at the deposit, not at the performance checkpoint
  15 seconds later: that one minted no management share and left the clock
  where it was. Then `nav`, seq 9: 5.4000001 EURC for 50,003,240 shares.
- **Checkpoint of 2026-10-25** (19:54:17 UTC,
  [run 40228196288](https://github.com/Foryield/soroban-yield-vault/actions/runs/40228196288)):
  [`0be985c0…9e3e`](https://stellar.expert/explorer/testnet/tx/0be985c0a40f3a2bead587c55eeccb2aed4f6aa8b59f68238d1de2a0d35a9e3e).
  `fee_minted`, kind `mgmt`, seq 10: period 2026-10-24 19:42:42 to
  2026-10-25 19:54:17 UTC (87,095 seconds), fee 2,982 units, 2,761 shares.
  Each period starts where the previous one ended: no second is charged
  twice. Then `nav`, seq 11: 50,006,001 shares.
- **Recomputed from the formula** in `accrue_fees`
  (`contracts/vault/src/lib.rs`): fee = assets x 200 x elapsed /
  (10,000 x 31,536,000), shares = fee x total shares / (assets - fee), both
  truncated. Both events match to the unit.
- **Effect on holders**: the assets stay at 5.4000001 EURC (no strategy, no
  yield since the 23rd), so the dilution lowers the NAV per share from
  1.0800000 to 1.0799300, then 1.0798704. The high-water mark stays at
  1.080000019: no performance fee is due until the NAV is back above it.
- **Dashboard**: https://vault.for-yield.com/dashboard/?vault=eurc, checked on
  2026-10-26: seq 1 to 11 without a gap, both `management` lines in the fee
  table, fees for October 0.1006481 EURC in the monthly statement (0.1
  performance plus both management charges), treasury holding 0.000648 EURC
  (its 6,001 remaining shares).
- **Archive**: both transactions are under `archive/d6/`.
- **Scheduling drift**: the task is set for 17:30 UTC and GitHub started it
  between 19:42 and 20:36 UTC on these three days. The ECB rate is published
  before either time, so the drift has no effect on the reference price.

### Still open

- ~~Both legs in one transaction on-chain~~: done on 2026-10-28, see below.
- Dashboard screenshot and the video walkthrough.

## 2026-10-28: both fee legs in one transaction, on-chain (D6b)

- **What it proves**: the atomic split. One transaction mints the management
  fee and the performance fee, each in its own `fee_minted` event, before the
  deposit that triggered the accrual. Until this date the split was proven in
  `test_fees.rs` only.
- **Simulated yield, stated as such**: 0.5 EURC transferred straight to the
  vault by the ops account (13:40:55 UTC), the instance having no strategy:
  [`55693e23…dd64`](https://stellar.expert/explorer/testnet/tx/55693e23950ad17afc34ebe62c4f16c894dd2c71e66f2709005e20d2241bdd64).
- **Deposit of 1 EURC by the tester wallet**
  `GBHFMYSEYXO42SCVOQ7GLJFR5CC2WVJKM5XED466GE2OKLL2DKJGH4ZX` (13:43:10 UTC):
  [`efb022c4…56bc`](https://stellar.expert/explorer/testnet/tx/efb022c4a9b6b9ca75ad78ee2a6acf6a685ed8bff1ff429647fe4f969c2c56bc).
  Events in this order:
  - `fee_minted`, kind `mgmt`, seq 16: rate 200 bps, fee 2,189 units, 1,855
    shares to the treasury, high-water mark unchanged at 1.080000019 ;
  - `fee_minted`, kind `perf`, seq 17: rate 2,000 bps, fee 997,081 units
    (0.0997081 EURC), 859,741 shares to the treasury, high-water mark
    1.080000019 to 1.159744981 ;
  - `deposit`, seq 18: 1 EURC for 8,622,585 shares, priced after both fees.
- **Next accruals**: deposit of 0.1558875 EURC by the tester (the EURC received
  from the D4 swap of the same day), management fee only, seq 19-20
  [`357835a0…e57d`](https://stellar.expert/explorer/testnet/tx/357835a0592ba8126610c0edb27bea4e46e2c626a90bc25574fa7b39ca6de57d) ;
  daily checkpoint by the keeper, management fee then `nav`, seq 21-22
  [`4ec96c69…8c55`](https://stellar.expert/explorer/testnet/tx/4ec96c69fc4eb18cf9a7db3500ced8b85ed43416e78a175ef96de9072b4e8c55).
  No performance fee: the NAV stays below the new high-water mark.

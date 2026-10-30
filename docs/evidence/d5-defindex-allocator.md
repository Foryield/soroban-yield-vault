# D5: Multi-protocol allocator (DeFindex strategies)

Design and decisions: `docs/plans/2026-10-20-d5-allocateur-conception.md`.
Code: [PR #34](https://github.com/Foryield/soroban-yield-vault/pull/34) (vault
and strategies), [PR #35](https://github.com/Foryield/soroban-yield-vault/pull/35)
(Aquarius deposit credited only for what it brings).

## 2026-10-21: aborted first attempt on Circle's testnet EURC

- **What it shows**: why the Aquarius leg does not pair USDC with Circle's
  EURC on testnet. Kept here so that the abandoned contracts below are
  accounted for.
- **Pool aligned on the ECB rate, then seeded** in the router's USDC/EURC pool
  `CDYPTHT6TO7IXXIYNTIMN6YYUGN35NE6Y2AXZJOUK3J2ORLKJS7LQDJV`:
  [`497a2c9e…35f2`](https://stellar.expert/explorer/testnet/tx/497a2c9e595700645c411117182d2a78f30e41052a3f0e7b4faa65fbde6435f2)
  (09:21:12 UTC),
  [`a02f439f…f8ae`](https://stellar.expert/explorer/testnet/tx/a02f439f7fe5810a53549a804ac5e5c00876d18e26f9c806535317311957f8ae)
  (09:21:37 UTC).
- **Blend strategy deployed and seeded** (`CCMOLCUAHYKJTEYT6W45DSCMXRBPT2EHNH5DNQ7P46X2Y6562OKCWC7D`,
  0.01 USDC):
  [`8d75e0f9…935e`](https://stellar.expert/explorer/testnet/tx/8d75e0f9b97063324697c08daa32e72da6332ad70094cce67b9ca170d044935e),
  [`387c90e4…0487`](https://stellar.expert/explorer/testnet/tx/387c90e45b12225087b9fb31c857c78f8d8a73f47cf693c3d3aecfc48d7a0487),
  [`1eda490a…ed7e`](https://stellar.expert/explorer/testnet/tx/1eda490a15e52369953de6a9a136b8cb3076f3aa9d00d81f3c0e16d89350ed7e).
- **Aquarius strategy deployed on the EURC pair, never seeded**
  (`CDMNEF5ZGBSUN6SNM72M56S2S5TYLZE2VLPK7B3HTRQFAMYCDUTZAWAR`):
  [`89e74b18…8434`](https://stellar.expert/explorer/testnet/tx/89e74b18b522305d80b14d529dc65a9dcd62cb2cb20540c10d59c49631428434),
  [`eb46bb48…5e43`](https://stellar.expert/explorer/testnet/tx/eb46bb487f86bfcf13468220cfd3738c112d01bdbeb1d6fe4049aac60fa55e43).
- **Third-party swap, 85 seconds after our seed**: account
  `GCKR2OXWN45UTZEPJQZYVFGP3FYFPSNMZ746ZVROVZ7CQPNRHT4OJP6C`, active on this
  pool since 2026-10-14 with about ten swaps a day, bought 12.73 EURC for
  69 USDC and moved the pool price to about 26 USDC per EURC:
  [`e0854b1a…e2c2`](https://stellar.expert/explorer/testnet/tx/e0854b1ae0595182f37598ffbea0329faaeaf81ecbf92e1290a2a16b9c26e2c2)
  (09:23:02 UTC).
- **Strategy guard held**: the Aquarius seed deposit failed in simulation on
  the slippage bound (1 %) against the ECB fair value. Nothing was submitted.
- **Our liquidity withdrawn**:
  [`36418bee…99d3`](https://stellar.expert/explorer/testnet/tx/36418bee7a40acadbd08e3b203064291d989b7276ea906e9f294e363d10499d3)
  (09:27:07 UTC).
- **Conclusion**: on a pool that others arbitrage towards an unrelated price,
  keeping it aligned on the ECB rate would be a standing subsidy. The two
  contracts above stay on-chain, unused, and are not part of the allocator.

## 2026-10-21: FYEUR, the test euro of the Aquarius leg

- **What it proves**: the Aquarius leg trades against a pool whose only price
  is ours, the ECB reference rate.
- **Trust assumption, stated**: FYEUR is a classic Stellar asset issued by our
  own key `GAGW52B7RDYCRYZXLT24GNIQFXP4YWFMREBK3NGOEWM4BOKBWDE56PLF`
  (`scripts/issue_test_eur.sh`). It is worth one euro by test convention only:
  we can issue more at will. It has no market outside our pool.
- **Asset**: `FYEUR:GAGW52B7RDYCRYZXLT24GNIQFXP4YWFMREBK3NGOEWM4BOKBWDE56PLF`,
  SAC `CDEU3YYGB2P5ZFDY7BJTLYAGPQEIQO5OPJCZUJNLACKKF2MFQYLGSTPP`.
- **SAC deployed**:
  [`89a9336b…0724`](https://stellar.expert/explorer/testnet/tx/89a9336b03a44bbe47ecac6d721d797e9635e84d73e595b0e1058f6cc8710724);
  **ops trustline**:
  [`5ae1b481…0976`](https://stellar.expert/explorer/testnet/tx/5ae1b48139d4499b2f14e0682bf2b565b58d527796070d047454a410eed10976);
  **1,000 FYEUR issued to the ops account**:
  [`b7ad84f8…9b18`](https://stellar.expert/explorer/testnet/tx/b7ad84f8bce215dac9f1323c47e5c2bd2ae4372a57976b3579b1898dcb709b18).

## 2026-10-21: USDC / FYEUR pool seeded at the ECB rate

- **What it proves**: the pool the Aquarius strategy values is priced at the
  ECB reference rate of the day (1.1234 USD per EUR).
- **Pool**: `CA2KS6SYWJNRSYSVSX5MO3MLCINHEXKW6FTCR3GGIAC6RQZ6R4OSPBU3`,
  constant product, 0.3 % fee, pool hash
  `9ac7a9cde23ac2ada11105eeaa42e43c2ea8332ca0aa8f41f58d7160274d718e`.
- **Created** (1 AQUA creation payment):
  [`7bee5403…9430`](https://stellar.expert/explorer/testnet/tx/7bee54035b1b631eec138f28000515fdd7f332509f0ef3a29c445c75c69d9430);
  **seeded with 112.34 USDC and 100 FYEUR**:
  [`e5883b14…edc7`](https://stellar.expert/explorer/testnet/tx/e5883b14f083d49789221a2ce6afa565faf12c416f3b5468aa7de94a8c44edc7).
- Post-state read on-chain: reserves `[1123400000, 1000000000]`.

## 2026-10-21: allocator deployed

- **What it proves**: the D5 vault and its two DeFindex-trait strategies are
  live on testnet, configured in the vault's deploy transaction.
- **Blend strategy** (DeFindex wasm, vendored, hash
  `5dcc905bbeddfc94046bfd26346146e08ef587c3bc061bfb1b12bd462b64a8fb`):
  `CCDZZ4NXLWFXWIJIFSOG37K3NIMITZPMUJAHLELRSHO7XO5VA5ZYDEV5`. Deploy
  [`25f87ebf…abd4`](https://stellar.expert/explorer/testnet/tx/25f87ebfdebea229c1459e9265b4c33c27305c6c61f674e8d4f469f2b24cabd4),
  seed deposit of 0.01 USDC from the ops account (the 1,000 dead shares come
  out of it, not out of the vault)
  [`3089b97e…ef58`](https://stellar.expert/explorer/testnet/tx/3089b97ef9801d81e56634677a7cdc85ebf96e7e620180fe6aa293f35aeeef58).
- **Aquarius strategy** (wasm hash
  `e6e387bcedccd60a051b9fb64ee0d097f44c69777bcdff3315f0950512b2fedc`):
  `CACZE525I6OY7ASL2UUSHCWY45QA4PXZ3BB3ZEUT6UUUYCFHDVQESEDF`, reference price
  1.1234 at deploy. Deploy
  [`5c32732e…052e`](https://stellar.expert/explorer/testnet/tx/5c32732e66019a11ecb2a89a51caf5706a5f5973bb452b63aa51854f88e4052e),
  seed deposit of 0.01 USDC, the first real LP deposit through the strategy
  into the deployed pool
  [`0983f977…7e7e`](https://stellar.expert/explorer/testnet/tx/0983f9779b7f4f15ca597039ee7899a55c1fe5f61eac6f4f79e61792c8387e7e).
- **Vault** (wasm hash
  `9364a31e8d0b0467690104f4c5bb66d62ca27d6fecdc6dfdf05d9059aa2a0b2e`):
  `CBESWJ4CXLWEFU33HEEMJFT4SAYV5PW6LPEBAYRQBAPKIGRFSVBWXVOV`. Wasm upload
  [`ca429f2f…e468`](https://stellar.expert/explorer/testnet/tx/ca429f2fee391a16d88c7b16ff48d45e052952126498b4d6427413e41617e468),
  deploy with constructor (targets Blend 70 %, Aquarius 20 %; caps 90 % and
  30 %; drift trigger 500 bps)
  [`e8b756b2…670e`](https://stellar.expert/explorer/testnet/tx/e8b756b22e589e6b3774b917d23ac9016fbb01d05f751d0d589d8bbd15d7670e).
- Wasm built from `main` at `5f0efc1`.
- **Simulated on the deployed contracts** (nothing submitted): strategy
  deposits and withdrawals, vault deposit. Instructions: Aquarius deposit
  21.8 M, withdrawal 21.2 M; Blend 4.0 M and 3.8 M; vault deposit 2.7 M. All
  within network limits, invocation memory included (a simulation over the
  limit fails).
- **Rent prepaid on first call**: each contract extends its instance and code
  to 120 days on its first call. The Aquarius seed deposit paid 527.44 XLM of
  rent for that, for a deposit that costs 0.004 XLM in resources. One-off,
  testnet XLM.

## 2026-10-23: first vault deposit

- **What it proves**: the D5 vault takes a deposit and mints shares; the
  funds sit idle until the first rebalance.
- **Deposit of 2 USDC** from the ops account, 100 % idle
  [`d42bc230…1a36`](https://stellar.expert/explorer/testnet/tx/d42bc23001b4ec2a12e13ca34c41a853c77e016f035110e1fea18fe7f02c1a36):
  19,999,000 shares to the depositor, 1,000 dead shares locked by the vault,
  `total_assets` 20,000,000, event `seq` 1.
- **Rent prepaid on first call**: 623.31 XLM charged, almost all of it
  refundable resource fee for the 120-day extension of the vault's instance
  and code. One-off, testnet XLM.
- **Simulated after the deposit** (nothing submitted):
  - `rebalance`: 1.4 USDC to Blend, 0.4 USDC to Aquarius, valued at
    0.3990275 USDC right after entry (0.24 % entry cost, swap fee and price
    impact on the 0.2 USDC swapped, under the 1 % bound); 35.4 M
    instructions.
  - `withdraw` of 0.15 USDC worth of shares, served from idle: 2.7 M
    instructions.
  - Both within network limits, invocation memory included (a simulation over
    the limit fails).
- **Reference price republished** at the ECB rate of 2026-10-22, 1.1205
  (was 1.1234), valid until 2026-10-26 09:10 UTC
  [`579303de…a8c5`](https://stellar.expert/explorer/testnet/tx/579303de0799d07b7cfeb28f58c03aab8e995d2ac14d17e3597f6569d33da8c5).
  The pool stays at 1.1234 until the next alignment, done right before the
  first Aquarius move.

## 2026-10-23: ops keeper, first daily NAV, first rebalance

- **What it proves**: the allocator spreads the vault across both strategies
  in one batch, each move recorded with its decision maker and algorithm; the
  keeper role sits on a dedicated key with no admin power.
- **Keeper handed to `fy-ops-bot`**
  (`GCYKCA4FMM4GQKAIUQ2ZNOKY2B74XMWXN55MVMTE7YDQAHUJOSYXSTTR`), the key of the
  daily ops job, on the vault
  [`e02e76e1…b4b0`](https://stellar.expert/explorer/testnet/tx/e02e76e1a9b7b044b015b0b391db58a8f9170fb7ebad2961fcb0f8c2aeb4b4b0)
  (`keeper_set`, seq 2, old and new keeper recorded) and on the Aquarius
  strategy
  [`a2d0510b…7f99`](https://stellar.expert/explorer/testnet/tx/a2d0510b3463a9abd32859f9602e454bac7756b62e0202cc2c236cddf95b7f99).
  The ops account stays admin.
- **First daily NAV** (`checkpoint`, unauthenticated):
  [`8edbd8f7…4971`](https://stellar.expert/explorer/testnet/tx/8edbd8f785f0d304ea907beba5ecd59a5079d5bacfdbf77ffdcb365c82194971),
  event `nav`, seq 3, 2 USDC for 2 shares.
- **Pool aligned** on the reference price (1.1205) before the first Aquarius
  move: 26 bps gap closed by a 0.1341784 FYEUR swap from the ops account
  [`9670d479…9fcd`](https://stellar.expert/explorer/testnet/tx/9670d479d32ff1df4f0723976b723f45c201c081307e8077df38bc0872379fcd).
- **First rebalance**, signed by the keeper
  [`e6ebfe6d…0c15`](https://stellar.expert/explorer/testnet/tx/e6ebfe6d91b159cc06c0728808cc24051dfb07948a455a64a301ff0ec42f0c15):
  two `allocate` events in one transaction, Blend +1.4 USDC (position
  1.3999999, seq 4) and Aquarius +0.4 USDC (position 0.3990449, seq 5), both
  `decided_by` the keeper, `algo` `fy_alloc_v1`, `reason` `drift`. Allocation
  after: 70.0 / 19.9 / 10.0 %, `total_assets` 1.9990448 USDC (0.24 % entry
  cost on the Aquarius leg, swap fee and price impact). Fee 0.50 XLM.

## 2026-10-28: walkthrough session, deposits, two batch rebalances, redemptions

- **What it proves**: the allocator under an outside depositor: the tester
  wallet `GBHFMYSEYXO42SCVOQ7GLJFR5CC2WVJKM5XED466GE2OKLL2DKJGH4ZX` deposits
  from the demo page, the keeper spreads the idle funds across both strategies
  in one transaction, and redemptions pull back from Blend when idle cash is
  short. Figures below are read from the decoded events (archive branch
  `data/events`).
- **Deposit of 1 USDC** by the tester, seq 10:
  [`3947da40…fc00`](https://stellar.expert/explorer/testnet/tx/3947da4035e9429ec35f93b38da1ccbf9253c23492e06a143272eb9a35a3fc00).
- **Rebalance** signed by the keeper, two `allocate` events in one
  transaction, Blend +0.6982939 USDC (seq 11) and Aquarius +0.2017821 USDC
  (seq 12), `algo` `fy_alloc_v1`, `reason` `drift`:
  [`f57dc94c…ddf2`](https://stellar.expert/explorer/testnet/tx/f57dc94c12411eea2cbaa7c1a1965688a220e2de4e67a89c65cad881591dddf2).
- **Second rebalance** after three small deposits by the tester, Blend
  +0.1887249 USDC (seq 16) and Aquarius +0.0543142 USDC (seq 17):
  [`b8cd7795…7f48`](https://stellar.expert/explorer/testnet/tx/b8cd77953586c695c50684e920ea982ec34ee370b8ea524ceae6225e145a7f48).
- **Redemption of 1 USDC** larger than the idle cash: `deallocate` from Blend
  of 0.823111 USDC, `reason` `redeem`, `decided_by` the redeeming holder
  (seq 19), then `withdraw` (seq 20):
  [`74b91f57…6dfe`](https://stellar.expert/explorer/testnet/tx/74b91f57e364436dfc94927634af170a3224d64c1a6f08a1382e5ee5311e6dfe).
- **Recorded sequence of the video**: redemption of 0.0999999 USDC pulled from
  Blend (seq 21-22)
  [`b8524799…6339`](https://stellar.expert/explorer/testnet/tx/b852479979efddb6483ce5da966d61944fa949489731ecdf80127ba2f35f6339),
  deposit of 1 USDC (seq 23)
  [`66832a82…5d0c`](https://stellar.expert/explorer/testnet/tx/66832a829f4e837a71d13baaba08aad23659a4dd55a3f34c140eac2c43115d0c),
  redemption of 0.2 USDC served from idle (seq 24)
  [`56ab01fa…214a`](https://stellar.expert/explorer/testnet/tx/56ab01fa85312b3950d0837418593d1c2f658f9358f5b1950bd9c895ccce214a).
- **Pool aligned** on the reference price before the rebalances by two
  FYEUR swaps from the ops account:
  [`ebed8bf9…b43c`](https://stellar.expert/explorer/testnet/tx/ebed8bf9ff3ee3ce0b1010f022b9f095ff4d3dcd13e4aa234eca570168fbb43c),
  [`2e0eba55…6471`](https://stellar.expert/explorer/testnet/tx/2e0eba559ac6378678a55bea685597c8902c46b92b3a4ea1f02e6fe0e27e6471).
- **Test USDC**: the ops account sent the tester 10 Blend testnet USDC
  [`13cc4f8b…15e1`](https://stellar.expert/explorer/testnet/tx/13cc4f8b658b04d48e60107461e6f834cd9ed1c6bf0bede4e98e08b7f94915e1);
  this vault takes Blend's test USDC, not Circle's.

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

## 2026-07-22 — Aquarius pool seeded

- **What it proves**: the second D4 venue is real — a standard
  (constant-product, 0.3% fee) USDC-Blend/EURC-Circle pool created and
  seeded on the DEPLOYED Aquarius testnet router, permissionlessly, by the
  ops account. Together with the Soroswap pair above, both venues of the
  SwapRouter now have on-chain liquidity for the same token pair.
- **Router address source**: `CBCFTQSPDBAIZ6R6PJQKSQWKNKWH2QIV3I4J72SHWBIK3ADRRAM5A6GD`
  comes from the S1 spike of 2026-07-21 (announced stable across testnet
  resets). No public JSON registry is known for Aquarius (the canonical
  `AquaToken/soroban-amm` repo is 404), so unlike the Soroswap/Blend
  addresses this one cannot be re-read from a canonical source at run
  time; the seed script exposes it as a documented `AQUA_ROUTER`
  environment variable instead.
- **Pool-creation payment finding**: the deployed router charges a
  creation fee that our local fixture configures to 0. Read on-chain
  before submitting anything (simulated getters):
  `get_standard_pool_payment_amount` = `10000000` (1.0, 7 decimals),
  `get_init_pool_payment_token` =
  `CDNVQW44C3HALYNVQ4SOBXY5EWYTGVYXX6JPESOLQDABJI5FC5LTRRUE`
  (SAC of `AQUA:GAHPYWLK6YRN7CVYZOO4H3VDRZ7PVF5UJGLZCSPAEIKJE2XSWF5LAGER`),
  paid to `get_init_pool_payment_address` =
  `CBIEL5HBXWXYNYFVULPFZU5OZLZCCCIZXCY3KUDRFX4OFANEJJPIBXGG`. The
  payment is only charged when the pool does not exist yet
  (mirror source `calc1f4r/soroban-amm@f9d4a5e0`, `contract.rs`,
  `init_standard_pool`: existing pool returned without payment).
- **Obtaining testnet AQUA**: the same router hosts XLM/AQUA pools, so
  the fee is self-serviceable — trustline to the AQUA asset, then
  `swap_chained` XLM->AQUA on the deepest pool (~21 AQUA per XLM):
  - trustline:
    [7e848bcfc9ec97ab0ff3a735de801523f74b78b683c50797dbc4033d5b158868](https://stellar.expert/explorer/testnet/tx/7e848bcfc9ec97ab0ff3a735de801523f74b78b683c50797dbc4033d5b158868)
  - swap 0.1 XLM -> 2.1027164 AQUA:
    [c3d0fe1f27f905da5a5c36b1398833f4898ffa663abeba83e8be3ccc96730f3f](https://stellar.expert/explorer/testnet/tx/c3d0fe1f27f905da5a5c36b1398833f4898ffa663abeba83e8be3ccc96730f3f)
- **Pool**: address `CDYPTHT6TO7IXXIYNTIMN6YYUGN35NE6Y2AXZJOUK3J2ORLKJS7LQDJV`,
  `pool_hash` (the `pool_index` returned by `init_standard_pool`, i.e.
  the value `set_aqua_pool` on our SwapRouter expects) =
  `9ac7a9cde23ac2ada11105eeaa42e43c2ea8332ca0aa8f41f58d7160274d718e`.
  Note this hash is the standard-pool salt of the 30 bps fee tier, so it
  is identical across pairs — it only identifies the pool within
  `get_pools(tokens)` for a given pair.
- **Seed transactions** (ops key `d1-ops`,
  `GCC4ZBLBYJJD33WOX4EJKDRQSZJMTX7CGBFJWDUH4CDUX2CETUHWGCPG`):
  - `init_standard_pool` (1.0 AQUA creation fee paid):
    [423d2ddbb5b1dbda7a5cb45f42a2631a213ef9cad32b7c999109aeccfd3e6384](https://stellar.expert/explorer/testnet/tx/423d2ddbb5b1dbda7a5cb45f42a2631a213ef9cad32b7c999109aeccfd3e6384)
  - `deposit` 1 USDC / 1 EURC, 1.0 LP shares minted to ops:
    [0108b658ce1ea916637928953079982e092349e0beef6047f0a5b542d1a065e2](https://stellar.expert/explorer/testnet/tx/0108b658ce1ea916637928953079982e092349e0beef6047f0a5b542d1a065e2)
- **Sizing (deliberate)**: reserves 1 USDC / 1 EURC, an order of
  magnitude below the Soroswap pair (16/15) — a real venue with visibly
  worse execution for a ~1 USDC swap, which is what the best-execution
  and fallback demos need. The plan's 2/2 target was cut to 1/1 because
  the ops account held only 2.0 EURC (project memory said ~17), and
  1.0 EURC plus ~123 USDC had to remain available for the upcoming vault
  deposits.
- **Verification** (`get_reserves` simulated read after seed, token
  order = sorted addresses, USDC first):
  `["10000000","10000000"]`.
- **Reseed script** (testnet resets 2-4x/year):
  `scripts/seed_aquarius_pool.sh` — token addresses re-read from their
  canonical sources, pool creation idempotent (re-run observed:
  "pool fee=30 deja existant, creation sautee", deposit replayed),
  simulation-first so the AQUA fee question is answered before any
  submission.

## 2026-07-22 — SwapRouter deployed to testnet

- **What it proves**: the D4 routing contract is live on testnet with a
  verifiable address, initialized against the two real venues (Soroswap
  aggregator, Aquarius router) and configured with the Aquarius pool
  seeded above.
- **Contract ID**: `CC25CDFP3L65HHHTTFTEYOCXAVQRDVXGG7RWN7EGYB3JMWTTXB2PDAKK`
  ([explorer](https://stellar.expert/explorer/testnet/contract/CC25CDFP3L65HHHTTFTEYOCXAVQRDVXGG7RWN7EGYB3JMWTTXB2PDAKK)),
  wasm hash `44b5c2c5517a21cb76fdd86923d00df5e15bdd0d51ca6bed7120895bdae920b2`,
  built from commit b2d7887. Admin = ops key `d1-ops`
  (`GCC4ZBLBYJJD33WOX4EJKDRQSZJMTX7CGBFJWDUH4CDUX2CETUHWGCPG`).
- **Transactions** (`scripts/deploy_swap_router.sh`, one run):
  - wasm upload:
    [965082deeffd3cf782264327f83894ed4c51a157b84764e439cb46cb8fd3ea2f](https://stellar.expert/explorer/testnet/tx/965082deeffd3cf782264327f83894ed4c51a157b84764e439cb46cb8fd3ea2f)
  - deploy (createContract):
    [e093d47cac83ee67d77e63eaab36b85efe4338a38ef0b7c297b323cf36c46077](https://stellar.expert/explorer/testnet/tx/e093d47cac83ee67d77e63eaab36b85efe4338a38ef0b7c297b323cf36c46077)
  - `initialize` (aggregator + Aqua router, fee bps 30/30 — Soroswap LP
    fee 0.3%, Aqua pool created at the 30 bps tier):
    [7a830ab14dd99d8b8317534cc21b29ac51d78de02b00c7351569172b1c8fc597](https://stellar.expert/explorer/testnet/tx/7a830ab14dd99d8b8317534cc21b29ac51d78de02b00c7351569172b1c8fc597)
  - `set_aqua_pool` (pool hash of 2026-07-22 seed, `aqua_pool_set` event
    emitted with the sorted pair, USDC first):
    [ad717b4a5488cec26b76187d6fea39fe2e48183d504d2afa60f4dcae629eb60f](https://stellar.expert/explorer/testnet/tx/ad717b4a5488cec26b76187d6fea39fe2e48183d504d2afa60f4dcae629eb60f)
- **Anti-front-run posture** (review follow-up, Task 2): `deploy` and
  `initialize` are chained back-to-back in the same script run — an
  adversarial `initialize` on the fresh contract would fix arbitrary
  venues, a more profitable target than the vault where the posture comes
  from. The stellar CLI only passes deployment arguments to
  `__constructor` contracts; ours exposes `initialize`, so the window
  cannot be closed to zero without a constructor refactor. Observed
  window this run: one ledger (deploy in 3743227, initialize in 3743228,
  5 seconds). Accepted for testnet; a mainnet deployment should move
  initialization into `__constructor` or deploy from a source with
  simulation pre-signed.
- **Aggregator address source (posture upgrade)**: a canonical registry
  DOES exist for the aggregator —
  `soroswap/aggregator` `public/testnet.contracts.json`, key
  `ids.aggregator` = `CC74XDT7UVLUZCELKBIYXFYIX6A6LGPWURJVUXGRPQO745RWX7WEURMA`
  (same address the S1 spike had found). The deploy script re-reads it at
  run time, same posture as the Soroswap router/Blend USDC addresses;
  `AQUA_ROUTER` remains an env default with spike provenance (no Aquarius
  registry known, unchanged).
- **Deployed aggregator pre-flight** (read-only `get_adapters`
  simulation, closing the deployed-vs-pinned gap left open in PR B): the
  deployed aggregator decodes cleanly with our replicated `Adapter` type
  and returns three adapters, all unpaused:
  `[{protocol_id: 0 (Soroswap), router: CCJUD55AG6W5HAI5LRVNKAE5WDP5XGZBUDS5WNTIVDU7O264UZZE7BRD},
  {protocol_id: 1 (Phoenix), router: CBAMPJTMNDXBBYYQ77C7WX2MTBR3XZHERDJ7NQMXLEZHMWUFQBQMFTOC},
  {protocol_id: 2 (Aqua), router: CBCFTQSPDBAIZ6R6PJQKSQWKNKWH2QIV3I4J72SHWBIK3ADRRAM5A6GD}]`.
  Two findings: (1) the Soroswap adapter's router equals the core
  registry router, and `router_pair_for(USDC, EURC)` on it returns
  exactly the pair seeded on 2026-07-21
  (`CAKF65K72WQ5N3LOSDX3GRLZQPN5D2MJVTXJBF4J3EZNLH4GTX4PKEIW`) — our
  `pull_auth_entries` discovery path works against the real deployment;
  (2) CONTRARY to the working assumption, the deployed aggregator DOES
  expose an Aqua adapter (protocol_id 2, the very router our fallback
  venue calls directly). The direct-Aquarius venue therefore is not the
  only path to Aqua liquidity; it remains justified as a fallback that
  does not depend on the aggregator being live, unpaused, and correctly
  adapted (single point of failure the fallback exists to route around),
  but the earlier "no Aqua adapter" justification is corrected here.
- **Post-deploy read-only verification** (all simulated, nothing
  submitted):
  - `aqua_pool_of(USDC, EURC)` =
    `9ac7a9cde23ac2ada11105eeaa42e43c2ea8332ca0aa8f41f58d7160274d718e`
    (the Task 13 seed hash);
  - `pair_stats(USDC, EURC)` =
    `{volume_in: 0, volume_out: 0, fees: 0, swaps: 0}`;
  - a second `initialize` fails in simulation with
    `Error(Contract, #1)` = `AlreadyInitialized` — the guard is proven
    without submitting anything.
- **TTL / archival ops note** (review follow-up, Task 6): the contract
  contains no `extend_ttl` calls (repo posture, same as the vault). The
  instance entries (admin, venues, fee bps, Aqua pool registry) live and
  archive together with the contract instance; the per-pair `Stats`
  entries are persistent with their own TTL. Ops must monitor
  liveUntilLedger on the instance and the stats keys
  (`stellar contract extend --id <contract> --ledgers-to-extend <n>` to
  push them forward). After archival, restoration is permissionless:
  `stellar contract restore --id CC25CDFP3L65HHHTTFTEYOCXAVQRDVXGG7RWN7EGYB3JMWTTXB2PDAKK --network testnet`
  (an archived persistent `Stats` entry requires
  `--key-xdr <base64 LedgerKey XDR>` plus `--durability persistent` —
  `--key` accepts symbol keys only, not tuple keys like
  `Stats(token_in, token_out)`). State
  is preserved across archival/restore; nothing is lost, only
  temporarily inaccessible.
- **Redeploy script** (testnet resets 2-4x/year):
  `scripts/deploy_swap_router.sh <key> <aqua_pool_hash>` — rebuilds the
  wasm, re-reads the aggregator and token addresses from their canonical
  registries, chains deploy + initialize, registers the Aqua pool hash
  printed by `seed_aquarius_pool.sh`, and prints the `aqua_pool_of`
  verification.

## 2026-07-22 — Best-execution quotes, rebalance and on-chain fallback proof

- **What it proves**: the complete D4 loop, live — both venues quoted
  off-chain by simulation, a USDC→EURC rebalance executed in 3 transaction
  hashes through the deployed SwapRouter (vault withdraw → routed swap →
  vault deposit), an on-chain fallback where the client asks for Aquarius
  and the contract serves via Soroswap (proven by the decoded `swap` event
  of the transaction itself), and `pair_stats` accounting that matches the
  two swaps to the unit. All amounts in 7-decimal units; every transaction
  simulated before submission; ops key `d1-ops`
  (`GCC4ZBLBYJJD33WOX4EJKDRQSZJMTX7CGBFJWDUH4CDUX2CETUHWGCPG`).
- **Quotes for 1.0 USDC (`10000000`), both venues, simulation only**
  (raw `--build-only` + `stellar tx simulate` outputs archived in the
  session workspace; `scripts/quote_venues.sh` reproduces them):
  - Soroswap, `router_get_amounts_out(10000000, [USDC, EURC])` on the
    canonical router: **8798611** out, against reserves
    `160000000/150000000` — constant-product with the 0.3% fee on the
    input amount.
  - Aquarius, `estimate_swap` on the deployed router (pool
    `9ac7a9cd…718e`): **4992488** out, against reserves
    `10000000/10000000`. Cross-checked with a full `swap_chained`
    simulation: identical output, `trade` event fee `15000`. A version
    nuance surfaced: the deployed pool's arithmetic matches the 0.3% fee
    applied on the INPUT (`floor(10^7·9970000/(10^7+9970000)) =
    4992488`), whereas the vendored-wasm fixture of PR B derived
    fee-on-output-with-ceil from the mirror sources (which would give
    `4985000`). The deployed testnet deployment differs from the pinned
    wasm on where the fee is applied (~0.15% here); immaterial for this
    campaign — both figures sit far below Soroswap — and the calibration
    below uses the deployed router's own numbers.
  - **Winner: Soroswap** (8798611 vs 4992488, +76%) — expected, its pool
    is an order of magnitude deeper (16/15 vs 1/1 by design, cf. the
    Aquarius seed section). `preferred = SoroswapAggregator` for the
    rebalance swap.
  - Ordering choice (documented per plan): the rebalance chain runs
    FIRST, against these fresh quotes; the fallback transaction runs
    after, with both venues re-quoted post-shift for its calibration.
- **Rebalance chain, 3 hashes** (step 0 unnecessary: simulated
  `shares_of(ops)` on the D1 USDC vault returned `599999000` — the
  evidence-instance shares of 2026-07-21 are still held, the withdraw is
  real):
  1. USDC vault `CC3AEKESVOYLHAEBV3F3WOJP3JHF754ZEEXYG6XD3VQGI5YZEV2OEC6C`
     `withdraw(ops, 10000000 shares)` → **10000029** USDC returned
     (share price slightly above par from accrued Blend interest:
     `total_assets 600001744 / total_shares 600000000`; simulation
     predicted the exact amount):
     [daa6fcd99ad02a5965faed59f22ddddba35afd1258706073f98bc17902e26963](https://stellar.expert/explorer/testnet/tx/daa6fcd99ad02a5965faed59f22ddddba35afd1258706073f98bc17902e26963)
  2. SwapRouter `swap_exact_in(ops, USDC, EURC, 10000029, min_out
     8710647, preferred=SoroswapAggregator)` — min_out = the re-quote for
     the exact withdrawn amount (`router_get_amounts_out(10000029)` =
     `8798634`) minus a 1% slippage margin. Served **8798634** EURC,
     effective venue Soroswap (`SwapResult {amount_out: 8798634, venue:
     0, fee: 30000}`, delivered output equal to the quote to the unit):
     [30afee289178c6962a793805ce4f19e568d2c940bbce3f55a9e7e852a056146b](https://stellar.expert/explorer/testnet/tx/30afee289178c6962a793805ce4f19e568d2c940bbce3f55a9e7e852a056146b)
  3. EURC vault `CAA4MCRSKZ53KUE6L4SIWWRWRF3BGCSFKQKZJVEZSDPXTHYPGHUCMM7H`
     `deposit(ops, 8798634)` → **8798634** shares minted (vault at par):
     [cb38a0ed8510f313d67d1d314b1bb7a5077e237ede747d1368b582952fdaf04f](https://stellar.expert/explorer/testnet/tx/cb38a0ed8510f313d67d1d314b1bb7a5077e237ede747d1368b582952fdaf04f)
- **Fallback proof**: post-rebalance re-quotes for 1.0 USDC — Soroswap
  **7822289** (shifted reserves `170000029/141201366`), Aquarius
  unchanged at **4992488**. `min_out` calibrated to **6000000**: 20.2%
  ABOVE what Aquarius can serve, 23.3% BELOW what Soroswap serves.
  `swap_exact_in(ops, USDC, EURC, 10000000, min_out 6000000,
  preferred=AquariusRouter)` succeeded:
  [d5b0e95f11c3cf01f0e9c51116f5dc3ecfa5162011be35cf889cdd73f67ffa56](https://stellar.expert/explorer/testnet/tx/d5b0e95f11c3cf01f0e9c51116f5dc3ecfa5162011be35cf889cdd73f67ffa56)
  (ledger 3743449). The `swap` event decoded from the transaction meta
  IS the proof — topics `[swap, ops, USDC, EURC]`, data:
  `{amount_in: 10000000, amount_out: 7822289, fee: 30000, min_out:
  6000000, preferred: 1 (AquariusRouter), venue: 0
  (SoroswapAggregator)}` — the on-chain record that the client asked for
  Aquarius and the contract fell back to Soroswap inside the same
  transaction. No Aquarius-side events appear in the final meta: the
  failed attempt's frame rolled back atomically, events included.
- **Preflight footprint finding** (ops note for any fallback
  transaction): the first two submissions of the fallback transaction
  failed on-chain with `ResourceLimitExceeded` DESPITE clean simulations
  ([fdfba43d…](https://stellar.expert/explorer/testnet/tx/fdfba43d8cee89aec0a69e2033000ded87fd238e6f68aa44432bed6e823ee785),
  [c84ea8ee…](https://stellar.expert/explorer/testnet/tx/c84ea8eec67b4a5fddacedc9d33afaf8ca7f3fef405daae425d1a1a276a75576)
  — the second with `--instruction-leeway`, ruling instructions out).
  Root cause: the RPC preflight records the entries touched by the
  rolled-back Aquarius attempt as READ-ONLY and omits part of its write
  set (pool token balances, pool instance, the Aqua router's outbound
  escrow balance) — but real execution performs those writes BEFORE the
  venue's trap, and the rollback happens after the footprint check, so
  the transaction dies on a footprint violation (surfaced as
  `ResourceLimitExceeded`). Fix applied: decode the simulated envelope,
  merge in the `read_write` footprint of a direct `swap_chained`
  simulation (the failing venue's true footprint), bump resources, sign
  and send manually — third submission succeeded. Any
  execute-then-fail preferred venue will hit this; budget the manual
  footprint merge (or submit the fallback path with a pre-widened
  footprint) in ops runbooks.
- **`pair_stats(USDC, EURC)` read on-chain after the campaign**:
  `{volume_in: 20000029, volume_out: 16620923, fees: 60000, swaps: 2}` —
  exactly `10000029 + 10000000`, `8798634 + 7822289`, `30000 + 30000`,
  and the two served swaps. The failed submissions left no trace in the
  stats (full revert), and the vault legs are invisible to the router by
  design: only served swaps are accounted.
- **Quote helper** (durable): `scripts/quote_venues.sh <key>
  <usdc_amount_7dp>` — quotes both venues by simulation only (Soroswap
  `router_get_amounts_out`, Aquarius `estimate_swap` across every pool
  `get_pools` returns for the pair), prints both outputs and the winner;
  addresses re-read from their canonical sources at run time, same
  posture as the seed scripts.

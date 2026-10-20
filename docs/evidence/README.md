# Reviewer evidence log

One file per SCF deliverable. Every proof is recorded **the day it is produced**,
not reconstructed at submission time.

| File | Deliverable |
|---|---|
| `d1-vault-mvp.md` | Soroban YieldVault (USDC, Blend v2, 200+ tests) |
| `d2-wallet-onboarding.md` | SWK multi-wallet + DFNS embedded wallet |
| `d3-eurc-sac.md` | EURC SAC wrapper |
| `d4-dex-routing.md` | Aquarius routing (Soroswap removed 2026-08-28) |
| `d5-defindex-allocator.md` | Multi-protocol allocator |
| `d6-fees-audit-trail.md` | Performance fees + compliance events |

Each entry records:

- **Date** (UTC)
- **What it proves** (one line, tied to the deliverable's Measure)
- **Transaction hash** + [Stellar Expert](https://stellar.expert/explorer/testnet) link, when on-chain
- **Contract ID**, when a (re)deployment
- **Pull request** link, when code
- **Media** (screenshot / video) path or URL, when visual

Nothing sensitive belongs here: testnet only, public addresses only.

## Offline archive

A testnet reset wipes the ledger and Stellar Expert testnet with it (next one
announced for 2026-12-16), which turns every hash above into a dead link. Each
proof transaction is therefore also archived the day it is produced, under
`archive/<deliverable>/<hash>/`: raw Horizon and Stellar Expert responses, the
decoded envelope, result and meta (events included), and a Stellar Expert
screenshot. [`archive/INDEX.md`](archive/INDEX.md) lists them all.

```bash
scripts/archive_proof.sh d5 <tx_hash>   # one new proof
scripts/archive_proof.sh --from-logs    # every tx link of the logs above
```

Before any proof that goes through Aquarius, `scripts/check_aquarius.sh <key>`
checks the venue against the state recorded on 2026-10-20 (see
`d4-dex-routing.md`).

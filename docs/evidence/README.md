# Reviewer evidence log

One file per SCF deliverable. Every proof is recorded **the day it is produced**,
not reconstructed at submission time.

| File | Deliverable |
|---|---|
| `d1-vault-mvp.md` | Soroban YieldVault (USDC, Blend v2, 200+ tests) |
| `d2-wallet-onboarding.md` | SWK multi-wallet + DFNS embedded wallet |
| `d3-eurc-sac.md` | EURC SAC wrapper |
| `d4-dex-routing.md` | Soroswap + Aquarius routing |
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

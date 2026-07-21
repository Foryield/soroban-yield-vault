# @foryield/stellar-onboarding

Deliverable 2 bricks: provision a Soroban-compatible wallet through the
**DFNS API** from an email identifier - no browser extension, no seed phrase -
and deposit into the testnet demo vault. **Testnet only.**

Three bricks, one orchestrator, one demo. Each brick is a standalone CLI that
prints a single JSON line to stdout (errors go to stderr).

## Setup

```bash
cp .env.example .env    # .env is gitignored
npm install
```

Fill the three `DFNS_*` values in `.env`: the service-account token
(`DFNS_AUTH_TOKEN`), the credential id (`DFNS_CRED_ID`), and the Ed25519
private key PEM (`DFNS_PRIVATE_KEY`). The Stellar testnet endpoints and the
demo vault contract id have public defaults - no change needed.

The CLIs and the demo load `.env` automatically from the package root;
environment variables already exported in your shell take precedence.

## Bricks

**Provision** - create a DFNS `StellarTestnet` wallet and fund it via Friendbot:

```bash
npm run provision -- <wallet-name>
# {"walletId":"wa-...","address":"G..."}
```

**Envelope** - build and simulate the Soroban `deposit` / `withdraw`
invocation. The `hex` field is what the DFNS broadcast API expects:

```bash
npm run envelope -- <deposit|withdraw> <G-address> <stroops>
# {"xdrBase64":"...","hex":"..."}
```

**Submit** - broadcast through DFNS (which signs with the wallet's key), then
wait for Horizon inclusion:

```bash
npm run submit -- <walletId> <hex>
# {"txHash":"...","ledger":123456,"successful":true}
```

**Onboard** - the full chain (provision → fund → envelope → submit → inclusion)
from a single email:

```bash
npm run onboard -- <email> <stroops>
# {"email":"...","walletId":"...","address":"G...","txHash":"...","ledger":123456,"successful":true}
```

**Demo** - a local-only walkthrough page:

```bash
npm run demo    # http://127.0.0.1:4600
```

The server binds loopback only; the browser talks to localhost and never sees
the DFNS credentials, which stay in the server process.

## Reuse seam

Each brick reads plain positional args and writes one JSON line to stdout,
errors to stderr - designed to be called as a subprocess from any backend,
whatever its language. The envelope brick is the piece with no equivalent
outside the JS Stellar SDK (transaction building + Soroban simulation); the
provision and submit bricks are thin DFNS API calls that a backend could also
reimplement natively.

## Notes

- Exit codes: `0` success, `1` error, `2` transaction included but failed
  on-chain (the JSON summary is still printed with `"successful":false`).
- The wallet name is the raw email in this demo. Do not copy that pattern into
  production custody wiring without treating the wallet name as PII.
- `npm test` (vitest) and `npx tsc --noEmit` run without any credentials.

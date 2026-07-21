#!/usr/bin/env bash
# Seed (re-seed) de la paire Soroswap testnet USDC-Blend / EURC-Circle.
#
# Rejouable apres chaque reset du testnet SDF (2-4x/an) : les adresses des
# contrats Soroswap et des tokens sont relues a chaque execution, jamais
# codees en dur. Prerequis : cle `stellar keys` financee detenant du USDC
# de test Blend et de l'EURC Circle testnet (cf. docs/evidence/).
#
# Usage : scripts/seed_soroswap_pool.sh <cle> <montant_usdc_7dp> <montant_eurc_7dp>
set -euo pipefail

KEY="${1:?usage: seed_soroswap_pool.sh <cle> <usdc_7dp> <eurc_7dp>}"
AMOUNT_USDC="${2:?montant USDC en unites 7 decimales}"
AMOUNT_EURC="${3:?montant EURC en unites 7 decimales}"
NETWORK=testnet

ADDR=$(stellar keys address "$KEY")

# Adresses relues aux sources canoniques.
ROUTER=$(curl -sf https://raw.githubusercontent.com/soroswap/core/main/public/testnet.contracts.json | python3 -c "import json,sys; d=json.load(sys.stdin); print(d.get('ids', d)['router'])")
USDC=$(curl -sf https://raw.githubusercontent.com/blend-capital/blend-utils/main/testnet.contracts.json | python3 -c "import json,sys; d=json.load(sys.stdin); print(d.get('ids', d)['USDC'])")
EURC=$(stellar contract id asset --asset EURC:GB3Q6QDZYTHWT7E5PVS3W7FUT5GVAFC5KSZFFLPU25GO7VTC3NM2ZTVO --network $NETWORK)

echo "router=$ROUTER usdc=$USDC eurc=$EURC to=$ADDR"

DEADLINE=$(( $(date +%s) + 3600 ))

# add_liquidity cree la paire si elle n'existe pas (semantique Uniswap V2).
# Mins a 90 % : premiere fourniture = prix libre, re-seed = tolerance de 10 %.
stellar contract invoke --id "$ROUTER" --source "$KEY" --network $NETWORK -- \
  add_liquidity \
  --token-a "$USDC" --token-b "$EURC" \
  --amount-a-desired "$AMOUNT_USDC" --amount-b-desired "$AMOUNT_EURC" \
  --amount-a-min $(( AMOUNT_USDC * 9 / 10 )) --amount-b-min $(( AMOUNT_EURC * 9 / 10 )) \
  --to "$ADDR" --deadline "$DEADLINE"

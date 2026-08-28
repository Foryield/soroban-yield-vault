#!/usr/bin/env bash
# Cotation de la venue D4 (Aquarius) pour USDC-Blend -> EURC, par SIMULATION
# UNIQUEMENT : aucune transaction soumise, aucun frais.
#
# Ce script CALIBRE min_out. Il ne choisit plus de venue : le routeur est
# mono-venue depuis le 28/08/2026 (retrait de Soroswap, cf.
# docs/plans/2026-08-28-retrait-soroswap-aquarius-seul.md), la
# best-execution n'a donc plus d'objet. Ce que le contrat garantit reste
# entier : le swap sert au moins min_out, ou tout revert. La marge de
# slippage se choisit ici, sur la cotation imprimee.
#
# Aquarius : estimate_swap du router, sur CHAQUE pool de la paire rendu par
# get_pools (la meilleure sortie gagne ; en pratique un seul pool standard
# 30 bps existe).
#
# Une sortie a 0 signale un pool vide ou inexistant : le swap echouerait en
# VenueFailed. Sans seconde venue pour prendre le relais, c'est un etat
# bloquant, a corriger par scripts/seed_aquarius_pool.sh avant toute demo.
#
# Usage : scripts/quote_aqua.sh <cle> <montant_usdc_7dp>
set -euo pipefail

KEY="${1:?usage: quote_aqua.sh <cle> <montant_usdc_7dp>}"
AMOUNT_IN="${2:?montant USDC en unites 7 decimales}"
NETWORK=testnet

# Router Aquarius : pas de registre public connu (cf. seed_aquarius_pool.sh),
# adresse du spike S1 21/07/2026, surcharger AQUA_ROUTER si elle change.
AQUA_ROUTER="${AQUA_ROUTER:-CBCFTQSPDBAIZ6R6PJQKSQWKNKWH2QIV3I4J72SHWBIK3ADRRAM5A6GD}"

# Adresses relues aux sources canoniques, jamais codees en dur.
USDC=$(curl -sf https://raw.githubusercontent.com/blend-capital/blend-utils/main/testnet.contracts.json | python3 -c "import json,sys; d=json.load(sys.stdin); print(d.get('ids', d)['USDC'])")
EURC=$(stellar contract id asset --asset EURC:GB3Q6QDZYTHWT7E5PVS3W7FUT5GVAFC5KSZFFLPU25GO7VTC3NM2ZTVO --network $NETWORK)

# Paire TRIEE par octets bruts d'adresse (exigence Aqua, cf.
# seed_aquarius_pool.sh : le base32 des strkeys n'est pas monotone en ASCII).
SORTED=$(python3 - "$USDC" "$EURC" <<'PY'
import base64, sys
def raw(strkey):
    return base64.b32decode(strkey)[1:-2]
a, b = sys.argv[1], sys.argv[2]
print("\n".join([a, b] if raw(a) < raw(b) else [b, a]))
PY
)
TOKENS_JSON="[\"$(echo "$SORTED" | head -1)\",\"$(echo "$SORTED" | tail -1)\"]"

simulate() {
  local id="$1"; shift
  stellar contract invoke --id "$id" --source "$KEY" --network $NETWORK --send=no -- "$@" 2>/dev/null
}

# Meilleure sortie parmi les pools de la paire (get_pools rend
# {pool_hash: adresse} ; estimate_swap cote chaque pool).
AQUA_OUT=0
AQUA_POOL=none
for POOL_HASH in $(simulate "$AQUA_ROUTER" get_pools --tokens "$TOKENS_JSON" \
  | python3 -c "import json,sys; print('\n'.join(json.load(sys.stdin).keys()))"); do
  OUT=$(simulate "$AQUA_ROUTER" estimate_swap --tokens "$TOKENS_JSON" \
    --token_in "$USDC" --token_out "$EURC" --pool_index "$POOL_HASH" \
    --in_amount "$AMOUNT_IN" | tr -d '"')
  if [ "$OUT" -gt "$AQUA_OUT" ]; then
    AQUA_OUT=$OUT AQUA_POOL=$POOL_HASH
  fi
done

echo "amount_in=$AMOUNT_IN (USDC -> EURC)"
echo "aquarius_out=$AQUA_OUT (router $AQUA_ROUTER pool $AQUA_POOL)"
if [ "$AQUA_OUT" -eq 0 ]; then
  echo "ATTENTION : aucune sortie cotee. Pool vide ou inexistant, le swap"
  echo "echouerait en VenueFailed. Re-seed : scripts/seed_aquarius_pool.sh"
  exit 1
fi
# Marge de slippage de 1 %, celle employee pour la demonstration testnet.
echo "min_out_1pct=$(( AQUA_OUT * 99 / 100 ))"

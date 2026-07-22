#!/usr/bin/env bash
# Cotation des deux venues D4 (Soroswap, Aquarius) pour USDC-Blend -> EURC,
# par SIMULATION UNIQUEMENT : aucune transaction soumise, aucun frais.
#
# La best-execution du SwapRouter est off-chain (le contrat garantit min-out
# et fallback atomique, pas la selection) : ce script est l'outil de
# selection. Il imprime la sortie cotee par chaque venue pour un montant
# d'entree donne et designe la meilleure ; `preferred` du swap_exact_in en
# decoule, min_out = cotation gagnante moins la marge de slippage choisie.
#
# Soroswap : router_get_amounts_out du router canonique (la venue cote
# elle-meme, memes maths que l'execution). Aquarius : estimate_swap du
# router, sur CHAQUE pool de la paire rendu par get_pools (la meilleure
# sortie gagne ; en pratique un seul pool standard 30 bps existe).
#
# Usage : scripts/quote_venues.sh <cle> <montant_usdc_7dp>
set -euo pipefail

KEY="${1:?usage: quote_venues.sh <cle> <montant_usdc_7dp>}"
AMOUNT_IN="${2:?montant USDC en unites 7 decimales}"
NETWORK=testnet

# Router Aquarius : pas de registre public connu (cf. seed_aquarius_pool.sh),
# adresse du spike S1 21/07/2026, surcharger AQUA_ROUTER si elle change.
AQUA_ROUTER="${AQUA_ROUTER:-CBCFTQSPDBAIZ6R6PJQKSQWKNKWH2QIV3I4J72SHWBIK3ADRRAM5A6GD}"

# Adresses relues aux sources canoniques, jamais codees en dur.
SOROSWAP_ROUTER=$(curl -sf https://raw.githubusercontent.com/soroswap/core/main/public/testnet.contracts.json | python3 -c "import json,sys; d=json.load(sys.stdin); print(d.get('ids', d)['router'])")
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

# Soroswap : le router cote [amount_in, amount_out] sur le chemin direct.
SOROSWAP_OUT=$(simulate "$SOROSWAP_ROUTER" router_get_amounts_out \
  --amount_in "$AMOUNT_IN" --path "[\"$USDC\",\"$EURC\"]" \
  | python3 -c "import json,sys; print(json.load(sys.stdin)[1])")

# Aquarius : meilleure sortie parmi les pools de la paire (get_pools rend
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
echo "soroswap_out=$SOROSWAP_OUT (router $SOROSWAP_ROUTER)"
echo "aquarius_out=$AQUA_OUT (router $AQUA_ROUTER pool $AQUA_POOL)"
if [ "$SOROSWAP_OUT" -ge "$AQUA_OUT" ]; then
  echo "best=SoroswapAggregator (preferred=0)"
else
  echo "best=AquariusRouter (preferred=1)"
fi

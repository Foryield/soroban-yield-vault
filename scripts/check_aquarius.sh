#!/usr/bin/env bash
# Controle de la venue Aquarius avant toute preuve testnet (tournage, swap de
# demonstration, allocation D5). SIMULATION UNIQUEMENT : rien n'est soumis.
#
# Pourquoi : Aquarius est la seule venue du projet depuis le 28/08/2026, son
# code source n'est plus public, et ses contrats sont modifiables par leur
# admin (commit_upgrade / apply_upgrade, set_emergency_mode). Ce script
# compare l'etat en ligne a celui consigne dans docs/evidence/d4-dex-routing.md
# (entree du 2026-10-20) et refuse de laisser partir une preuve sur une venue
# qui a change ou un pool vide :
#   1. empreinte SHA-256 du wasm du routeur et de notre pool ;
#   2. mode d'urgence du routeur et du pool (actif au releve du 20/10, effet
#      non documente) et interrupteurs swap / deposit du pool ;
#   3. reserves du pool au-dessus d'un plancher (le pool est vide
#      regulierement par des tiers, cf. seed_aquarius_pool.sh) ;
#   4. pool enregistre dans notre SwapRouter = pool rendu par Aquarius.
#
# Usage : scripts/check_aquarius.sh <cle>
# Code de sortie : 0 si tout est conforme, 1 sinon (le detail est imprime).
#
# Variables d'environnement :
#   EXPECTED_ROUTER_WASM / EXPECTED_POOL_WASM  empreintes attendues ; a mettre
#       a jour, avec une entree datee dans le journal D4, si Aquarius publie
#       une nouvelle version
#   EXPECTED_EMERGENCY  valeur attendue du mode d'urgence (defaut true)
#   MIN_RESERVE         plancher par reserve, unites 7 decimales (defaut 1.0)
set -euo pipefail

KEY="${1:?usage: check_aquarius.sh <cle>}"
NETWORK=testnet

# Adresse publiee par Aquarius (docs.aqua.network, Addresses & Networks),
# annoncee stable aux resets du testnet.
AQUA_ROUTER="${AQUA_ROUTER:-CBCFTQSPDBAIZ6R6PJQKSQWKNKWH2QIV3I4J72SHWBIK3ADRRAM5A6GD}"
# Notre SwapRouter mono-venue (deploiement du 28/08/2026).
SWAP_ROUTER="${SWAP_ROUTER:-CCQJWT73HTZUVLM2UUPUA5VR53Z5MTHCRZDVF5RODH3ORMVALNQQY6EA}"
EXPECTED_ROUTER_WASM="${EXPECTED_ROUTER_WASM:-c99539b023df4a28d7857d33ca4c915c2e20fec04f12b125132b84fbdf94dad3}"
EXPECTED_POOL_WASM="${EXPECTED_POOL_WASM:-d691135aade93ff0f7c229e009cde042130a05124cf7202b03d11246b4f9b473}"
EXPECTED_EMERGENCY="${EXPECTED_EMERGENCY:-true}"
MIN_RESERVE="${MIN_RESERVE:-10000000}"

USDC=$(curl -sf https://raw.githubusercontent.com/blend-capital/blend-utils/main/testnet.contracts.json | python3 -c "import json,sys; d=json.load(sys.stdin); print(d.get('ids', d)['USDC'])")
EURC=$(stellar contract id asset --asset EURC:GB3Q6QDZYTHWT7E5PVS3W7FUT5GVAFC5KSZFFLPU25GO7VTC3NM2ZTVO --network $NETWORK)

# Paire triee par octets bruts d'adresse (exigence Aqua, cf. quote_aqua.sh).
TOKENS_JSON=$(python3 - "$USDC" "$EURC" <<'PY'
import base64, json, sys
raw = lambda s: base64.b32decode(s)[1:-2]
a, b = sys.argv[1], sys.argv[2]
print(json.dumps([a, b] if raw(a) < raw(b) else [b, a]))
PY
)

simulate() {
  local id="$1"; shift
  stellar contract invoke --id "$id" --source "$KEY" --network $NETWORK --send=no -- "$@" 2>/dev/null
}

FAILURES=0
check() {
  local label="$1" got="$2" want="$3"
  if [ "$got" = "$want" ]; then
    echo "OK     $label : $got"
  else
    echo "ECART  $label : $got (attendu $want)"
    FAILURES=$((FAILURES + 1))
  fi
}

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
wasm_sha() {
  stellar contract fetch --id "$1" --network $NETWORK -o "$TMP/$1.wasm" 2>/dev/null
  shasum -a 256 "$TMP/$1.wasm" | cut -d' ' -f1
}

POOLS=$(simulate "$AQUA_ROUTER" get_pools --tokens "$TOKENS_JSON")
read -r POOL_HASH POOL < <(echo "$POOLS" | python3 -c "import json,sys; d=json.load(sys.stdin); k=next(iter(d)); print(k, d[k])")
echo "pool Aquarius USDC/EURC : $POOL (hash $POOL_HASH)"

check "wasm routeur Aquarius" "$(wasm_sha "$AQUA_ROUTER")" "$EXPECTED_ROUTER_WASM"
check "wasm pool" "$(wasm_sha "$POOL")" "$EXPECTED_POOL_WASM"
check "mode d'urgence routeur" "$(simulate "$AQUA_ROUTER" get_emergency_mode)" "$EXPECTED_EMERGENCY"
check "mode d'urgence pool" "$(simulate "$POOL" get_emergency_mode)" "$EXPECTED_EMERGENCY"
check "swap coupe (pool)" "$(simulate "$POOL" get_is_killed_swap)" false
check "deposit coupe (pool)" "$(simulate "$POOL" get_is_killed_deposit)" false
check "pool au registre du SwapRouter" \
  "$(simulate "$SWAP_ROUTER" aqua_pool_of --token_a "$USDC" --token_b "$EURC" | tr -d '"')" "$POOL_HASH"

# Reserves dans l'ordre trie de la paire.
read -r R0 R1 < <(simulate "$POOL" get_reserves | python3 -c "import json,sys; print(*json.load(sys.stdin))")
if [ "$R0" -ge "$MIN_RESERVE" ] && [ "$R1" -ge "$MIN_RESERVE" ]; then
  echo "OK     reserves : $R0 / $R1"
else
  echo "ECART  reserves : $R0 / $R1, sous le plancher $MIN_RESERVE ; re-seed : scripts/seed_aquarius_pool.sh"
  FAILURES=$((FAILURES + 1))
fi

if [ "$FAILURES" -gt 0 ]; then
  echo "$FAILURES ecart(s) : ne pas produire de preuve avant de les avoir traites."
  exit 1
fi
echo "venue conforme au releve du 2026-10-20"

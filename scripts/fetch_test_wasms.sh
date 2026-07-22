#!/usr/bin/env bash
# Telechargement des wasm de venues (Soroswap, Aqua) pour les tests
# d'integration du routeur (contractimport! dans contracts/router).
#
# Source : depot soroswap/aggregator a COMMIT EPINGLE (le depot canonique
# Aqua est en 404, cf. contracts/router/test_wasms/README.md). Rejouable :
# chaque fichier est re-telecharge par-dessus l'existant, puis l'integrite
# est verifiee contre contracts/router/test_wasms/SHA256SUMS. Toute
# divergence de checksum fait echouer le script.
#
# Usage : scripts/fetch_test_wasms.sh
set -euo pipefail

PINNED_SHA=84de10e0f8d26168b4a76f8c23b963e50917517c
BASE_URL="https://raw.githubusercontent.com/soroswap/aggregator/${PINNED_SHA}/contracts/aggregator"
DEST_DIR="$(cd "$(dirname "$0")/.." && pwd)/contracts/router/test_wasms"

# Fichiers vendorises : <sous-repertoire source>/<nom de fichier>.
FILES=(
  soroswap_contracts/soroswap_factory.wasm
  soroswap_contracts/soroswap_pair.wasm
  soroswap_contracts/soroswap_router.wasm
  aqua_contracts/soroban_liquidity_pool_router_contract.wasm
  aqua_contracts/soroban_liquidity_pool_contract.wasm
  aqua_contracts/soroban_liquidity_pool_plane_contract.wasm
  aqua_contracts/soroban_liquidity_pool_liquidity_calculator_contract.wasm
  aqua_contracts/soroban_token_contract.wasm
)

mkdir -p "$DEST_DIR"

for path in "${FILES[@]}"; do
  name="$(basename "$path")"
  echo "fetch ${path}"
  curl -sf "${BASE_URL}/${path}" -o "${DEST_DIR}/${name}"
done

# Verification d'integrite contre les checksums consignes.
echo "verification SHA-256"
cd "$DEST_DIR"
shasum -a 256 -c SHA256SUMS

echo "OK : $(wc -l < SHA256SUMS | tr -d ' ') wasm verifies dans ${DEST_DIR}"

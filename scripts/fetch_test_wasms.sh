#!/usr/bin/env bash
# Telechargement des wasm de venues (Soroswap, Aqua) pour les tests
# d'integration du routeur (contractimport! dans contracts/router).
#
# Source : depot soroswap/aggregator a COMMIT EPINGLE (le depot canonique
# Aqua est en 404, cf. contracts/router/test_wasms/README.md). Rejouable :
# chaque fichier est re-telecharge par-dessus l'existant (via un fichier
# temporaire, jamais d'ecriture partielle), puis l'integrite est verifiee
# contre contracts/router/test_wasms/SHA256SUMS. Toute divergence de
# checksum ou de nombre de fichiers fait echouer le script.
#
# Usage : scripts/fetch_test_wasms.sh
set -euo pipefail

PINNED_SHA=84de10e0f8d26168b4a76f8c23b963e50917517c
BASE_URL="https://raw.githubusercontent.com/soroswap/aggregator/${PINNED_SHA}/contracts/aggregator"
DEST_DIR="$(cd "$(dirname "$0")/.." && pwd)/contracts/router/test_wasms"
SUMS_FILE="${DEST_DIR}/SHA256SUMS"

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

# Outil de checksum : sha256sum si disponible, sinon shasum -a 256 (macOS).
if command -v sha256sum >/dev/null 2>&1; then
  SHA_CHECK=(sha256sum -c)
else
  SHA_CHECK=(shasum -a 256 -c)
fi

# Garde anti-derive FILES vs SHA256SUMS : chaque entree de FILES doit avoir
# sa ligne de checksum, sinon un fichier telecharge ne serait jamais verifie.
[ -f "$SUMS_FILE" ] || { echo "ERREUR : ${SUMS_FILE} introuvable" >&2; exit 1; }
SUMS_COUNT="$(grep -c . "$SUMS_FILE")"
if [ "${#FILES[@]}" -ne "$SUMS_COUNT" ]; then
  echo "ERREUR : ${#FILES[@]} entrees dans FILES mais ${SUMS_COUNT} lignes dans SHA256SUMS" >&2
  echo "Regenerer SHA256SUMS et le README ensemble (cf. section Re-epinglage du README)" >&2
  exit 1
fi

mkdir -p "$DEST_DIR"

for path in "${FILES[@]}"; do
  name="$(basename "$path")"
  echo "fetch ${path}"
  # Telechargement vers un temporaire puis mv : un transfert interrompu ne
  # laisse jamais un wasm partiel a la place du fichier committe.
  curl -fsS "${BASE_URL}/${path}" -o "${DEST_DIR}/${name}.tmp"
  mv "${DEST_DIR}/${name}.tmp" "${DEST_DIR}/${name}"
done

# Verification d'integrite contre les checksums consignes.
echo "verification SHA-256"
cd "$DEST_DIR"
"${SHA_CHECK[@]}" SHA256SUMS

echo "OK : ${SUMS_COUNT} wasm verifies dans ${DEST_DIR}"

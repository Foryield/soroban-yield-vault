#!/usr/bin/env bash
# Verification d'integrite des wasm de venue (Aquarius) utilises par les
# tests d'integration du routeur (contractimport! dans contracts/router).
#
# CE SCRIPT NE TELECHARGE PLUS RIEN depuis le 28/08/2026. Sa source amont
# etait le depot soroswap/aggregator, seul a publier les binaires Aqua (le
# depot canonique AquaToken/soroban-amm est en 404) ; ce depot sort du
# projet suite a la compromission de Soroswap (cf.
# docs/plans/2026-08-28-retrait-soroswap-aquarius-seul.md et
# contracts/router/test_wasms/README.md). Les binaires deja vendorises sont
# CONSERVES : epingles au commit 84de10e0 de juillet 2026, anterieur a
# l'incident rapporte, et verifiables par leurs empreintes.
#
# Ce qui reste : la verification d'integrite des fichiers committes contre
# contracts/router/test_wasms/SHA256SUMS. Toute divergence de checksum ou de
# nombre de fichiers fait echouer le script. La meme verification court a
# chaque execution des tests (vendored_wasms_match_sha256sums), ce script
# la rend disponible seule, sans compiler.
#
# Usage : scripts/fetch_test_wasms.sh
set -euo pipefail

DEST_DIR="$(cd "$(dirname "$0")/.." && pwd)/contracts/router/test_wasms"
SUMS_FILE="${DEST_DIR}/SHA256SUMS"

# Fichiers vendorises attendus (venue Aquarius).
FILES=(
  soroban_liquidity_pool_router_contract.wasm
  soroban_liquidity_pool_contract.wasm
  soroban_liquidity_pool_plane_contract.wasm
  soroban_liquidity_pool_liquidity_calculator_contract.wasm
  soroban_token_contract.wasm
)

# Outil de checksum : sha256sum si disponible, sinon shasum -a 256 (macOS).
if command -v sha256sum >/dev/null 2>&1; then
  SHA_CHECK=(sha256sum -c)
else
  SHA_CHECK=(shasum -a 256 -c)
fi

# Garde anti-derive FILES vs SHA256SUMS : chaque entree de FILES doit avoir
# sa ligne de checksum, sinon un fichier present ne serait jamais verifie.
[ -f "$SUMS_FILE" ] || { echo "ERREUR : ${SUMS_FILE} introuvable" >&2; exit 1; }
SUMS_COUNT="$(grep -c . "$SUMS_FILE")"
if [ "${#FILES[@]}" -ne "$SUMS_COUNT" ]; then
  echo "ERREUR : ${#FILES[@]} entrees dans FILES mais ${SUMS_COUNT} lignes dans SHA256SUMS" >&2
  echo "Regenerer SHA256SUMS et le README ensemble (cf. README des test_wasms)" >&2
  exit 1
fi

# Presence : un fichier manquant est une erreur explicite, plus aucun
# telechargement ne peut le rattraper.
for name in "${FILES[@]}"; do
  [ -f "${DEST_DIR}/${name}" ] || {
    echo "ERREUR : ${name} absent de ${DEST_DIR}" >&2
    echo "Ces binaires sont committes, les restaurer depuis git (aucune source amont)" >&2
    exit 1
  }
done

echo "verification SHA-256"
cd "$DEST_DIR"
"${SHA_CHECK[@]}" SHA256SUMS

echo "OK : ${SUMS_COUNT} wasm verifies dans ${DEST_DIR}"

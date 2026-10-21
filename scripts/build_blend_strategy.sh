#!/usr/bin/env bash
# Reconstruit le wasm de la strategie Blend de DeFindex depuis sa source
# epinglee et le compare a la copie vendorisee
# (contracts/vault/test_wasms/blend_strategy.wasm). Le meme wasm sert aux
# tests du vault et au deploiement testnet de D5 : ce script prouve qu'il
# sort bien de la source citee, sans modification.
#
# Source : https://github.com/defindex-io/stellar-contracts, commit epingle
# ci-dessous, crate strategies/blend. Licence GPL-3.0 (declaree dans le
# Cargo.toml du depot) : nous ne copions pas cette source dans notre depot,
# seulement le binaire compile, dont la source est ainsi indiquee.
#
# Build reproductible verifie le 20/10/2026 (deux builds independants, meme
# empreinte) avec stellar-cli 27.1.0 et rustc 1.94.0, cible wasm32v1-none.
#
# Usage : scripts/build_blend_strategy.sh [dossier_de_travail]
set -euo pipefail

REPO=https://github.com/defindex-io/stellar-contracts.git
COMMIT=82079667253622aa125eeb626fabbf0e47fe5cda

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
VENDORED="$ROOT/contracts/vault/test_wasms/blend_strategy.wasm"
SUMS="$ROOT/contracts/vault/test_wasms/SHA256SUMS"
WORK="${1:-$(mktemp -d)}"

git clone -q --filter=blob:none --no-checkout "$REPO" "$WORK/src"
git -C "$WORK/src" checkout -q "$COMMIT"
stellar contract build --manifest-path "$WORK/src/strategies/blend/Cargo.toml" >/dev/null
BUILT="$WORK/src/target/wasm32v1-none/release/blend_strategy.wasm"

EXPECTED=$(awk '$2 == "blend_strategy.wasm" { print $1 }' "$SUMS")
GOT=$(shasum -a 256 "$BUILT" | cut -d' ' -f1)
echo "attendu  $EXPECTED"
echo "obtenu   $GOT"
if [ "$GOT" != "$EXPECTED" ]; then
  echo "ECART : le wasm reconstruit differe de l'empreinte consignee." >&2
  exit 1
fi
cmp -s "$BUILT" "$VENDORED" || { echo "ECART : la copie vendorisee a change." >&2; exit 1; }
echo "OK : wasm reconstruit identique a la copie vendorisee ($BUILT)."

#!/usr/bin/env bash
# Deploiement testnet de l'instance EURC a frais D6b (plan
# docs/plans/2026-10-23-d6b-frais.md, section 7).
#
# Un vault de garde pure sur l'EURC de Circle, sans strategie (aucun pool
# EURC n'existe sur testnet), au code D6b : constructeur en une transaction,
# puis reglage des frais par l'admin. L'instance D3 reste sur son bytecode,
# ses preuves intactes (D6a, reponse 5).
#
# Taux de demonstration : performance 20 % (valeur du fixture du vault EVM),
# gestion 2 % par an. Toute invocation est simulee par la CLI avant
# soumission : un echec de simulation n'envoie rien.
#
# Usage : scripts/deploy_d6b.sh <cle_admin> <adresse_tresorerie> [adresse_keeper]
# Le keeper (sans objet tant que le vault n'a pas de strategie) est par
# defaut l'admin.
set -euo pipefail

KEY="${1:?usage: deploy_d6b.sh <cle_admin> <adresse_tresorerie> [adresse_keeper]}"
TREASURY="${2:?adresse G... de la tresorerie qui recoit les parts de frais}"
NETWORK=testnet
ROOT="$(cd "$(dirname "$0")/.." && pwd)"

ADDR=$(stellar keys address "$KEY")
KEEPER="${3:-$ADDR}"

# EURC de Circle sur testnet (meme emetteur que web/lib/vaults.ts).
EURC_ASSET="EURC:GB3Q6QDZYTHWT7E5PVS3W7FUT5GVAFC5KSZFFLPU25GO7VTC3NM2ZTVO"
EURC=$(stellar contract id asset --asset "$EURC_ASSET" --network $NETWORK)

# Sans strategie, le declencheur est sans objet ; il doit seulement
# depasser la perte toleree par lot (garde du constructeur).
DRIFT_BPS=500
PERF_FEE_BPS=2000
MGMT_FEE_BPS=200

echo "admin=$ADDR keeper=$KEEPER tresorerie=$TREASURY"
echo "eurc=$EURC_ASSET ($EURC)"

cargo build --manifest-path "$ROOT/Cargo.toml" --target wasm32v1-none --release -p yield-vault
VAULT_WASM="$ROOT/target/wasm32v1-none/release/yield_vault.wasm"
echo "yield_vault.wasm sha256 : $(shasum -a 256 "$VAULT_WASM" | cut -d' ' -f1)"

VAULT=$(stellar contract deploy --wasm "$VAULT_WASM" --source "$KEY" --network $NETWORK -- \
  --admin "$ADDR" --keeper "$KEEPER" --asset "$EURC" \
  --strategies '[]' --drift_bps $DRIFT_BPS)
echo "vault=$VAULT"

stellar contract invoke --id "$VAULT" --source "$KEY" --network $NETWORK -- \
  set_fees --treasury "$TREASURY" --perf_bps $PERF_FEE_BPS --mgmt_bps $MGMT_FEE_BPS

echo
echo "Vault EURC D6b : $VAULT"
echo "Frais (simulation) :"
stellar contract invoke --id "$VAULT" --source "$KEY" --network $NETWORK --send=no -- fees

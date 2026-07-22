#!/usr/bin/env bash
# Deploiement (re-deploiement) du SwapRouter sur testnet.
#
# deploy + initialize ENCHAINES dans la meme execution, dos a dos : ferme
# la fenetre de front-run d'initialisation (posture heritee du vault D1,
# plus rentable a attaquer ici : un initialize adverse fixerait des venues
# arbitraires). La CLI stellar ne passe des arguments au deploiement que
# pour les contrats a __constructor ; le notre expose initialize, donc deux
# transactions successives. La fenetre residuelle (quelques secondes) est
# acceptee sur testnet et consignee dans l'evidence. Aucune simulation
# intercalee entre les deux : elle elargirait la fenetre, et la CLI simule
# de toute facon chaque invocation avant soumission.
#
# Rejouable apres chaque reset du testnet SDF (2-4x/an) : deploie un
# contrat NEUF (venues immuables, pas de setter), l'initialise, puis
# enregistre le pool Aquarius de la paire USDC/EURC. Le pool_hash vient de
# la sortie `pool_hash=...` de scripts/seed_aquarius_pool.sh.
#
# Usage : scripts/deploy_swap_router.sh <cle> <aqua_pool_hash_hex>
set -euo pipefail

KEY="${1:?usage: deploy_swap_router.sh <cle> <aqua_pool_hash_hex>}"
AQUA_POOL_HASH="${2:?pool_hash hex du pool Aquarius (sortie de seed_aquarius_pool.sh)}"
NETWORK=testnet
ROOT="$(cd "$(dirname "$0")/.." && pwd)"

ADDR=$(stellar keys address "$KEY")

# Aggregator Soroswap testnet : relu du registre canonique du repo
# soroswap/aggregator a chaque execution, meme posture que le router
# Soroswap dans seed_soroswap_pool.sh. Surcharger SOROSWAP_AGGREGATOR
# pour court-circuiter le registre.
SOROSWAP_AGGREGATOR="${SOROSWAP_AGGREGATOR:-$(curl -sf https://raw.githubusercontent.com/soroswap/aggregator/main/public/testnet.contracts.json | python3 -c "import json,sys; print(json.load(sys.stdin)['ids']['aggregator'])")}"

# Router Aquarius testnet : PAS de registre JSON public connu (repo
# canonique AquaToken/soroban-amm en 404), donc pas de source relue a
# l'execution. Adresse etablie au spike S1 du 21/07/2026, annoncee stable
# aux resets ; meme provenance que dans seed_aquarius_pool.sh. Surcharger
# AQUA_ROUTER si elle change.
AQUA_ROUTER="${AQUA_ROUTER:-CBCFTQSPDBAIZ6R6PJQKSQWKNKWH2QIV3I4J72SHWBIK3ADRRAM5A6GD}"

# fee_bps COMPTABLES (le routeur ne preleve rien, cf. lib.rs) :
# - Soroswap : 30 bps = 0,3 %, LP fee du protocole (constante du pair) ;
# - Aquarius : 30 bps = fee tier du pool cree par seed_aquarius_pool.sh
#   (FEE_FRACTION=30). Un re-seed a un autre tier impose de redeployer
#   avec la valeur alignee (fee_bps immuables).
SOROSWAP_FEE_BPS=30
AQUARIUS_FEE_BPS=30

# Tokens de la paire du registre Aqua : relus aux sources canoniques,
# jamais codes en dur (memes sources que les scripts de seed).
USDC=$(curl -sf https://raw.githubusercontent.com/blend-capital/blend-utils/main/testnet.contracts.json | python3 -c "import json,sys; d=json.load(sys.stdin); print(d.get('ids', d)['USDC'])")
EURC=$(stellar contract id asset --asset EURC:GB3Q6QDZYTHWT7E5PVS3W7FUT5GVAFC5KSZFFLPU25GO7VTC3NM2ZTVO --network $NETWORK)

echo "aggregator=$SOROSWAP_AGGREGATOR aqua_router=$AQUA_ROUTER admin=$ADDR"
echo "usdc=$USDC eurc=$EURC aqua_pool_hash=$AQUA_POOL_HASH"

# Build du wasm release depuis la racine du workspace.
cargo build --manifest-path "$ROOT/Cargo.toml" --target wasm32v1-none --release -p swap-router
WASM="$ROOT/target/wasm32v1-none/release/swap_router.wasm"
echo "wasm sha256 : $(shasum -a 256 "$WASM" | cut -d' ' -f1)"

# --- Fenetre critique : deploy puis initialize, rien entre les deux. ---
CONTRACT_ID=$(stellar contract deploy --wasm "$WASM" --source "$KEY" --network $NETWORK)
stellar contract invoke --id "$CONTRACT_ID" --source "$KEY" --network $NETWORK -- \
  initialize --admin "$ADDR" \
  --soroswap_aggregator "$SOROSWAP_AGGREGATOR" \
  --aquarius_router "$AQUA_ROUTER" \
  --soroswap_fee_bps $SOROSWAP_FEE_BPS \
  --aquarius_fee_bps $AQUARIUS_FEE_BPS
# --- Fin de la fenetre : les venues sont fixees, immuables. ---

stellar contract invoke --id "$CONTRACT_ID" --source "$KEY" --network $NETWORK -- \
  set_aqua_pool --token_a "$USDC" --token_b "$EURC" --pool_hash "$AQUA_POOL_HASH"

echo "contract_id=$CONTRACT_ID"
echo "verification aqua_pool_of (simulation) :"
stellar contract invoke --id "$CONTRACT_ID" --source "$KEY" --network $NETWORK --send=no -- \
  aqua_pool_of --token_a "$USDC" --token_b "$EURC"

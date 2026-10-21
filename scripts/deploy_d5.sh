#!/usr/bin/env bash
# Deploiement testnet de l'allocateur D5 : deux strategies au trait DeFindex
# et le vault USDC qui les alloue.
#
# 1. Strategie Blend de DeFindex : le wasm vendorise
#    (contracts/vault/test_wasms, empreinte verifiee ici contre SHA256SUMS),
#    notre propre instance. Au parametre du routeur Soroswap, l'adresse du
#    router Aquarius, qui n'expose pas router_pair_for : Soroswap ne peut
#    jamais etre appele (plan D5, section 4).
# 2. Strategie Aquarius : notre crate, prix de reference initial = taux BCE
#    du jour, pool realigne sur ce prix avant le premier depot.
# 3. Vault D5 : constructeur (admin, keeper, actif, strategies, drift), une
#    seule transaction, aucune fenetre de front-run.
#
# Chaque strategie recoit d'abord un depot d'amorcage de la cle d'ops : ses
# 1000 parts mortes sont prelevees sur ce depot, pas sur celui du vault. Le
# depot d'amorcage Aquarius est aussi la premiere entree reelle dans le pool
# deploye : il eprouve l'arbre d'autorisation du depot LP et la memoire de
# l'invocation sur le pool vivant, plus recent que nos wasm de test (plan D5,
# section 15). Toute invocation est simulee par la CLI avant soumission : un
# echec de simulation n'envoie rien.
#
# Valeurs du plan D5 (section 13) : cibles Blend 70 % et Aquarius 20 %,
# plafonds 90 % et 30 %, declencheur 500 pb ; pas de 1 % par publication,
# glissement maximal 1 %. Prix de reference perime au bout de 72 h, et non
# 26 h comme au plan (revue de securite du 21/10) : un prix perime gele tout
# le vault, depots et retraits compris ; 72 h couvrent un week-end ou une
# tache quotidienne manquee. L'euro-dollar bouge rarement de plus de 1 % en
# trois jours, l'ecart de valorisation reste dans la marge de glissement.
#
# L'euro de la paire Aquarius est le FYEUR (scripts/issue_test_eur.sh) par
# defaut, comme dans seed_aquarius_pool.sh ; PAIRED_ASSET=CODE:EMETTEUR pour
# un autre actif classique.
#
# Usage : scripts/deploy_d5.sh <cle_ops> <aqua_pool_hash_hex> [adresse_keeper]
# Le keeper est par defaut la cle d'ops (plan D5, section 4).
set -euo pipefail

KEY="${1:?usage: deploy_d5.sh <cle_ops> <aqua_pool_hash_hex> [adresse_keeper]}"
AQUA_POOL_HASH="${2:?pool_hash hex du pool Aquarius (sortie de seed_aquarius_pool.sh)}"
NETWORK=testnet
ROOT="$(cd "$(dirname "$0")/.." && pwd)"

ADDR=$(stellar keys address "$KEY")
KEEPER="${3:-$ADDR}"
PAIRED_ASSET="${PAIRED_ASSET:-FYEUR:$(stellar keys address fy-eur-issuer)}"

# Router Aquarius : adresse publiee sur docs.aqua.network (cf.
# seed_aquarius_pool.sh), surcharger AQUA_ROUTER si elle change.
AQUA_ROUTER="${AQUA_ROUTER:-CBCFTQSPDBAIZ6R6PJQKSQWKNKWH2QIV3I4J72SHWBIK3ADRRAM5A6GD}"

# Allocation et gardes (plan D5, section 13).
BLEND_TARGET_BPS=7000
BLEND_CAP_BPS=9000
AQUA_TARGET_BPS=2000
AQUA_CAP_BPS=3000
DRIFT_BPS=500
PRICE_MAX_AGE=259200     # 72 h
PRICE_MAX_STEP_BPS=100
MAX_SLIPPAGE_BPS=100
# Seuil de reinvestissement des BLND de la strategie Blend, celui des tests
# (test_blend.rs) : au-dessous, harvest reclame et s'arrete ; au-dessus, il
# echoue avant tout transfert.
BLEND_REWARD_THRESHOLD=10000000
# Depot d'amorcage de chaque strategie : 0,01 USDC, cent fois les parts
# mortes.
SEED=100000

# Sources canoniques, jamais codees en dur : USDC, BLND et le pool Blend du
# registre blend-utils, l'euro de la paire par son SAC deterministe.
REGISTRY=$(curl -sf https://raw.githubusercontent.com/blend-capital/blend-utils/main/testnet.contracts.json)
registry() { printf '%s' "$REGISTRY" | python3 -c "import json,sys; print(json.load(sys.stdin)['ids']['$1'])"; }
USDC=$(registry USDC)
BLND=$(registry BLND)
BLEND_POOL=$(registry TestnetV2)
EURC=$(stellar contract id asset --asset "$PAIRED_ASSET" --network $NETWORK)

# Prix de reference initial : taux BCE du jour, USD pour 1 EUR, echelle 1e9
# (meme lecture que push_reference_price.sh).
PRICE=$(curl -sf https://www.ecb.europa.eu/stats/eurofxref/eurofxref-daily.xml | python3 -c "
import re, sys
from decimal import Decimal
rate = re.search(r\"currency='USD' rate='([0-9.]+)'\", sys.stdin.read())
if not rate:
    sys.exit('flux BCE illisible')
print(int(Decimal(rate.group(1)) * 10**9))
")

echo "admin=$ADDR keeper=$KEEPER"
echo "usdc=$USDC euro=$PAIRED_ASSET ($EURC) blnd=$BLND blend_pool=$BLEND_POOL"
echo "aqua_router=$AQUA_ROUTER aqua_pool_hash=$AQUA_POOL_HASH prix_bce=$PRICE"

# Wasm : la strategie Blend vendorisee, verifiee ; les notres, compiles.
( cd "$ROOT/contracts/vault/test_wasms" && shasum -a 256 -c SHA256SUMS )
cargo build --manifest-path "$ROOT/Cargo.toml" --target wasm32v1-none --release \
  -p yield-vault -p strategy-aquarius
BLEND_WASM="$ROOT/contracts/vault/test_wasms/blend_strategy.wasm"
AQUA_WASM="$ROOT/target/wasm32v1-none/release/strategy_aquarius.wasm"
VAULT_WASM="$ROOT/target/wasm32v1-none/release/yield_vault.wasm"
for w in "$AQUA_WASM" "$VAULT_WASM"; do
  echo "$(basename "$w") sha256 : $(shasum -a 256 "$w" | cut -d' ' -f1)"
done

invoke() {
  local id="$1"; shift
  stellar contract invoke --id "$id" --source "$KEY" --network $NETWORK -- "$@"
}

# --- 1. Strategie Blend ------------------------------------------------------
BLEND_STRATEGY=$(stellar contract deploy --wasm "$BLEND_WASM" --source "$KEY" --network $NETWORK -- \
  --asset "$USDC" \
  --init_args "[{\"address\":\"$BLEND_POOL\"},{\"address\":\"$BLND\"},{\"address\":\"$AQUA_ROUTER\"},{\"i128\":\"$BLEND_REWARD_THRESHOLD\"},{\"address\":\"$KEEPER\"}]")
echo "blend_strategy=$BLEND_STRATEGY"
invoke "$BLEND_STRATEGY" deposit --amount $SEED --from "$ADDR"

# --- 2. Strategie Aquarius ---------------------------------------------------
# Le constructeur verifie que le pool existe pour la paire et qu'il est a
# produit constant.
AQUA_STRATEGY=$(stellar contract deploy --wasm "$AQUA_WASM" --source "$KEY" --network $NETWORK -- \
  --asset "$USDC" \
  --init_args "[{\"address\":\"$ADDR\"},{\"address\":\"$KEEPER\"},{\"address\":\"$AQUA_ROUTER\"},{\"bytes\":\"$AQUA_POOL_HASH\"},{\"address\":\"$EURC\"},{\"i128\":\"$PRICE\"},{\"u64\":\"$PRICE_MAX_AGE\"},{\"u32\":$PRICE_MAX_STEP_BPS},{\"u32\":$MAX_SLIPPAGE_BPS}]")
echo "aquarius_strategy=$AQUA_STRATEGY"
"$ROOT/scripts/align_aquarius_pool.sh" "$KEY" "$AQUA_STRATEGY"
invoke "$AQUA_STRATEGY" deposit --amount $SEED --from "$ADDR"

# --- 3. Vault D5 -------------------------------------------------------------
# Ordre de la liste = ordre de liquidite des retraits : Blend d'abord.
STRATEGIES="[{\"address\":\"$BLEND_STRATEGY\",\"protocol\":\"blend\",\"target_bps\":$BLEND_TARGET_BPS,\"cap_bps\":$BLEND_CAP_BPS,\"active\":true},{\"address\":\"$AQUA_STRATEGY\",\"protocol\":\"aquarius\",\"target_bps\":$AQUA_TARGET_BPS,\"cap_bps\":$AQUA_CAP_BPS,\"active\":true}]"
VAULT=$(stellar contract deploy --wasm "$VAULT_WASM" --source "$KEY" --network $NETWORK -- \
  --admin "$ADDR" --keeper "$KEEPER" --asset "$USDC" \
  --strategies "$STRATEGIES" --drift_bps $DRIFT_BPS)

echo
echo "Strategie Blend    : $BLEND_STRATEGY"
echo "Strategie Aquarius : $AQUA_STRATEGY"
echo "Vault D5           : $VAULT"
echo "Configuration (simulation) :"
stellar contract invoke --id "$VAULT" --source "$KEY" --network $NETWORK --send=no -- strategies
echo
echo "Avant tout mouvement Aquarius : scripts/align_aquarius_pool.sh $KEY $AQUA_STRATEGY"
echo "Chaque jour : scripts/push_reference_price.sh $KEY $AQUA_STRATEGY"

#!/usr/bin/env bash
# Seed (re-seed) du pool Aquarius testnet USDC-Blend / EURC-Circle.
#
# Miroir de seed_soroswap_pool.sh cote Aquarius : cree le pool standard
# (constant product) via init_standard_pool puis le finance via deposit.
# Affiche pool_hash et adresse du pool : le pool_hash est la valeur
# attendue par set_aqua_pool sur notre SwapRouter.
#
# Idempotent : init_standard_pool du router Aqua rend le pool existant
# sans paiement si la paire a deja un pool au meme fee tier (source :
# miroir calc1f4r/soroban-amm@f9d4a5e0, contract.rs) ; la creation est
# alors sautee et seul le deposit est rejoue. Rejouable apres chaque
# reset du testnet SDF (2-4x/an).
#
# Paiement de creation : le router Aquarius DEPLOYE exige un paiement en
# AQUA testnet a la creation d'un pool (get_standard_pool_payment_amount,
# 1.0 AQUA constate le 22/07/2026 -- notre fixture le configure a 0, pas
# le vrai router). La simulation prealable echoue si le compte ne peut pas
# payer ; marche a suivre pour obtenir de l'AQUA : trustline puis swap
# XLM->AQUA sur le pool XLM/AQUA du meme router
# (cf. docs/evidence/d4-dex-routing.md, section du 2026-07-22).
#
# Usage : scripts/seed_aquarius_pool.sh <cle> <montant_usdc_7dp> <montant_eurc_7dp>
set -euo pipefail

KEY="${1:?usage: seed_aquarius_pool.sh <cle> <usdc_7dp> <eurc_7dp>}"
AMOUNT_USDC="${2:?montant USDC en unites 7 decimales}"
AMOUNT_EURC="${3:?montant EURC en unites 7 decimales}"
NETWORK=testnet

# Router Aquarius testnet : PAS de registre JSON public connu (le repo
# canonique AquaToken/soroban-amm est en 404), donc pas de source relue a
# l'execution. Adresse etablie au spike S1 du 21/07/2026, annoncee stable
# aux resets du testnet ; surcharger AQUA_ROUTER si elle change.
AQUA_ROUTER="${AQUA_ROUTER:-CBCFTQSPDBAIZ6R6PJQKSQWKNKWH2QIV3I4J72SHWBIK3ADRRAM5A6GD}"

# fee_fraction en 1/10 000 : liste blanche du router [10, 30, 100]
# (miroir, liquidity_pool_router/src/constants.rs). 30 = 0,3 %, meme taux
# nominal que Soroswap.
FEE_FRACTION=30

ADDR=$(stellar keys address "$KEY")

# Adresses de tokens relues aux sources canoniques, jamais codees en dur.
USDC=$(curl -sf https://raw.githubusercontent.com/blend-capital/blend-utils/main/testnet.contracts.json | python3 -c "import json,sys; d=json.load(sys.stdin); print(d.get('ids', d)['USDC'])")
EURC=$(stellar contract id asset --asset EURC:GB3Q6QDZYTHWT7E5PVS3W7FUT5GVAFC5KSZFFLPU25GO7VTC3NM2ZTVO --network $NETWORK)

# Paire TRIEE par adresse (assert_tokens_sorted du router Aqua). L'ordre
# des Address Soroban est celui des octets bruts du hash de contrat, PAS
# l'ordre lexicographique des strkeys (le base32 n'est pas monotone en
# ASCII) : on decode avant de comparer.
SORTED=$(python3 - "$USDC" "$EURC" <<'PY'
import base64, sys
def raw(strkey):
    return base64.b32decode(strkey)[1:-2]
a, b = sys.argv[1], sys.argv[2]
print("\n".join([a, b] if raw(a) < raw(b) else [b, a]))
PY
)
TOKEN_0=$(echo "$SORTED" | head -1)
TOKEN_1=$(echo "$SORTED" | tail -1)
TOKENS_JSON="[\"$TOKEN_0\",\"$TOKEN_1\"]"
if [ "$TOKEN_0" = "$USDC" ]; then
  AMOUNT_0=$AMOUNT_USDC AMOUNT_1=$AMOUNT_EURC
else
  AMOUNT_0=$AMOUNT_EURC AMOUNT_1=$AMOUNT_USDC
fi

echo "router=$AQUA_ROUTER usdc=$USDC eurc=$EURC user=$ADDR"

# Invocation en simulation seule (aucune soumission).
simulate() {
  stellar contract invoke --id "$AQUA_ROUTER" --source "$KEY" --network $NETWORK --send=no -- "$@"
}

# Simulation d'abord : idempotente cote router (pool existant rendu tel
# quel), et son succes prouve que le paiement de creation est couvert.
SIM=$(simulate init_standard_pool --user "$ADDR" --tokens "$TOKENS_JSON" --fee_fraction $FEE_FRACTION) || {
  echo "ERREUR : simulation init_standard_pool en echec (diagnostic ci-dessus)." >&2
  echo "Cause probable : paiement de creation en AQUA non couvert. Obtenir de" >&2
  echo "l'AQUA testnet : trustline AQUA puis swap XLM->AQUA sur le meme router" >&2
  echo "(cf. docs/evidence/d4-dex-routing.md, 2026-07-22)." >&2
  exit 1
}
POOL_HASH=$(echo "$SIM" | python3 -c "import json,sys; print(json.load(sys.stdin)[0])")
POOL_ADDRESS=$(echo "$SIM" | python3 -c "import json,sys; print(json.load(sys.stdin)[1])")

if simulate get_pools --tokens "$TOKENS_JSON" | python3 -c "import json,sys; sys.exit(0 if \"$POOL_HASH\" in json.load(sys.stdin) else 1)"; then
  echo "pool fee=$FEE_FRACTION deja existant, creation sautee"
else
  stellar contract invoke --id "$AQUA_ROUTER" --source "$KEY" --network $NETWORK -- \
    init_standard_pool --user "$ADDR" --tokens "$TOKENS_JSON" --fee_fraction $FEE_FRACTION
fi

echo "pool_hash=$POOL_HASH"
echo "pool_address=$POOL_ADDRESS"

# Deposit : parts attendues lues en simulation, puis min_shares a 90 %
# (meme tolerance que le script Soroswap ; premiere fourniture = prix
# libre, re-seed = tolerance de 10 %).
DESIRED="[\"$AMOUNT_0\",\"$AMOUNT_1\"]"
SHARES=$(simulate deposit --user "$ADDR" --tokens "$TOKENS_JSON" --pool_index "$POOL_HASH" --desired_amounts "$DESIRED" --min_shares 0 | python3 -c "import json,sys; print(json.load(sys.stdin)[1])")
stellar contract invoke --id "$AQUA_ROUTER" --source "$KEY" --network $NETWORK -- \
  deposit --user "$ADDR" --tokens "$TOKENS_JSON" --pool_index "$POOL_HASH" \
  --desired_amounts "$DESIRED" --min_shares $(( SHARES * 9 / 10 ))

echo "reserves apres seed (ordre des tokens tries) :"
simulate get_reserves --tokens "$TOKENS_JSON" --pool_index "$POOL_HASH"

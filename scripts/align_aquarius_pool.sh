#!/usr/bin/env bash
# Realignement du pool Aquarius USDC/EURC sur le prix de reference (D5).
#
# La strategie Aquarius valorise sa position LP a la valeur equitable au prix
# de reference, et controle chaque entree et chaque sortie contre elle (1 %).
# Un pool decale du prix de reference fait donc payer le decalage a qui entre
# ou sort, jusqu'au refus. Le pool de demonstration est etroit et nous en
# sommes l'essentiel de la liquidite : personne ne l'arbitre pour nous. Ce
# script le ramene au prix de reference par un swap de la cle d'ops, juste
# avant tout mouvement Aquarius (rebalance, retrait qui puise dans la
# strategie), comme le prevoit le plan D5 (section 5).
#
# Tout est relu sur la chaine : le pool, la paire et le router dans la
# configuration de la strategie, le prix dans reference_price, les reserves
# dans le router. Cotation en simulation, puis swap avec out_min a 99 % de la
# cotation. Rien n'est soumis si l'ecart est deja sous la tolerance.
#
# Usage : scripts/align_aquarius_pool.sh <cle> <strategie_aquarius> [tolerance_pb]
set -euo pipefail

KEY="${1:?usage: align_aquarius_pool.sh <cle> <strategie_aquarius> [tolerance_pb]}"
STRATEGY="${2:?adresse de la strategie Aquarius}"
# Ecart toleree, en points de base du prix : 10 pb = 0,1 %, le dixieme de la
# marge de glissement.
TOLERANCE_BPS="${3:-10}"
NETWORK=testnet

ADDR=$(stellar keys address "$KEY")

simulate() {
  local id="$1"; shift
  stellar contract invoke --id "$id" --source "$KEY" --network $NETWORK --send=no -- "$@"
}

CONFIG=$(simulate "$STRATEGY" config)
PRICE=$(simulate "$STRATEGY" reference_price | python3 -c "import json,sys; print(int(json.load(sys.stdin)['price']))")
read -r ROUTER POOL_HASH TOKENS_JSON USDC EURC FEE_BPS < <(printf '%s' "$CONFIG" | python3 -c "
import json, sys
c = json.load(sys.stdin)
print(c['router'], c['pool_hash'], json.dumps(c['tokens'], separators=(',', ':')),
      c['asset'], c['paired'], int(c['fee_bps']))
")
# Une substitution de processus en echec ne fait pas echouer `read` : les
# gardes suivantes s'en chargent.
: "${FEE_BPS:?configuration de la strategie illisible}"
RESERVES=$(simulate "$ROUTER" get_reserves --tokens "$TOKENS_JSON" --pool_index "$POOL_HASH")

# Calcul exact en entiers. Prix de reference : USDC pour 1 EURC, echelle 1e9.
# Produit constant k = r_eurc x r_usdc ; au prix p, la reserve EURC alignee
# vaut sqrt(k x 1e9 / p). Le swap qui y mene est majore des frais du pool,
# preleves sur l'entree.
read -r GAP_BPS TOKEN_IN TOKEN_OUT AMOUNT_IN < <(python3 - "$RESERVES" "$TOKENS_JSON" "$USDC" "$PRICE" "$FEE_BPS" <<'PY'
import json, math, sys
reserves, tokens = json.loads(sys.argv[1]), json.loads(sys.argv[2])
usdc, price, fee = sys.argv[3], int(sys.argv[4]), int(sys.argv[5])
by_token = dict(zip(tokens, map(int, reserves)))
r_usdc = by_token[usdc]
r_eurc = next(v for t, v in by_token.items() if t != usdc)
pool_price = r_usdc * 10**9 // r_eurc
gap = abs(pool_price - price) * 10_000 // price
k = r_eurc * r_usdc
target_eurc = math.isqrt(k * 10**9 // price)
gross = lambda x: x * 10_000 // (10_000 - fee) + 1
if target_eurc > r_eurc:
    # EURC trop rare dans le pool, donc trop cher : on en apporte.
    print(gap, "eurc", "usdc", gross(target_eurc - r_eurc))
else:
    # EURC en exces : on apporte de l'USDC et on retire de l'EURC.
    print(gap, "usdc", "eurc", gross(k // target_eurc - r_usdc))
PY
)
: "${AMOUNT_IN:?calcul de l'alignement en echec}"
[ "$TOKEN_IN" = usdc ] && IN=$USDC OUT=$EURC || { IN=$EURC; OUT=$USDC; }

echo "prix de reference=$PRICE reserves=$RESERVES ecart=${GAP_BPS} pb"
if [ "$GAP_BPS" -le "$TOLERANCE_BPS" ]; then
  echo "pool aligne a ${TOLERANCE_BPS} pb pres : rien a faire"
  exit 0
fi

QUOTE=$(simulate "$ROUTER" estimate_swap --tokens "$TOKENS_JSON" --token_in "$IN" \
  --token_out "$OUT" --pool_index "$POOL_HASH" --in_amount "$AMOUNT_IN" | tr -d '"')
echo "swap $TOKEN_IN -> $TOKEN_OUT : $AMOUNT_IN pour $QUOTE cotes"
stellar contract invoke --id "$ROUTER" --source "$KEY" --network $NETWORK -- \
  swap_chained --user "$ADDR" \
  --swaps_chain "[[$TOKENS_JSON,\"$POOL_HASH\",\"$OUT\"]]" \
  --token_in "$IN" --in_amount "$AMOUNT_IN" --out_min $(( QUOTE * 99 / 100 ))

RESERVES=$(simulate "$ROUTER" get_reserves --tokens "$TOKENS_JSON" --pool_index "$POOL_HASH")
echo "reserves apres alignement : $RESERVES"

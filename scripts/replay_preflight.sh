#!/usr/bin/env bash
# Verification prealable au rejeu des preuves apres un reset du testnet.
# SIMULATION ET LECTURE UNIQUEMENT : rien n'est signe ni soumis.
#
# Le rejeu (docs/plans/2026-10-20-rejeu-des-preuves-apres-reset.md) enchaine
# les scripts de redeploiement existants ; chacun echoue tard et a mi-chemin
# s'il manque un jeton. Ce script dit d'emblee ce qui manque au compte d'ops,
# avec le renvoi vers l'etape de la procedure qui le fournit :
#   1. XLM pour les frais (Friendbot) ;
#   2. USDC Blend pour D1 et D4 (emprunt sur le pool TestnetV2) ;
#   3. trustline et solde EURC pour D3 et D4 (faucet Circle) ;
#   4. routeur Aquarius joignable, et AQUA si le pool USDC/EURC est a recreer.
#
# Usage : scripts/replay_preflight.sh <cle>
# Code de sortie : 0 si le compte peut lancer le rejeu, 1 sinon.
#
# Variables d'environnement (unites 7 decimales) :
#   MIN_XLM   defaut 100 XLM
#   MIN_USDC  defaut 101.5 USDC : depot de preuve D1 (100) + seed Aquarius (1.5)
#   MIN_EURC  defaut 6.5 EURC : depot de preuve D3 (5) + seed Aquarius (1.5)
#   MIN_AQUA  defaut 1 AQUA : paiement de creation d'un pool Aquarius
set -euo pipefail

KEY="${1:?usage: replay_preflight.sh <cle>}"
NETWORK=testnet
MIN_XLM="${MIN_XLM:-1000000000}"
MIN_USDC="${MIN_USDC:-1015000000}"
MIN_EURC="${MIN_EURC:-65000000}"
MIN_AQUA="${MIN_AQUA:-10000000}"
RUNBOOK=docs/plans/2026-10-20-rejeu-des-preuves-apres-reset.md

# Adresse publiee par Aquarius (docs.aqua.network), annoncee stable aux resets.
AQUA_ROUTER="${AQUA_ROUTER:-CBCFTQSPDBAIZ6R6PJQKSQWKNKWH2QIV3I4J72SHWBIK3ADRRAM5A6GD}"
EURC_ASSET="EURC:GB3Q6QDZYTHWT7E5PVS3W7FUT5GVAFC5KSZFFLPU25GO7VTC3NM2ZTVO"
AQUA_ASSET="AQUA:GAHPYWLK6YRN7CVYZOO4H3VDRZ7PVF5UJGLZCSPAEIKJE2XSWF5LAGER"

ADDR=$(stellar keys address "$KEY")
echo "compte d'ops : $ADDR ($KEY)"

simulate() {
  local id="$1"; shift
  stellar contract invoke --id "$id" --source "$KEY" --network $NETWORK --send=no -- "$@" 2>/dev/null
}

# Solde d'un SAC ; une simulation en echec (compte absent, trustline absente)
# compte pour zero, la remediation est la meme.
balance_of() {
  simulate "$1" balance --id "$ADDR" | tr -d '"' || echo 0
}

FAILURES=0
need() {
  local label="$1" got="$2" min="$3" fix="$4"
  if [ "$got" -ge "$min" ]; then
    echo "OK      $label : $got (minimum $min)"
  else
    echo "MANQUE  $label : $got (minimum $min) ; $fix"
    FAILURES=$((FAILURES + 1))
  fi
}

# Le compte doit exister avant toute simulation : apres un reset, il n'existe
# plus et chaque appel suivant echouerait pour cette seule raison.
if ! curl -sf "https://horizon-testnet.stellar.org/accounts/$ADDR" -o /dev/null; then
  echo "MANQUE  compte absent du ledger ; etape 1 de $RUNBOOK (stellar keys fund $KEY --network testnet)"
  exit 1
fi

XLM=$(stellar contract id asset --asset native --network $NETWORK)
need "XLM" "$(balance_of "$XLM")" "$MIN_XLM" "etape 1 de $RUNBOOK (Friendbot)"

REGISTRY=$(curl -sf https://raw.githubusercontent.com/blend-capital/blend-utils/main/testnet.contracts.json) || {
  echo "MANQUE  registre blend-utils injoignable : Blend n'a peut-etre pas encore redeploye"
  exit 1
}
USDC=$(printf '%s' "$REGISTRY" | python3 -c "import json,sys; print(json.load(sys.stdin)['ids']['USDC'])")
need "USDC Blend" "$(balance_of "$USDC")" "$MIN_USDC" "etape 2 de $RUNBOOK (emprunt Blend)"

EURC=$(stellar contract id asset --asset "$EURC_ASSET" --network $NETWORK)
need "EURC" "$(balance_of "$EURC")" "$MIN_EURC" "etape 3 de $RUNBOOK (trustline puis faucet Circle)"

TOKENS_JSON=$(python3 - "$USDC" "$EURC" <<'PY'
import base64, json, sys
raw = lambda s: base64.b32decode(s)[1:-2]
a, b = sys.argv[1], sys.argv[2]
print(json.dumps([a, b] if raw(a) < raw(b) else [b, a]))
PY
)
if ! POOLS=$(simulate "$AQUA_ROUTER" get_pools --tokens "$TOKENS_JSON"); then
  echo "MANQUE  routeur Aquarius $AQUA_ROUTER injoignable : verifier l'adresse publiee sur docs.aqua.network"
  FAILURES=$((FAILURES + 1))
elif [ "$POOLS" = "{}" ]; then
  AQUA=$(stellar contract id asset --asset "$AQUA_ASSET" --network $NETWORK)
  need "AQUA (pool USDC/EURC a recreer)" "$(balance_of "$AQUA")" "$MIN_AQUA" "etape 4 de $RUNBOOK (trustline puis swap XLM vers AQUA)"
else
  echo "OK      pool Aquarius USDC/EURC present, pas de paiement de creation"
fi

if [ "$FAILURES" -gt 0 ]; then
  echo "$FAILURES manque(s) : completer avant de lancer le rejeu."
  exit 1
fi
echo "compte pret pour le rejeu"

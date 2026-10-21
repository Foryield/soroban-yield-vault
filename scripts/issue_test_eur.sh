#!/usr/bin/env bash
# Emission du FYEUR, euro de test de la jambe Aquarius D5.
#
# Le pool Aquarius USDC / EURC de Circle du testnet est arbitre par un robot
# tiers (essai du 21/10, docs/evidence/d5-defindex-allocator.md) : il impose
# son propre prix, loin du taux BCE, et la garde de glissement de la strategie
# refuse alors toute entree. Le FYEUR n'a aucun marche tiers : le pool
# USDC / FYEUR n'a que notre prix, celui de la BCE. Contrepartie assumee :
# nous emettons ce jeton, il ne vaut un euro que par convention de test.
#
# Etapes, chacune sautee si deja faite (rejouable apres un reset du testnet) :
# 1. cle emettrice locale, creee et financee par friendbot ;
# 2. contrat d'actif Stellar (SAC) du FYEUR ;
# 3. ligne de confiance du compte d'ops ;
# 4. emission vers le compte d'ops jusqu'au solde cible.
#
# Usage : scripts/issue_test_eur.sh <cle_ops> <solde_cible_7dp> [cle_emettrice]
set -euo pipefail

KEY="${1:?usage: issue_test_eur.sh <cle_ops> <solde_cible_7dp> [cle_emettrice]}"
TARGET="${2:?solde FYEUR cible du compte d ops, en unites 7 decimales}"
ISSUER_KEY="${3:-fy-eur-issuer}"
NETWORK=testnet
CODE=FYEUR
HORIZON=https://horizon-testnet.stellar.org

if ! stellar keys address "$ISSUER_KEY" >/dev/null 2>&1; then
  stellar keys generate "$ISSUER_KEY" --network $NETWORK --fund
fi
ISSUER=$(stellar keys address "$ISSUER_KEY")
ADDR=$(stellar keys address "$KEY")
ASSET="$CODE:$ISSUER"

# --- 2. SAC -----------------------------------------------------------------
SAC=$(stellar contract id asset --asset "$ASSET" --network $NETWORK)
if stellar contract invoke --id "$SAC" --source "$KEY" --network $NETWORK --send=no -- name >/dev/null 2>&1; then
  echo "SAC deja deploye"
else
  stellar contract asset deploy --asset "$ASSET" --source "$KEY" --network $NETWORK
fi

# Solde classique du compte d'ops en unites 7 decimales, vide sans ligne de
# confiance.
balance() {
  curl -sf "$HORIZON/accounts/$ADDR" | python3 -c "
import sys, json
from decimal import Decimal
for b in json.load(sys.stdin)['balances']:
    if b.get('asset_code') == '$CODE' and b.get('asset_issuer') == '$ISSUER':
        print(int(Decimal(b['balance']) * 10**7))
"
}

# --- 3. Ligne de confiance --------------------------------------------------
HELD=$(balance)
if [ -z "$HELD" ]; then
  stellar tx new change-trust --source "$KEY" --line "$ASSET" --network $NETWORK
  HELD=0
fi

# --- 4. Emission ------------------------------------------------------------
if [ "$HELD" -lt "$TARGET" ]; then
  stellar tx new payment --source "$ISSUER_KEY" --destination "$ADDR" \
    --asset "$ASSET" --amount $(( TARGET - HELD )) --network $NETWORK
else
  echo "solde deja atteint : $HELD"
fi

echo "fyeur_asset=$ASSET"
echo "fyeur_sac=$SAC"
echo "solde_ops=$(balance)"

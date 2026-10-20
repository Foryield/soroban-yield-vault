#!/usr/bin/env bash
# Publication du prix de reference EURC/USDC sur la strategie Aquarius (D5).
#
# Source : taux de reference quotidien de la BCE, USD pour 1 EUR, flux XML
# public et gratuit (aucune cle, aucun abonnement). USDC et EURC suivent le
# dollar et l'euro : ce taux est la valeur equitable de la paire. Il sert a
# valoriser la position LP (valeur equitable, plan D5 section 5), jamais a
# trader.
#
# Le contrat borne chaque publication : 1 % d'ecart au plus avec le prix en
# place, une publication par heure au plus. Si le taux du jour s'ecarte de
# plus d'un pas, le script publie le pas maximal dans sa direction et dit
# l'ecart restant : relance une heure plus tard, il converge. Le week-end,
# sans nouveau taux, il republie celui du vendredi : la publication rafraichit
# la date, sans quoi le prix perime au bout de max_age (26 h) et toute
# valorisation echoue.
#
# source_date = date du taux BCE au format AAAAMMJJ (ex. 20261020), lisible
# dans l'event de publication.
#
# Tache planifiee : une fois par jour suffit (taux publie vers 16 h CET),
# par exemple en crontab :
#   30 17 * * * /chemin/scripts/push_reference_price.sh d1-ops <strategie>
#
# Usage : scripts/push_reference_price.sh <cle keeper ou admin> <strategie_aquarius>
set -euo pipefail

KEY="${1:?usage: push_reference_price.sh <cle> <strategie_aquarius>}"
STRATEGY="${2:?adresse de la strategie Aquarius}"
NETWORK=testnet
ECB_URL=https://www.ecb.europa.eu/stats/eurofxref/eurofxref-daily.xml
# Echelle du prix dans le contrat (PRICE_SCALE).
SCALE=1000000000
# Intervalle minimal entre deux publications (MIN_PUBLISH_INTERVAL), en s.
MIN_INTERVAL=3600

ADDR=$(stellar keys address "$KEY")

simulate() {
  stellar contract invoke --id "$STRATEGY" --source "$KEY" --network $NETWORK --send=no -- "$@"
}

# Taux du jour : date et cours USD, convertis en entier exact a l'echelle 1e9
# (la BCE publie 4 decimales, aucune perte).
read -r SOURCE_DATE TARGET < <(curl -sf "$ECB_URL" | python3 -c "
import re, sys
from decimal import Decimal
xml = sys.stdin.read()
date = re.search(r\"time='(\d{4})-(\d{2})-(\d{2})'\", xml)
rate = re.search(r\"currency='USD' rate='([0-9.]+)'\", xml)
if not date or not rate:
    sys.exit('flux BCE illisible')
print(''.join(date.groups()), int(Decimal(rate.group(1)) * $SCALE))
")
# Une substitution de processus en echec ne fait pas echouer `read` : les
# gardes suivantes s'en chargent.
: "${TARGET:?taux BCE illisible}"

# Prix en place et pas autorise, lus en simulation.
read -r CURRENT UPDATED_AT < <(simulate reference_price | python3 -c "
import json, sys
p = json.load(sys.stdin)
print(int(p['price']), int(p['updated_at']))
")
: "${UPDATED_AT:?prix en place illisible}"
STEP_BPS=$(simulate config | python3 -c "import json,sys; print(int(json.load(sys.stdin)['max_step_bps']))")

NOW=$(date +%s)
if [ $(( NOW - UPDATED_AT )) -lt $MIN_INTERVAL ]; then
  echo "derniere publication il y a $(( NOW - UPDATED_AT )) s, moins d'une heure : rien a faire"
  exit 0
fi

# Pas maximal dans la direction du taux du jour, meme arrondi que le contrat
# (ecart x 10 000 <= prix x pas) : on publie la cible si elle est a portee.
read -r PRICE GAP_BPS < <(python3 -c "
current, target, step = $CURRENT, $TARGET, $STEP_BPS
reach = current * step // 10_000
price = max(current - reach, min(current + reach, target))
print(price, abs(target - price) * 10_000 // current)
")
: "${GAP_BPS:?calcul du pas en echec}"

echo "bce date=$SOURCE_DATE prix=$TARGET ; en place=$CURRENT ; publie=$PRICE"
stellar contract invoke --id "$STRATEGY" --source "$KEY" --network $NETWORK -- \
  set_reference_price --caller "$ADDR" --price "$PRICE" --source_date "$SOURCE_DATE"

if [ "$PRICE" != "$TARGET" ]; then
  echo "ATTENTION : taux du jour a plus d'un pas ; ecart restant ${GAP_BPS} pb."
  echo "Relancer dans une heure jusqu'a ecart nul, puis realigner le pool"
  echo "(scripts/align_aquarius_pool.sh) avant tout mouvement Aquarius."
fi

#!/usr/bin/env bash
# Sequence du livrable D5 en ligne de commande (plan D5 section 13), pour
# l'ops : repetition de la seance filmee, ou preuve rejouee apres un reset du
# testnet. La video, elle, se tourne dans le navigateur (brief
# docs/plans/2026-10-23-d5-tournage.md).
#
# Deux cles : un deposant et la cle keeper du vault (fy-ops-bot).
#
# Six etapes, une pause avant chacune pour laisser le temps de cadrer :
#   1. depot de 1 USDC, reste oisif ;
#   2. reequilibrage : Blend 70 %, Aquarius 20 %, oisif 10 % ;
#   3. depot de 0,05 USDC, reequilibrage refuse (aucune poche a plus de
#      5 points de sa cible) ;
#   4. depot de 0,2 USDC, reequilibrage accepte ;
#   5. retrait de 0,15 USDC, servi par l'oisif ;
#   6. retrait de 1 USDC, complete par Blend.
# Apres chaque etape, la repartition lue sur la chaine. Le pool Aquarius est
# realigne sur le prix de reference avant chaque reequilibrage (plan D5
# section 5) : le script n'echange rien si l'ecart est sous 0,1 %.
#
# Valeurs calculees pour un vault qui detient deja 2 USDC d'ops, repartis ou
# non : 3 USDC apres l'etape 1, ecart de 1,5 point a l'etape 3 et de 6,9 points
# a l'etape 4, pour un seuil de 5.
#
# Chaque transaction est journalisee dans d5-demo-<horodatage>.log, a envoyer
# a l'ops le jour meme pour archivage (scripts/archive_proof.sh).
#
# Usage : scripts/d5_demo_sequence.sh <cle_deposant> <cle_keeper> [etape_de_depart]
set -euo pipefail

DEPOSITOR="${1:?usage: d5_demo_sequence.sh <cle_deposant> <cle_keeper> [etape]}"
KEEPER="${2:?cle keeper du vault}"
# Le realignement echange sur le pool : il faut une cle qui detient USDC et
# FYEUR, ce qui n'est pas le cas de la cle keeper.
ALIGN_KEY="${ALIGN_KEY:-d1-ops}"
START="${3:-1}"
NETWORK=testnet
VAULT=CBESWJ4CXLWEFU33HEEMJFT4SAYV5PW6LPEBAYRQBAPKIGRFSVBWXVOV
AQUARIUS=CACZE525I6OY7ASL2UUSHCWY45QA4PXZ3BB3ZEUT6UUUYCFHDVQESEDF
# Code d'erreur du vault : aucune poche hors de sa marge.
NOTHING_TO_REBALANCE='Error(Contract, #18)'
# Marge exigee sur l'age du prix de reference avant de commencer, en s.
PRICE_MARGIN=7200
HERE="$(cd "$(dirname "$0")" && pwd)"
LOG="d5-demo-$(date -u +%Y%m%dT%H%M%SZ).log"

FROM=$(stellar keys address "$DEPOSITOR")
CALLER=$(stellar keys address "$KEEPER")

read_vault() {
  stellar contract invoke --id "$VAULT" --source "$DEPOSITOR" --network $NETWORK --send=no -- "$@"
}

# Unites a 7 decimales -> USDC lisible.
usdc() {
  python3 -c "import sys; print(f'{int(sys.argv[1]) / 10**7:.7f}')" "$1"
}

# Repartition courante, lue sur la chaine.
show_state() {
  local allocation total shares
  allocation=$(read_vault allocation)
  total=$(read_vault total_assets | tr -d '"')
  shares=$(read_vault shares_of --owner "$FROM" | tr -d '"')
  python3 - "$allocation" "$total" "$shares" <<'PY'
import json, sys
allocation, total, shares = json.loads(sys.argv[1]), int(sys.argv[2]), int(sys.argv[3])
names = {"CCDZZ4NXLWFXWIJIFSOG37K3NIMITZPMUJAHLELRSHO7XO5VA5ZYDEV5": "Blend",
         "CACZE525I6OY7ASL2UUSHCWY45QA4PXZ3BB3ZEUT6UUUYCFHDVQESEDF": "Aquarius"}
pockets = [(names.get(a, a), int(v)) for a, v in allocation]
pockets.append(("Oisif", total - sum(v for _, v in pockets)))
targets = {"Blend": 70, "Aquarius": 20, "Oisif": 10}
print(f"  Vault {total / 10**7:.7f} USDC")
for name, value in pockets:
    pct = value * 100 / total if total else 0
    print(f"    {name:<9}{value / 10**7:>11.7f} USDC  {pct:5.1f} %  (cible {targets[name]} %, ecart {pct - targets[name]:+.1f} pts)")
print(f"  Parts du deposant : {shares}")
PY
}

# Execute une commande, sortie complete au journal ; arret au premier echec,
# pour ne jamais enchainer sur un etat faux. Rend la sortie.
logged() {
  local label="$1" out; shift
  echo "== $label" >> "$LOG"
  if ! out=$("$@" 2>&1); then
    echo "$out" >> "$LOG"
    echo "  ECHEC : $label, voir $LOG" >&2
    exit 1
  fi
  echo "$out" >> "$LOG"
  printf '%s\n' "$out"
}

# Transaction signee : events du vault, retour et lien a l'ecran.
send() {
  local label="$1" key="$2" out; shift 2
  out=$(logged "$label" stellar contract invoke --id "$VAULT" --source "$key" --network $NETWORK -- "$@")
  printf '%s\n' "$out" | grep -E "stellar.expert|Event: [A-Z]|^\"|^\[" || true
}

pause() {
  echo
  echo "### Etape $1 : $2"
  read -r -p "Entree pour lancer... " _
}

align_pool() {
  echo "  Realignement du pool Aquarius sur le prix de reference"
  local out
  out=$(logged "alignement du pool" "$HERE/align_aquarius_pool.sh" "$ALIGN_KEY" "$AQUARIUS")
  printf '%s\n' "$out" | grep -E "ecart|rien a faire|stellar.expert" || true
}

rebalance() {
  align_pool
  send "reequilibrage" "$KEEPER" rebalance --caller "$CALLER"
}

# Retrait d'un montant en USDC : parts necessaires, arrondies au-dessus,
# au prix de la part lu juste avant.
withdraw_usdc() {
  local amount="$1" total_assets total_shares shares
  total_assets=$(read_vault total_assets | tr -d '"')
  total_shares=$(read_vault total_shares | tr -d '"')
  shares=$(python3 -c "import sys; a, ta, ts = map(int, sys.argv[1:]); print(-(-a * ts // ta))" \
    "$amount" "$total_assets" "$total_shares")
  send "retrait de $(usdc "$amount") USDC ($shares parts)" "$DEPOSITOR" \
    withdraw --from "$FROM" --shares "$shares"
}

# Pre-vol : vault actif et prix de reference valable pour toute la seance.
[ "$(read_vault is_paused)" = false ] || { echo "vault en pause : arret"; exit 1; }
UPDATED_AT=$(stellar contract invoke --id "$AQUARIUS" --source "$DEPOSITOR" --network $NETWORK --send=no -- \
  reference_price | python3 -c "import json,sys; print(json.load(sys.stdin)['updated_at'])")
MAX_AGE=$(stellar contract invoke --id "$AQUARIUS" --source "$DEPOSITOR" --network $NETWORK --send=no -- \
  config | python3 -c "import json,sys; print(json.load(sys.stdin)['max_age'])")
LEFT=$(( UPDATED_AT + MAX_AGE - $(date +%s) ))
if [ "$LEFT" -lt "$PRICE_MARGIN" ]; then
  echo "prix de reference perime dans ${LEFT} s : demander a l'ops de le republier avant de filmer"
  exit 1
fi

: > "$LOG"
echo "Deposant $FROM"
echo "Keeper   $CALLER"
echo "Journal  $LOG"
show_state

if [ "$START" -le 1 ]; then
  pause 1 "depot de 1 USDC"
  send "depot de 1 USDC" "$DEPOSITOR" deposit --from "$FROM" --amount 10000000
  show_state
fi

if [ "$START" -le 2 ]; then
  pause 2 "reequilibrage vers 70 / 20 / 10"
  rebalance
  show_state
fi

if [ "$START" -le 3 ]; then
  pause 3 "depot de 0,05 USDC, puis reequilibrage"
  send "depot de 0,05 USDC" "$DEPOSITOR" deposit --from "$FROM" --amount 500000
  show_state
  echo "  Tentative de reequilibrage"
  echo "== reequilibrage (refus attendu)" >> "$LOG"
  # Le refus survient a la simulation : la CLI n'envoie rien, aucun hash.
  if OUT=$(stellar contract invoke --id "$VAULT" --source "$KEEPER" --network $NETWORK -- \
        rebalance --caller "$CALLER" 2>&1); then
    echo "$OUT" >> "$LOG"
    echo "  INATTENDU : le reequilibrage est passe"
    exit 1
  fi
  echo "$OUT" >> "$LOG"
  if [[ "$OUT" == *"$NOTHING_TO_REBALANCE"* ]]; then
    echo "  Refuse : NothingToRebalance (erreur 18), aucune poche a plus de 5 points de sa cible"
  else
    echo "  Echec inattendu, voir $LOG"
    exit 1
  fi
fi

if [ "$START" -le 4 ]; then
  pause 4 "depot de 0,2 USDC, puis reequilibrage"
  send "depot de 0,2 USDC" "$DEPOSITOR" deposit --from "$FROM" --amount 2000000
  show_state
  rebalance
  show_state
fi

if [ "$START" -le 5 ]; then
  pause 5 "retrait de 0,15 USDC, servi par l'oisif"
  withdraw_usdc 1500000
  show_state
fi

if [ "$START" -le 6 ]; then
  pause 6 "retrait de 1 USDC, complete par Blend"
  withdraw_usdc 10000000
  show_state
fi

echo
echo "Sequence terminee. Hashes a archiver :"
grep -oE "tx/[0-9a-f]{64}" "$LOG" | cut -c4- | sort -u || true

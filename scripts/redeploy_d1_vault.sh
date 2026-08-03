#!/usr/bin/env bash
# Redeploiement de l'instance d'evidence du Deliverable 1 sur testnet.
#
# Pourquoi : le premier reflexe d'un reviewer technique est de comparer
# l'empreinte du wasm en ligne au code du depot. Des que le contrat evolue,
# l'instance d'evidence doit suivre, sinon la verification de dix secondes
# devient un paragraphe de justification. Ce script produit une instance neuve
# issue du main publie, avec les transactions de preuve exigees par la Measure :
# initialize, deposit, withdraw.
#
# Rejouable apres chaque reset du testnet SDF (2-4x/an) et apres tout
# changement du contrat.
#
# Usage : scripts/redeploy_d1_vault.sh <cle> [ancien_vault_id]
#
# `ancien_vault_id` vaut par defaut l'instance d'evidence COURANTE, celle que le
# README et docs/evidence/d1-vault-mvp.md designent : c'est elle qu'il faut
# vider, sous peine d'y laisser les fonds. Tenir cette valeur a jour a chaque
# redeploiement, elle change a chaque fois.
#
# Variables d'environnement :
#   SKIP_DRAIN=1     ne pas vider l'ancienne instance (deja fait, ou instance
#                    absente apres un reset du testnet)
#   REUSE_VAULT=C... reprendre sur un contrat DEJA deploye et non initialise,
#                    au lieu d'en deployer un neuf. Sert quand l'execution a
#                    casse entre le deploy et l'initialize : la fenetre de
#                    front-run reste ouverte tant que l'initialize n'est pas
#                    passe, donc on reprend, on ne redeploie pas.
#   DEPOSIT_AMOUNT   montant du depot de preuve, en unites brutes 7 decimales
#                    (defaut 1000000000 = 100 USDC, comme l'evidence de juillet)
#   ALLOW_DIRTY=1    autoriser un arbre de travail sale ou une branche autre que
#                    main. A n'utiliser que pour une repetition a blanc : la
#                    raison d'etre du script est de deployer le main publie.
set -euo pipefail

KEY="${1:?usage: redeploy_d1_vault.sh <cle> [ancien_vault_id]}"
OLD_VAULT="${2:-CCE5ITQQF4GWG5FA47D2XJBKXASWJ2E5V5AWW5U5BBAFWIXA77YYGWNI}"
NETWORK=testnet
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEPOSIT_AMOUNT="${DEPOSIT_AMOUNT:-1000000000}"

ADDR=$(stellar keys address "$KEY")
HASHFILE="$(mktemp)"
trap 'rm -f "$HASHFILE"' EXIT

# --- Garde : ce script existe pour deployer le code publie, pas un brouillon.
if [ "${ALLOW_DIRTY:-0}" != "1" ]; then
  BRANCH=$(git -C "$ROOT" rev-parse --abbrev-ref HEAD)
  [ "$BRANCH" = "main" ] || {
    echo "refus : branche $BRANCH, attendu main (ALLOW_DIRTY=1 pour passer outre)" >&2
    exit 1
  }
  [ -z "$(git -C "$ROOT" status --porcelain contracts/)" ] || {
    echo "refus : contracts/ a des modifications non commitees" >&2
    exit 1
  }
  git -C "$ROOT" fetch -q origin main
  [ "$(git -C "$ROOT" rev-parse HEAD)" = "$(git -C "$ROOT" rev-parse origin/main)" ] || {
    echo "refus : HEAD n'est pas origin/main" >&2
    exit 1
  }
fi

# --- Sources canoniques, jamais codees en dur : memes registres que les autres
# scripts du depot. USDC et le pool TestnetV2 viennent tous deux de blend-utils.
REGISTRY=$(curl -sf https://raw.githubusercontent.com/blend-capital/blend-utils/main/testnet.contracts.json)
USDC=$(printf '%s' "$REGISTRY" | python3 -c "import json,sys; print(json.load(sys.stdin)['ids']['USDC'])")
POOL=$(printf '%s' "$REGISTRY" | python3 -c "import json,sys; print(json.load(sys.stdin)['ids']['TestnetV2'])")

echo "admin=$ADDR"
echo "usdc=$USDC"
echo "pool=$POOL"

# Invocation qui consigne le hash de transaction : la CLI l'ecrit sur stderr
# ("Signing transaction: <64 hex>"), et l'evidence les exige un par un.
invoke() {
  local label="$1"
  shift
  local errf out h
  errf=$(mktemp)
  if ! out=$(stellar contract invoke "$@" 2>"$errf"); then
    cat "$errf" >&2
    rm -f "$errf"
    return 1
  fi
  cat "$errf" >&2
  h=$(grep -oE 'Signing transaction: [0-9a-f]{64}' "$errf" | tail -1 | awk '{print $NF}')
  rm -f "$errf"
  printf '%s\t%s\n' "$label" "$h" >>"$HASHFILE"
  printf '%s' "$out"
}

unquote() { tr -d '"'; }

# --- 1. Vider l'ancienne instance ------------------------------------------
# Les parts mortes et leur contre-valeur (1000 unites) restent verrouillees a
# jamais dans l'ancien vault : c'est le verrou anti-inflation, pas une perte
# accidentelle. L'ancienne instance reste en ligne, elle porte les hashes deja
# publies dans les entrees datees de juillet.
if [ "${SKIP_DRAIN:-0}" != "1" ]; then
  OLD_SHARES=$(stellar contract invoke --id "$OLD_VAULT" --source "$KEY" --network $NETWORK --send=no -- \
    shares_of --owner "$ADDR" | unquote)
  echo "ancienne instance : $OLD_SHARES parts detenues par $ADDR"
  if [ "$OLD_SHARES" -gt 0 ]; then
    RETURNED=$(invoke "old_withdraw" --id "$OLD_VAULT" --source "$KEY" --network $NETWORK -- \
      withdraw --from "$ADDR" --shares "$OLD_SHARES" | unquote)
    echo "recupere : $RETURNED unites"
  fi
fi

BALANCE=$(stellar contract invoke --id "$USDC" --source "$KEY" --network $NETWORK --send=no -- \
  balance --id "$ADDR" | unquote)
echo "solde USDC de l'admin : $BALANCE unites"
[ "$BALANCE" -ge "$DEPOSIT_AMOUNT" ] || {
  echo "refus : solde insuffisant pour un depot de preuve de $DEPOSIT_AMOUNT" >&2
  exit 1
}

# --- 2. Build --------------------------------------------------------------
cargo build --manifest-path "$ROOT/Cargo.toml" --target wasm32v1-none --release -p yield-vault
WASM="$ROOT/target/wasm32v1-none/release/yield_vault.wasm"
echo "wasm sha256 local : $(shasum -a 256 "$WASM" | cut -d' ' -f1)"

# --- 3. Fenetre critique : deploy puis initialize, rien entre les deux ------
# Le contrat n'expose pas de __constructor, donc initialize est une transaction
# distincte du deploiement. L'enchainement reduit la fenetre de front-run a
# quelques secondes, il ne la ferme pas : accepte sur testnet et consigne dans
# l'evidence. Depuis le durcissement du 03/08, un initialize adverse pose sur un
# pool sans reserve pour l'actif echoue au lieu de briquer l'adresse.
#
# `pool` est un Option<Address> : depuis la CLI 27, un argument optionnel se
# passe en JSON, donc entre guillemets DANS la valeur. La forme brute, acceptee
# par la CLI 26, rend « Invalid JSON in argument 'pool' » et laisse le contrat
# deploye sans administrateur, fenetre grande ouverte. Ne pas simplifier.
if [ -n "${REUSE_VAULT:-}" ]; then
  VAULT="$REUSE_VAULT"
  echo "reprise sur un contrat deja deploye : $VAULT"
else
  VAULT=$(stellar contract deploy --wasm "$WASM" --source "$KEY" --network $NETWORK)
fi
invoke "initialize" --id "$VAULT" --source "$KEY" --network $NETWORK -- \
  initialize --admin "$ADDR" --asset "$USDC" --pool "\"$POOL\"" >/dev/null
# --- Fin de la fenetre : admin, actif et pool sont fixes, immuables. --------

echo "nouveau vault : $VAULT"

# --- 4. Transactions de preuve ---------------------------------------------
MINTED=$(invoke "deposit" --id "$VAULT" --source "$KEY" --network $NETWORK -- \
  deposit --from "$ADDR" --amount "$DEPOSIT_AMOUNT" | unquote)
echo "parts emises : $MINTED (1000 parts mortes verrouillees au premier depot)"

BURN=$((MINTED * 40 / 100))
RETURNED=$(invoke "withdraw" --id "$VAULT" --source "$KEY" --network $NETWORK -- \
  withdraw --from "$ADDR" --shares "$BURN" | unquote)
echo "retrait de $BURN parts : $RETURNED unites rendues"

# --- 5. Etat final, lu sur la chaine ---------------------------------------
TA=$(stellar contract invoke --id "$VAULT" --source "$KEY" --network $NETWORK --send=no -- total_assets | unquote)
TS=$(stellar contract invoke --id "$VAULT" --source "$KEY" --network $NETWORK --send=no -- total_shares | unquote)
IDLE=$(stellar contract invoke --id "$USDC" --source "$KEY" --network $NETWORK --send=no -- balance --id "$VAULT" | unquote)
WASM_ONCHAIN=$(curl -sf "https://api.stellar.expert/explorer/testnet/contract/$VAULT" |
  python3 -c "import json,sys; print(json.load(sys.stdin).get('wasm',''))" 2>/dev/null || echo "")

echo
echo "================ A CONSIGNER DANS docs/evidence/d1-vault-mvp.md ================"
echo "Contract ID   : $VAULT"
echo "Asset (USDC)  : $USDC"
echo "Pool (Blend)  : $POOL"
echo "Wasm on-chain : ${WASM_ONCHAIN:-<indisponible, relire plus tard>}"
echo "Commit        : $(git -C "$ROOT" rev-parse --short HEAD)"
echo
echo "Hashes :"
cat "$HASHFILE"
echo
echo "Etat final : total_assets=$TA total_shares=$TS solde_oisif=$IDLE"
echo "Explorateur : https://stellar.expert/explorer/testnet/contract/$VAULT"
echo "================================================================================"
echo
echo "Verification d'archivage a refaire dans sept jours si l'instance n'a recu"
echo "aucune transaction : le champ ext d'une simulation de deposit doit rester"
echo "\"v0\". Le contrat prolonge desormais lui-meme, donc toute transaction"
echo "repousse l'echeance a 120 jours."

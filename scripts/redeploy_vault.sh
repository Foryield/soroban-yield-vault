#!/usr/bin/env bash
# Redeploiement d'une instance d'evidence du vault sur testnet.
#
# Pourquoi : le premier reflexe d'un reviewer technique est de comparer
# l'empreinte du wasm en ligne au code du depot. Des que le contrat evolue,
# l'instance d'evidence doit suivre, sinon la verification de dix secondes
# devient un paragraphe de justification. Ce script produit une instance neuve
# issue du main publie, avec les transactions de preuve exigees par la Measure :
# initialize, deposit, withdraw.
#
# Trois instances vivent en parallele, une par profil, et chacune derive du meme
# wasm : seuls l'actif depose et la presence d'un pool changent. Un profil est
# donc un jeu de constantes, jamais une variante de code.
#
# Rejouable apres chaque reset du testnet SDF (2-4x/an) et apres tout
# changement du contrat.
#
# Usage : scripts/redeploy_vault.sh <cle> [ancien_vault_id]
#         VAULT_PROFILE=eurc scripts/redeploy_vault.sh <cle>
#
# `ancien_vault_id` vaut par defaut l'instance d'evidence COURANTE du profil,
# celle que le README et docs/evidence/ designent : c'est elle qu'il faut vider,
# sous peine d'y laisser les fonds. Tenir ces valeurs a jour a chaque
# redeploiement, elles changent a chaque fois.
#
# Variables d'environnement :
#   VAULT_PROFILE    d1 (defaut) | eurc | demo. Voir le bloc de profils.
#   SKIP_DRAIN=1     ne pas vider l'ancienne instance (deja fait, ou instance
#                    absente apres un reset du testnet)
#   REUSE_VAULT=C... reprendre sur un contrat DEJA deploye et non initialise,
#                    au lieu d'en deployer un neuf. Sert quand l'execution a
#                    casse entre le deploy et l'initialize : la fenetre de
#                    front-run reste ouverte tant que l'initialize n'est pas
#                    passe, donc on reprend, on ne redeploie pas.
#   DEPOSIT_AMOUNT   montant du depot de preuve, en unites brutes 7 decimales.
#                    Defaut par profil, aligne sur l'evidence deja publiee.
#   ALLOW_DIRTY=1    autoriser un arbre de travail sale ou une branche autre que
#                    main. A n'utiliser que pour une repetition a blanc : la
#                    raison d'etre du script est de deployer le main publie.
set -euo pipefail

KEY="${1:?usage: redeploy_vault.sh <cle> [ancien_vault_id]  (VAULT_PROFILE=d1|eurc|demo)}"
PROFILE="${VAULT_PROFILE:-d1}"
NETWORK=testnet
ROOT="$(cd "$(dirname "$0")/.." && pwd)"

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

# --- Profils ----------------------------------------------------------------
# Sources canoniques, jamais codees en dur : l'USDC et le pool Blend viennent du
# registre blend-utils, les deux autres actifs sont des SAC deterministes que la
# CLI derive de l'actif Classic. Seul l'emetteur EURC est une constante, parce
# que c'est une identite Circle et non une adresse derivee.
EURC_ISSUER="EURC:GB3Q6QDZYTHWT7E5PVS3W7FUT5GVAFC5KSZFFLPU25GO7VTC3NM2ZTVO"

case "$PROFILE" in
d1)
  REGISTRY=$(curl -sf https://raw.githubusercontent.com/blend-capital/blend-utils/main/testnet.contracts.json)
  ASSET=$(printf '%s' "$REGISTRY" | python3 -c "import json,sys; print(json.load(sys.stdin)['ids']['USDC'])")
  POOL=$(printf '%s' "$REGISTRY" | python3 -c "import json,sys; print(json.load(sys.stdin)['ids']['TestnetV2'])")
  ASSET_LABEL="USDC Blend (SAC)"
  OLD_VAULT="${2:-CCE5ITQQF4GWG5FA47D2XJBKXASWJ2E5V5AWW5U5BBAFWIXA77YYGWNI}"
  DEPOSIT_AMOUNT="${DEPOSIT_AMOUNT:-1000000000}" # 100 USDC, comme l'evidence de juillet
  EVIDENCE="docs/evidence/d1-vault-mvp.md"
  ;;
eurc)
  ASSET=$(stellar contract id asset --asset "$EURC_ISSUER" --network $NETWORK)
  POOL="" # aucun pool de lending EURC sur testnet : garde pure
  ASSET_LABEL="EURC Circle (SAC wrapper)"
  OLD_VAULT="${2:-CDZR2IY4V3GXUONLTVXJNCMTIR2LLFC55ZRPPEHCTI4RM7LVF25UKG5K}"
  DEPOSIT_AMOUNT="${DEPOSIT_AMOUNT:-50000000}" # 5 EURC, comme l'evidence de juillet
  EVIDENCE="docs/evidence/d3-eurc-sac.md"
  ;;
demo)
  ASSET=$(stellar contract id asset --asset native --network $NETWORK)
  POOL="" # XLM natif, aucune strategie : tout compte Friendbot peut deposer
  ASSET_LABEL="XLM natif (SAC)"
  OLD_VAULT="${2:-CCP3EJYJ55RLZYCHABIWCTCWRHQN2BYZVXLCHZLPCCKIKA4VNK6TMCHN}"
  DEPOSIT_AMOUNT="${DEPOSIT_AMOUNT:-10000000}" # 1 XLM
  EVIDENCE="docs/evidence/d2-wallet-onboarding.md"
  ;;
*)
  echo "refus : VAULT_PROFILE=$PROFILE inconnu (attendu d1, eurc ou demo)" >&2
  exit 1
  ;;
esac

echo "profil=$PROFILE"
echo "admin=$ADDR"
echo "actif=$ASSET ($ASSET_LABEL)"
echo "pool=${POOL:-<aucun>}"

# Execution qui consigne le hash de transaction : la CLI l'ecrit sur stderr
# ("Signing transaction: <64 hex>"), et l'evidence les exige un par un. Le
# deploiement passe par ici au meme titre que les invocations : son hash fait
# partie du cycle de vie que la Measure demande, et le run du 04/08 a montre
# qu'il manquait au recapitulatif quand seules les invocations etaient captees.
capture() {
  local label="$1"
  shift
  local errf out h
  errf=$(mktemp)
  if ! out=$("$@" 2>"$errf"); then
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

invoke() {
  local label="$1"
  shift
  capture "$label" stellar contract invoke "$@"
}

unquote() { tr -d '"'; }

# --- 1. Vider l'ancienne instance ------------------------------------------
# Les parts mortes et leur contre-valeur (1000 unites) restent verrouillees a
# jamais dans l'ancien vault : c'est le verrou anti-inflation, pas une perte
# accidentelle. L'ancienne instance reste en ligne, elle porte les hashes deja
# publies dans les entrees datees.
#
# Profil demo : le retrait ne porte QUE sur les parts de l'admin. Les positions
# ouvertes par des tiers depuis la page publique restent sur l'ancienne
# instance, qui reste en ligne pour elles. C'est la contrepartie assumee du
# redeploiement d'une demo publique.
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

BALANCE=$(stellar contract invoke --id "$ASSET" --source "$KEY" --network $NETWORK --send=no -- \
  balance --id "$ADDR" | unquote)
echo "solde de l'admin : $BALANCE unites de $ASSET_LABEL"
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
# L'absence de pool se dit en OMETTANT l'argument, comme les instances EURC et
# demo l'ont toujours fait ; ne pas inventer une forme explicite sans l'avoir
# verifiee sur la version de CLI installee.
INIT_ARGS=(initialize --admin "$ADDR" --asset "$ASSET")
if [ -n "$POOL" ]; then
  # Pas de `[ -n "$POOL" ] && ...` en une ligne : sous set -e, la commande
  # composee renvoie 1 quand le test echoue et tue le script sur les profils
  # justement sans pool.
  INIT_ARGS+=(--pool "\"$POOL\"")
fi

if [ -n "${REUSE_VAULT:-}" ]; then
  VAULT="$REUSE_VAULT"
  echo "reprise sur un contrat deja deploye : $VAULT"
else
  VAULT=$(capture "deploy" stellar contract deploy --wasm "$WASM" --source "$KEY" --network $NETWORK)
fi
invoke "initialize" --id "$VAULT" --source "$KEY" --network $NETWORK -- \
  "${INIT_ARGS[@]}" >/dev/null
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
IDLE=$(stellar contract invoke --id "$ASSET" --source "$KEY" --network $NETWORK --send=no -- balance --id "$VAULT" | unquote)
WASM_ONCHAIN=$(curl -sf "https://api.stellar.expert/explorer/testnet/contract/$VAULT" |
  python3 -c "import json,sys; print(json.load(sys.stdin).get('wasm',''))" 2>/dev/null || echo "")

echo
echo "================ A CONSIGNER DANS $EVIDENCE ================"
echo "Profil        : $PROFILE"
echo "Contract ID   : $VAULT"
echo "Asset         : $ASSET ($ASSET_LABEL)"
echo "Pool (Blend)  : ${POOL:-aucun (garde pure)}"
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
echo "A mettre a jour avec ce Contract ID : $EVIDENCE, le tableau du README, et le"
echo "defaut du profil dans ce script (il sert de cible au vidage suivant, une"
echo "valeur perimee viderait la mauvaise instance)."
if [ "$PROFILE" = "demo" ]; then
  echo
  echo "Profil demo : la page publique pointe encore l'ancienne instance. Mettre a"
  echo "jour NEXT_PUBLIC_VAULT_ID dans render.yaml ET le defaut testnet de"
  echo "web/lib/stellar.ts, puis redeployer la demo, sinon le reviewer depose"
  echo "toujours dans l'ancien contrat."
fi
echo
echo "Verification d'archivage a refaire dans sept jours si l'instance n'a recu"
echo "aucune transaction : le champ ext d'une simulation de deposit doit rester"
echo "\"v0\". Le contrat prolonge desormais lui-meme, donc toute transaction"
echo "repousse l'echeance a 120 jours."

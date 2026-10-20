# Wasm vendorisé : stratégie Blend de DeFindex

`blend_strategy.wasm` est la stratégie Blend de DeFindex, compilée par nous
depuis sa source. Le vault D5 l'appelle comme stratégie au trait
`defindex-strategy-core` : les tests d'intégration du vault l'importent par
`contractimport!`, et le déploiement testnet de D5 déploie ce même binaire.

## Provenance

- Dépôt source : [`defindex-io/stellar-contracts`](https://github.com/defindex-io/stellar-contracts),
  crate `strategies/blend`.
- Commit épinglé : `82079667253622aa125eeb626fabbf0e47fe5cda`.
- Compilé le 2026-10-20 avec stellar-cli 27.1.0 et rustc 1.94.0, cible
  `wasm32v1-none`, `soroban-sdk` 22.0.6 (verrouillé par leur `Cargo.lock`).
- Build reproductible : deux builds indépendants donnent la même empreinte.

Empreinte SHA-256 de `blend_strategy.wasm` :
`5dcc905bbeddfc94046bfd26346146e08ef587c3bc061bfb1b12bd462b64a8fb`.

L'empreinte est vérifiée à chaque exécution des tests
(`vendored_wasms_match_sha256sums`, `src/test_blend.rs`).
`scripts/build_blend_strategy.sh` reconstruit le wasm depuis la source
épinglée et vérifie qu'il est identique à cette copie.

## Licence

Le dépôt DeFindex déclare la licence GPL-3.0 dans son `Cargo.toml`. Nous ne
copions pas sa source dans ce dépôt : seul le binaire compilé est vendorisé,
et sa source correspondante est celle du commit ci-dessus. La stratégie
reste un contrat distinct, que le vault appelle par son interface publique.

## Usage sans Soroswap

Le constructeur de la stratégie attend l'adresse d'un routeur Soroswap, qui
ne sert qu'à `harvest`, pour revendre les BLND réclamés. Nous y passons une
adresse qui n'expose pas `router_pair_for` : sous le seuil de récompense,
`harvest` réclame les BLND et s'arrête ; au-dessus, il échoue avant tout
transfert. Soroswap ne peut jamais être appelé (plan D5, section 4).

#!/usr/bin/env python3
"""Genere contracts/vault/src/test_matrix.rs.

Oracle independant de la math du vault : les valeurs attendues sont
calculees ici en Python (division entiere) et emises comme litteraux.
Usage : python3 scripts/generate_matrix_tests.py  (depuis la racine du repo)
"""
deposits = [1_001, 1_500, 2_000, 10_000, 33_333, 100_000, 1_234_567, 999_999_937]
yields = [0, 1, 999, 10_000, 123_457]
fractions = [("all", 1, 1), ("half", 1, 2), ("third", 1, 3), ("twothirds", 2, 3), ("one", 0, 0)]

out = []
out.append("#![cfg(test)]")
out.append("//! FICHIER GENERE - ne pas editer a la main.")
out.append("//! Matrice deposit x rendement x fraction de retrait : chaque valeur")
out.append("//! attendue est un litteral calcule par un oracle Python independant")
out.append("//! (scripts/generate_matrix_tests.py), pas par la formule du contrat.")
out.append("")
out.append("use super::{YieldVault, YieldVaultClient};")
out.append("use soroban_sdk::{")
out.append("    testutils::{Address as _, EnvTestConfig},")
out.append("    token::{StellarAssetClient, TokenClient},")
out.append("    Address, Env,")
out.append("};")
out.append("")
out.append("fn bench(mint: i128) -> (Env, Address, TokenClient<'static>, YieldVaultClient<'static>) {")
out.append("    // Pas de snapshot par test : 200 bancs generes pollueraient le repo.")
out.append("    let env = Env::new_with_config(EnvTestConfig {")
out.append("        capture_snapshot_at_drop: false,")
out.append("    });")
out.append("    env.mock_all_auths();")
out.append("    let admin = Address::generate(&env);")
out.append("    let user = Address::generate(&env);")
out.append("    let asset = env.register_stellar_asset_contract_v2(admin.clone()).address();")
out.append("    StellarAssetClient::new(&env, &asset).mint(&user, &mint);")
out.append("    let vault_id = env.register(YieldVault, ());")
out.append("    let vault = YieldVaultClient::new(&env, &vault_id);")
out.append("    vault.initialize(&admin, &asset, &None);")
out.append("    (env.clone(), user, TokenClient::new(&env, &asset), vault)")
out.append("}")

count = 0
for dep in deposits:
    for yld in yields:
        for (fname, num, den) in fractions:
            shares_user = dep - 1_000
            total_shares = dep
            total_assets = dep + yld
            wshares = 1 if den == 0 else max(1, shares_user * num // den)
            amount = wshares * total_assets // total_shares
            shares_after = shares_user - wshares
            total_after = total_shares - wshares
            assets_after = total_assets - amount
            claim_after = shares_after * assets_after // total_after
            name = f"m_d{dep}_y{yld}_{fname}"
            binding = "(env, user, token, vault)" if yld else "(_env, user, _token, vault)"
            donate = (
                "StellarAssetClient::new(&env, &token.address).mint(&vault.address, &"
                + str(yld) + ");"
            ) if yld else "// pas de rendement simule"
            out.append(f"""
#[test]
fn {name}() {{
    let {binding} = bench(2_000_000_000);
    assert_eq!(vault.deposit(&user, &{dep}), {shares_user});
    {donate}
    assert_eq!(vault.total_assets(), {total_assets});
    assert_eq!(vault.withdraw(&user, &{wshares}), {amount});
    assert_eq!(vault.shares_of(&user), {shares_after});
    assert_eq!(vault.total_shares(), {total_after});
    assert_eq!(vault.total_assets(), {assets_after});
    // Solvabilite relue depuis le contrat (l'oracle attend {claim_after}).
    let claim = vault.shares_of(&user) * vault.total_assets() / vault.total_shares();
    assert!(claim <= vault.total_assets());
    assert_eq!(claim, {claim_after});
}}""")
            count += 1

open('contracts/vault/src/test_matrix.rs', 'w').write("\n".join(out) + "\n")
print(f"{count} tests generes")

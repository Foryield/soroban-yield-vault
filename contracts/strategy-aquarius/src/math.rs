//! Arithmetique entiere de la strategie : racine carree et montant de swap
//! optimal pour une entree a un seul actif. Fonctions pures, `None` sur
//! depassement : l'appelant le traduit en erreur typee.

/// Racine carree entiere, arrondie vers le bas (methode de Newton).
pub fn isqrt(n: u128) -> u128 {
    if n < 2 {
        return n;
    }
    // Point de depart au-dessus de la racine : 2^(ceil(bits/2)).
    let bits = 128 - n.leading_zeros();
    let mut x: u128 = 1u128 << bits.div_ceil(2);
    loop {
        let y = (x + n / x) / 2;
        if y >= x {
            return x;
        }
        x = y;
    }
}

/// Part de `amount` a swapper d'un actif vers l'autre avant de deposer les
/// deux jambes dans un pool a produit constant de reserve `reserve_in` sur
/// l'actif d'entree, frais `fee_bps` preleves a l'entree du swap. C'est la
/// solution de l'equation du « zap » Uniswap V2 :
///
/// s = (sqrt(R^2 (2F - f)^2 + 4 (F - f) F R A) - R (2F - f)) / (2 (F - f))
///
/// avec F = 10 000, soit un peu plus de la moitie de `amount` (environ
/// A / (2 - f) pour un petit montant) : les frais rendent l'autre jambe plus
/// chere. Apres ce swap, le reste de l'actif d'entree et le produit
/// du swap sont dans le rapport des reserves : le depot n'en laisse presque
/// rien. Le pool Aquarius preleve ses frais a la sortie et non a l'entree :
/// l'ecart laisse une poussiere oisive, comptee dans la valeur.
pub fn zap_swap_amount(reserve_in: u128, amount: u128, fee_bps: u128) -> Option<u128> {
    const F: u128 = 10_000;
    // Au-dela de 1e14 unites, reserve et montant sont reduits d'un meme
    // facteur : le carre du terme principal tient alors en u128. La
    // precision perdue (le facteur, en unites) est une poussiere, laissee
    // oisive et comptee dans la valeur.
    const MAX_EXACT: u128 = 100_000_000_000_000;
    if reserve_in == 0 || amount == 0 || fee_bps >= F {
        return Some(0);
    }
    let scale = reserve_in.max(amount) / MAX_EXACT + 1;
    let (reserve_in, amount) = (reserve_in / scale, amount / scale);
    if reserve_in == 0 || amount == 0 {
        return Some(0);
    }
    let two_f_minus_fee = 2 * F - fee_bps;
    let f_minus_fee = F - fee_bps;
    let a = reserve_in.checked_mul(two_f_minus_fee)?;
    let a2 = a.checked_mul(a)?;
    let b = (4 * f_minus_fee * F)
        .checked_mul(reserve_in)?
        .checked_mul(amount)?;
    let root = isqrt(a2.checked_add(b)?);
    (root.saturating_sub(a) / (2 * f_minus_fee)).checked_mul(scale)
}

#[cfg(test)]
// Montants en convention Stellar 7 decimales (X_XXXXXXX), comme les tests du
// vault.
#[allow(clippy::inconsistent_digit_grouping)]
mod tests {
    use super::*;

    #[test]
    fn isqrt_is_exact_on_squares_and_floors_otherwise() {
        for n in [0u128, 1, 2, 3, 4, 15, 16, 17, 1 << 64, u64::MAX as u128] {
            let r = isqrt(n);
            assert!(r * r <= n, "{n}");
            assert!((r + 1) * (r + 1) > n, "{n}");
        }
        assert_eq!(isqrt(u128::MAX), u64::MAX as u128);
    }

    #[test]
    fn zap_swaps_a_bit_more_than_half() {
        // Petit montant devant la reserve : environ A / (2 - f), un peu plus
        // de la moitie, l'impact du swap retranchant une poussiere.
        let s = zap_swap_amount(1_000_0000000, 2_0000000, 30).unwrap();
        assert!(s > 1_0000000 && s < 1_0020000, "{s}");
        assert_eq!(zap_swap_amount(0, 1, 30), Some(0));
        // Reserve de 1e18 unites : pas de depassement, meme proportion.
        let big = zap_swap_amount(1_000_000_000_000_000_000, 2_000_000_000_000_000, 30).unwrap();
        let ratio = big * 1_000_000 / 2_000_000_000_000_000;
        assert!((500_000..501_000).contains(&ratio), "{ratio}");
        assert_eq!(zap_swap_amount(1, 0, 30), Some(0));
    }
}

//! Conversions pures i128 <-> u128 de la frontiere Aquarius (montants u128
//! cote venue, i128 cote routeur). Retour Option : la frontiere ne panique
//! jamais, l'appelant (`aqua::attempt`) traduit `None` en `false` et laisse
//! le fallback decider. Testees aux bornes SANS contrat dans la boucle :
//! preuve directe de la garde promise par la re-revue de Task 3.

/// i128 -> u128 : `None` si negatif.
pub fn u128_from_i128(value: i128) -> Option<u128> {
    u128::try_from(value).ok()
}

/// u128 -> i128 : `None` au-dela de i128::MAX.
pub fn i128_from_u128(value: u128) -> Option<i128> {
    i128::try_from(value).ok()
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn u128_from_i128_rejects_negative() {
        assert_eq!(u128_from_i128(-1), None);
    }

    #[test]
    fn u128_from_i128_accepts_zero() {
        assert_eq!(u128_from_i128(0), Some(0));
    }

    #[test]
    fn i128_max_roundtrips() {
        let up = u128_from_i128(i128::MAX);
        assert_eq!(up, Some(i128::MAX as u128));
        assert_eq!(i128_from_u128(up.unwrap()), Some(i128::MAX));
    }

    #[test]
    fn i128_from_u128_rejects_above_i128_max() {
        assert_eq!(i128_from_u128((i128::MAX as u128) + 1), None);
    }
}

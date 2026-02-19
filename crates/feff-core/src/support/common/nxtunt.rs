pub fn nxtunt<F>(start_unit: i32, mut is_open: F) -> i32
where
    F: FnMut(i32) -> bool,
{
    let mut unit = start_unit.max(1) - 1;

    loop {
        unit += 1;
        if unit == 5 || unit == 6 {
            unit = 7;
        }
        if !is_open(unit) {
            return unit;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::nxtunt;
    use std::collections::BTreeSet;

    #[test]
    fn nxtunt_starts_at_one_when_requested_start_is_non_positive() {
        let unit = nxtunt(0, |_| false);
        assert_eq!(unit, 1);
    }

    #[test]
    fn nxtunt_skips_reserved_units() {
        let unit = nxtunt(5, |_| false);
        assert_eq!(unit, 7);
    }

    #[test]
    fn nxtunt_returns_first_unopened_unit() {
        let open: BTreeSet<i32> = [1, 2, 3, 4, 7, 8].into_iter().collect();
        let unit = nxtunt(1, |candidate| open.contains(&candidate));
        assert_eq!(unit, 9);
    }
}

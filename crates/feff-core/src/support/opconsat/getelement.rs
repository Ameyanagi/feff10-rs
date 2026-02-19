use crate::support::common::pertab::atsym;

pub fn getelement(iz: usize) -> Option<&'static str> {
    atsym(iz)
}

#[cfg(test)]
mod tests {
    use super::getelement;

    #[test]
    fn resolves_known_element_symbols() {
        assert_eq!(getelement(1), Some("H"));
        assert_eq!(getelement(29), Some("Cu"));
        assert_eq!(getelement(92), Some("U"));
    }

    #[test]
    fn returns_none_for_out_of_range_atomic_numbers() {
        assert_eq!(getelement(0), None);
        assert_eq!(getelement(500), None);
    }
}

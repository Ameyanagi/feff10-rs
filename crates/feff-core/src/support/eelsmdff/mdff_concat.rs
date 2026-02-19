pub fn mdff_concat(str1: &str, str2: &str) -> (String, usize) {
    let mut combined = String::with_capacity(str1.len() + str2.len());
    combined.push_str(str1.trim_end_matches(' '));
    combined.push_str(str2);

    let length = combined.trim_end_matches(' ').len();
    (combined, length)
}

#[cfg(test)]
mod tests {
    use super::mdff_concat;

    #[test]
    fn trims_first_operand_before_concatenation() {
        let (combined, length) = mdff_concat("xmu   ", ".dat  ");
        assert_eq!(combined, "xmu.dat  ");
        assert_eq!(length, 7);
    }

    #[test]
    fn preserves_non_blank_prefix() {
        let (combined, length) = mdff_concat("opconsKK", "10.dat");
        assert_eq!(combined, "opconsKK10.dat");
        assert_eq!(length, combined.len());
    }

    #[test]
    fn blank_suffix_is_not_counted_in_returned_length() {
        let (combined, length) = mdff_concat("abc", "   ");
        assert_eq!(combined, "abc   ");
        assert_eq!(length, 3);
    }
}

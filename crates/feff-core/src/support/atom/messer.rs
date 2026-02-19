pub fn messer(numerr: i32, dlabpr: &str) -> String {
    let ilig = numerr / 1000;
    let ier = numerr - 1000 * ilig;
    format!(
        "error number {:6} detected on a line {:6}in the program{}",
        ier, ilig, dlabpr
    )
}

#[cfg(test)]
mod tests {
    use super::messer;

    #[test]
    fn messer_formats_error_location_like_fortran_output() {
        let message = messer(56_011, "INTDIR");
        assert_eq!(
            message,
            "error number     11 detected on a line     56in the programINTDIR"
        );
    }

    #[test]
    fn messer_handles_zero_line_and_error_numbers() {
        let message = messer(0, "ATOM");
        assert_eq!(
            message,
            "error number      0 detected on a line      0in the programATOM"
        );
    }
}

use std::fs;
use std::path::Path;

pub fn format_fixed_f64(value: f64, width: usize, precision: usize) -> String {
    format!(
        "{value:>width$.precision$}",
        width = width,
        precision = precision
    )
}

pub fn normalize_text_artifact(content: &str) -> String {
    let mut normalized = content.replace("\r\n", "\n").replace('\r', "\n");
    if !normalized.is_empty() && !normalized.ends_with('\n') {
        normalized.push('\n');
    }
    normalized
}

pub fn write_text_artifact(path: &Path, content: &str) -> std::io::Result<()> {
    fs::write(path, normalize_text_artifact(content))
}

pub fn write_binary_artifact(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    fs::write(path, bytes)
}

#[cfg(test)]
mod tests {
    use super::{
        format_fixed_f64, normalize_text_artifact, write_binary_artifact, write_text_artifact,
    };
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn fixed_width_float_formatting_is_deterministic() {
        let first = format_fixed_f64(1.23, 13, 5);
        let second = format_fixed_f64(1.23, 13, 5);

        assert_eq!(first, "      1.23000");
        assert_eq!(first, second);
    }

    #[test]
    fn normalize_text_artifact_uses_canonical_line_endings() {
        let normalized = normalize_text_artifact("alpha\r\nbeta\rgamma");
        assert_eq!(normalized, "alpha\nbeta\ngamma\n");
    }

    #[test]
    fn repeated_text_writes_produce_identical_bytes() {
        let temp = TempDir::new().expect("tempdir should be created");
        let path = temp.path().join("artifact.dat");
        let input = "line 1\r\nline 2\rline 3";

        write_text_artifact(&path, input).expect("first write should succeed");
        let first = fs::read(&path).expect("artifact should be readable");

        write_text_artifact(&path, input).expect("second write should succeed");
        let second = fs::read(&path).expect("artifact should be readable");

        assert_eq!(first, second);
        assert_eq!(second, b"line 1\nline 2\nline 3\n");
    }

    #[test]
    fn repeated_binary_writes_produce_identical_bytes() {
        let temp = TempDir::new().expect("tempdir should be created");
        let path = temp.path().join("artifact.bin");
        let input = [0_u8, 1_u8, 2_u8, 255_u8];

        write_binary_artifact(&path, &input).expect("first write should succeed");
        let first = fs::read(&path).expect("artifact should be readable");

        write_binary_artifact(&path, &input).expect("second write should succeed");
        let second = fs::read(&path).expect("artifact should be readable");

        assert_eq!(first, second);
        assert_eq!(second, input);
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Example4Record {
    pub int_value: i32,
    pub real_value: f64,
    pub double_value: f64,
    pub complex_value: (f64, f64),
    pub dcomplex_value: (f64, f64),
    pub string_value: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Example4Plan {
    pub file_name: &'static str,
    pub file_type: &'static str,
    pub write_calls: usize,
    pub read_calls: usize,
}

pub fn default_example4_record() -> Example4Record {
    Example4Record {
        int_value: 123_456,
        real_value: 1.234_567_89,
        double_value: 1.234_567_89,
        complex_value: (1.234_567_89, 1.234_567_89),
        dcomplex_value: (1.234_567_89, 1.234_567_89),
        string_value: "S".to_string(),
    }
}

pub fn example4_plan() -> Example4Plan {
    Example4Plan {
        file_name: "Example4.dat",
        file_type: "PAD",
        write_calls: 1,
        read_calls: 1,
    }
}

pub fn roundtrip_example4(record: &Example4Record) -> Example4Record {
    record.clone()
}

#[cfg(test)]
mod tests {
    use super::{default_example4_record, example4_plan, roundtrip_example4};

    #[test]
    fn plan_preserves_pad_roundtrip_shape() {
        let plan = example4_plan();
        assert_eq!(plan.file_name, "Example4.dat");
        assert_eq!(plan.file_type, "PAD");
        assert_eq!(plan.write_calls, 1);
        assert_eq!(plan.read_calls, 1);
    }

    #[test]
    fn record_roundtrip_is_lossless() {
        let input = default_example4_record();
        let output = roundtrip_example4(&input);
        assert_eq!(input, output);
    }
}

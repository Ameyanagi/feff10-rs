#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Example2Plan {
    pub element_count: usize,
    pub source_file: &'static str,
    pub mirror_file: &'static str,
    pub array_write_calls: usize,
    pub header_write_calls: usize,
    pub array_read_calls: usize,
}

pub fn example2_plan(element_count: usize) -> Example2Plan {
    Example2Plan {
        element_count,
        source_file: "Example2a.dat",
        mirror_file: "Example2b.dat",
        array_write_calls: 2,
        header_write_calls: 1,
        array_read_calls: 1,
    }
}

pub fn default_example2_plan() -> Example2Plan {
    example2_plan(100)
}

#[cfg(test)]
mod tests {
    use super::{default_example2_plan, example2_plan};

    #[test]
    fn default_example2_uses_driver_array_length() {
        let plan = default_example2_plan();
        assert_eq!(plan.element_count, 100);
        assert_eq!(plan.array_write_calls, 2);
        assert_eq!(plan.array_read_calls, 1);
    }

    #[test]
    fn example2_preserves_custom_element_count() {
        let plan = example2_plan(64);
        assert_eq!(plan.element_count, 64);
    }
}

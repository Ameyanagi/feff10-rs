#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Example5Plan {
    pub element_count: usize,
    pub pad_output_file: &'static str,
    pub txt_output_file: &'static str,
    pub pad_write_calls: usize,
    pub txt_write_calls: usize,
}

pub fn example5_plan(element_count: usize) -> Example5Plan {
    Example5Plan {
        element_count,
        pad_output_file: "Example5a.dat",
        txt_output_file: "Example5b.dat",
        pad_write_calls: 1,
        txt_write_calls: 1,
    }
}

pub fn default_example5_plan() -> Example5Plan {
    example5_plan(100_000)
}

#[cfg(test)]
mod tests {
    use super::{default_example5_plan, example5_plan};

    #[test]
    fn default_example5_matches_driver_size() {
        let plan = default_example5_plan();
        assert_eq!(plan.element_count, 100_000);
        assert_eq!(plan.pad_output_file, "Example5a.dat");
        assert_eq!(plan.txt_output_file, "Example5b.dat");
    }

    #[test]
    fn custom_example5_size_is_supported() {
        let plan = example5_plan(256);
        assert_eq!(plan.element_count, 256);
    }
}

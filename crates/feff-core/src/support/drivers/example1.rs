#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Example1Plan {
    pub iterations: usize,
    pub source_file: &'static str,
    pub mirror_file: &'static str,
    pub write_calls_to_source: usize,
    pub read_calls_from_source: usize,
    pub write_calls_to_mirror: usize,
}

pub fn example1_plan(iterations: usize) -> Example1Plan {
    Example1Plan {
        iterations,
        source_file: "Example1a.dat",
        mirror_file: "Example1b.dat",
        write_calls_to_source: iterations,
        read_calls_from_source: iterations,
        write_calls_to_mirror: iterations,
    }
}

pub fn default_example1_plan() -> Example1Plan {
    example1_plan(20)
}

#[cfg(test)]
mod tests {
    use super::{default_example1_plan, example1_plan};

    #[test]
    fn default_example1_matches_legacy_loop_count() {
        let plan = default_example1_plan();
        assert_eq!(plan.iterations, 20);
        assert_eq!(plan.write_calls_to_source, 20);
        assert_eq!(plan.read_calls_from_source, 20);
        assert_eq!(plan.write_calls_to_mirror, 20);
    }

    #[test]
    fn example1_supports_custom_iteration_counts() {
        let plan = example1_plan(7);
        assert_eq!(plan.write_calls_to_source, 7);
        assert_eq!(plan.read_calls_from_source, 7);
        assert_eq!(plan.write_calls_to_mirror, 7);
    }
}

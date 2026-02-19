#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TestIoPlan {
    pub data_file: &'static str,
    pub metadata_file: &'static str,
    pub header_only_writes: usize,
    pub scalar_write_calls: usize,
    pub loop_write_rows: usize,
    pub array_write_rows: usize,
    pub scalar_read_calls: usize,
    pub loop_read_rows: usize,
    pub array_read_rows: usize,
}

pub fn build_testio_plan(loop_write_rows: usize, loop_read_rows: usize) -> TestIoPlan {
    TestIoPlan {
        data_file: "TestIOData/WriteData.dat",
        metadata_file: "TestIO.dat",
        header_only_writes: 2,
        scalar_write_calls: 5 + loop_write_rows,
        loop_write_rows,
        array_write_rows: loop_write_rows,
        scalar_read_calls: 2,
        loop_read_rows,
        array_read_rows: loop_write_rows,
    }
}

pub fn default_testio_plan() -> TestIoPlan {
    build_testio_plan(20, 10)
}

#[cfg(test)]
mod tests {
    use super::{build_testio_plan, default_testio_plan};

    #[test]
    fn default_testio_plan_matches_legacy_driver_flow() {
        let plan = default_testio_plan();
        assert_eq!(plan.header_only_writes, 2);
        assert_eq!(plan.scalar_write_calls, 25);
        assert_eq!(plan.loop_write_rows, 20);
        assert_eq!(plan.array_write_rows, 20);
        assert_eq!(plan.scalar_read_calls, 2);
        assert_eq!(plan.loop_read_rows, 10);
        assert_eq!(plan.array_read_rows, 20);
    }

    #[test]
    fn custom_loop_sizes_are_reflected_in_plan() {
        let plan = build_testio_plan(8, 3);
        assert_eq!(plan.scalar_write_calls, 13);
        assert_eq!(plan.loop_write_rows, 8);
        assert_eq!(plan.loop_read_rows, 3);
        assert_eq!(plan.array_read_rows, 8);
    }
}

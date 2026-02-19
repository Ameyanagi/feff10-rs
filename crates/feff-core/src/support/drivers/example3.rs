#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Example3Plan {
    pub rows: usize,
    pub cols: usize,
    pub file_name: &'static str,
    pub file_type: &'static str,
    pub write_2d_calls: usize,
    pub read_2d_calls: usize,
}

pub fn example3_plan(rows: usize, cols: usize) -> Example3Plan {
    Example3Plan {
        rows,
        cols,
        file_name: "Example3.dat",
        file_type: "PAD",
        write_2d_calls: 1,
        read_2d_calls: 1,
    }
}

pub fn default_example3_plan() -> Example3Plan {
    example3_plan(2, 300)
}

#[cfg(test)]
mod tests {
    use super::{default_example3_plan, example3_plan};

    #[test]
    fn default_example3_tracks_driver_dimensions() {
        let plan = default_example3_plan();
        assert_eq!(plan.rows, 2);
        assert_eq!(plan.cols, 300);
        assert_eq!(plan.file_type, "PAD");
    }

    #[test]
    fn example3_allows_other_matrix_shapes() {
        let plan = example3_plan(3, 8);
        assert_eq!(plan.rows, 3);
        assert_eq!(plan.cols, 8);
    }
}

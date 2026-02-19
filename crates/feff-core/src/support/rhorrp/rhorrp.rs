use super::m_density_inp::DensityGrid;

pub fn filename_is_binary(filename: &str) -> bool {
    filename
        .rsplit_once('.')
        .is_some_and(|(_, extension)| extension.eq_ignore_ascii_case("bin"))
}

pub fn next_index(npts: &[usize], idx: &mut [usize]) {
    for (dimension, index_value) in idx.iter_mut().enumerate() {
        let max_points = npts.get(dimension).copied().unwrap_or(1).max(1);
        if *index_value < max_points {
            *index_value += 1;
            return;
        }
        *index_value = 1;
    }
}

pub fn point_at_index(grid: &DensityGrid, idx: &[usize]) -> [f64; 3] {
    let mut point = grid.origin;
    for (dimension, idx_value) in idx.iter().enumerate().take(grid.ndims()) {
        let npts = grid.npts[dimension].max(1);
        if npts <= 1 {
            continue;
        }

        let step = ((*idx_value).saturating_sub(1)) as f64 / (npts - 1) as f64;
        point[0] += grid.axes[dimension][0] * step;
        point[1] += grid.axes[dimension][1] * step;
        point[2] += grid.axes[dimension][2] * step;
    }
    point
}

pub fn line_density_with_broadening(rho: f64, normalized_distance: f64, broadening: f64) -> f64 {
    rho * (1.0 + normalized_distance.abs() * broadening)
}

pub fn iter_grid_points(grid: &DensityGrid) -> GridPointIter<'_> {
    let total_points = grid
        .npts
        .iter()
        .take(grid.ndims())
        .copied()
        .map(|value| value.max(1))
        .product::<usize>();
    GridPointIter {
        grid,
        idx: vec![1; grid.ndims()],
        emitted: 0,
        total_points,
    }
}

pub struct GridPointIter<'a> {
    grid: &'a DensityGrid,
    idx: Vec<usize>,
    emitted: usize,
    total_points: usize,
}

impl Iterator for GridPointIter<'_> {
    type Item = [f64; 3];

    fn next(&mut self) -> Option<Self::Item> {
        if self.emitted >= self.total_points {
            return None;
        }

        let point = point_at_index(self.grid, &self.idx);
        self.emitted += 1;
        if self.emitted < self.total_points {
            next_index(&self.grid.npts, &mut self.idx);
        }
        Some(point)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        filename_is_binary, iter_grid_points, line_density_with_broadening, next_index,
        point_at_index,
    };
    use crate::support::rhorrp::m_density_inp::{DensityCommand, DensityGrid};

    #[test]
    fn binary_extension_detection_is_case_insensitive() {
        assert!(filename_is_binary("density.bin"));
        assert!(filename_is_binary("density.BIN"));
        assert!(!filename_is_binary("density.dat"));
    }

    #[test]
    fn index_iterator_wraps_dimensions_like_fortran_logic() {
        let mut idx = vec![1, 1];
        next_index(&[2, 3], &mut idx);
        assert_eq!(idx, vec![2, 1]);
        next_index(&[2, 3], &mut idx);
        assert_eq!(idx, vec![1, 2]);
    }

    #[test]
    fn point_generation_matches_line_grid() {
        let grid = DensityGrid {
            command: DensityCommand::Line,
            filename: "density.dat".to_string(),
            origin: [-1.0, 0.0, 0.0],
            npts: vec![3],
            axes: vec![[2.0, 0.0, 0.0]],
            core: false,
        };
        assert_eq!(point_at_index(&grid, &[1]), [-1.0, 0.0, 0.0]);
        assert_eq!(point_at_index(&grid, &[2]), [0.0, 0.0, 0.0]);
        assert_eq!(point_at_index(&grid, &[3]), [1.0, 0.0, 0.0]);
    }

    #[test]
    fn grid_point_iterator_visits_all_points() {
        let grid = DensityGrid {
            command: DensityCommand::Plane,
            filename: "density.dat".to_string(),
            origin: [0.0, 0.0, 0.0],
            npts: vec![2, 2],
            axes: vec![[1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            core: false,
        };
        let points = iter_grid_points(&grid).collect::<Vec<_>>();
        assert_eq!(points.len(), 4);
    }

    #[test]
    fn line_density_scales_by_absolute_distance() {
        let value = line_density_with_broadening(2.0, -0.5, 0.3);
        assert!((value - 2.3).abs() <= 1.0e-12);
    }
}

use num_complex::Complex64;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct MdffSpectrum {
    pub s: Vec<Vec<f64>>,
    pub x: Vec<Vec<Complex64>>,
    pub xpart: Vec<Vec<Vec<Complex64>>>,
    pub ne: usize,
}

impl MdffSpectrum {
    pub fn allocate_spectrum_1(&mut self, n: usize, nip: usize) {
        self.ne = n;
        self.s = vec![vec![0.0_f64; 1 + nip]; n];
    }

    pub fn allocate_spectrum_2(&mut self, n: usize, nip: usize, nq: usize) {
        self.ne = n;
        let channels = 1 + nq.saturating_mul(nq);
        self.x = vec![vec![Complex64::new(0.0, 0.0); channels]; n];
        self.xpart = vec![vec![vec![Complex64::new(0.0, 0.0); channels]; nip]; n];
    }

    pub fn channel_count(&self) -> usize {
        self.x.first().map(Vec::len).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::MdffSpectrum;
    use num_complex::Complex64;

    #[test]
    fn allocate_spectrum_1_sets_scalar_buffer_shape() {
        let mut state = MdffSpectrum::default();
        state.allocate_spectrum_1(3, 9);

        assert_eq!(state.ne, 3);
        assert_eq!(state.s.len(), 3);
        assert_eq!(state.s[0].len(), 10);
        assert!(state.s.iter().flatten().all(|value| *value == 0.0));
    }

    #[test]
    fn allocate_spectrum_2_sets_complex_buffer_shape() {
        let mut state = MdffSpectrum::default();
        state.allocate_spectrum_2(2, 9, 2);

        assert_eq!(state.ne, 2);
        assert_eq!(state.channel_count(), 5);
        assert_eq!(state.xpart.len(), 2);
        assert_eq!(state.xpart[0].len(), 9);
        assert_eq!(state.xpart[0][0].len(), 5);
        assert!(
            state
                .x
                .iter()
                .flatten()
                .all(|value| *value == Complex64::new(0.0, 0.0))
        );
    }

    #[test]
    fn second_allocation_replaces_previous_dimensions() {
        let mut state = MdffSpectrum::default();
        state.allocate_spectrum_2(1, 9, 1);
        state.allocate_spectrum_2(4, 9, 3);

        assert_eq!(state.ne, 4);
        assert_eq!(state.channel_count(), 10);
        assert_eq!(state.xpart.len(), 4);
    }
}

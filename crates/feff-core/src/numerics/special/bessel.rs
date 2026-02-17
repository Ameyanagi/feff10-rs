use num_complex::Complex64;

const SERIES_CUTOFF: f64 = 1.0;
const MID_RANGE_CUTOFF: f64 = 7.51;
const SERIES_MAX_ITER: usize = 160;
const SERIES_REL_TOL: f64 = 1.0e-15;
const ASYMPTOTIC_SEED_MAX_ORDER: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SphericalBesselInput {
    pub order: usize,
    pub argument: Complex64,
}

impl SphericalBesselInput {
    pub fn new(order: usize, argument: Complex64) -> Self {
        Self { order, argument }
    }
}

pub trait SphericalBesselApi {
    fn spherical_j(&self, input: SphericalBesselInput) -> Complex64;
    fn spherical_n(&self, input: SphericalBesselInput) -> Complex64;
    fn spherical_h1(&self, input: SphericalBesselInput) -> Complex64;
}

pub fn spherical_j(order: usize, argument: Complex64) -> Complex64 {
    spherical_j_sequence(order, argument)[order]
}

fn spherical_j_sequence(max_order: usize, argument: Complex64) -> Vec<Complex64> {
    assert!(
        argument.re >= 0.0,
        "spherical_j requires Re(z) >= 0 to match FEFF besjh conventions"
    );

    let mut values = vec![Complex64::new(0.0, 0.0); max_order + 1];
    let imag_abs = argument.im.abs();

    if argument.re < SERIES_CUTOFF && imag_abs < SERIES_CUTOFF {
        for (order, value) in values.iter_mut().enumerate() {
            *value = bjnser_j(argument, order);
        }
        return values;
    }

    if argument.re < MID_RANGE_CUTOFF && imag_abs < MID_RANGE_CUTOFF {
        if max_order == 0 {
            values[0] = bjnser_j(argument, 0);
            return values;
        }

        values[max_order] = bjnser_j(argument, max_order);
        values[max_order - 1] = bjnser_j(argument, max_order - 1);

        if max_order >= 2 {
            for order in (0..=(max_order - 2)).rev() {
                let coefficient = (2 * order + 3) as f64;
                values[order] = coefficient * values[order + 1] / argument - values[order + 2];
            }
        }

        return values;
    }

    let (sjl, cjl) = asymptotic_basis(max_order, argument);
    let sin_argument = argument.sin();
    let cos_argument = argument.cos();
    for order in 0..=max_order {
        values[order] = sin_argument * sjl[order] + cos_argument * cjl[order];
    }

    values
}

fn bjnser_j(argument: Complex64, order: usize) -> Complex64 {
    let mut djl = 1.0;
    let mut odd_factor = -1.0;
    for _ in 0..=order {
        odd_factor += 2.0;
        djl *= odd_factor;
    }

    let u = argument * argument * 0.5;
    let mut pj = Complex64::new(1.0, 0.0);
    let mut nf = 1.0;
    let mut nfac = (2 * order + 3) as f64;
    let mut den = nfac;
    let mut sign = -1.0;
    let mut ux = u;

    for _ in 0..SERIES_MAX_ITER {
        let delta = ux * (sign / den);
        pj += delta;
        let rel_delta = if pj.norm() == 0.0 {
            delta.norm()
        } else {
            delta.norm() / pj.norm()
        };
        if rel_delta <= SERIES_REL_TOL {
            return pj * argument.powu(order as u32) / djl;
        }

        sign = -sign;
        ux *= u;
        nf += 1.0;
        nfac += 2.0;
        den = nf * nfac * den;
    }

    panic!("spherical_j series failed to converge for order {order} and argument {argument}");
}

fn asymptotic_basis(max_order: usize, argument: Complex64) -> (Vec<Complex64>, Vec<Complex64>) {
    let xi = Complex64::new(1.0, 0.0) / argument;
    let mut xi_powers = [Complex64::new(0.0, 0.0); 12];
    xi_powers[0] = Complex64::new(1.0, 0.0);
    for power in 1..xi_powers.len() {
        xi_powers[power] = xi_powers[power - 1] * xi;
    }

    let mut sjl = vec![Complex64::new(0.0, 0.0); max_order + 1];
    let mut cjl = vec![Complex64::new(0.0, 0.0); max_order + 1];

    let seeded_max_order = max_order.min(ASYMPTOTIC_SEED_MAX_ORDER);
    for order in 0..=seeded_max_order {
        let (s, c) = asymptotic_seed(order, &xi_powers);
        sjl[order] = s;
        cjl[order] = c;
    }

    if max_order > ASYMPTOTIC_SEED_MAX_ORDER {
        for order in (ASYMPTOTIC_SEED_MAX_ORDER + 1)..=max_order {
            let coefficient = (2 * order - 1) as f64;
            sjl[order] = coefficient * xi * sjl[order - 1] - sjl[order - 2];
            cjl[order] = coefficient * xi * cjl[order - 1] - cjl[order - 2];
        }
    }

    (sjl, cjl)
}

fn asymptotic_seed(order: usize, xi_powers: &[Complex64; 12]) -> (Complex64, Complex64) {
    let xi = xi_powers[1];
    let xi2 = xi_powers[2];
    let xi3 = xi_powers[3];
    let xi4 = xi_powers[4];
    let xi5 = xi_powers[5];
    let xi6 = xi_powers[6];
    let xi7 = xi_powers[7];
    let xi8 = xi_powers[8];
    let xi9 = xi_powers[9];
    let xi10 = xi_powers[10];
    let xi11 = xi_powers[11];

    match order {
        0 => (xi, Complex64::new(0.0, 0.0)),
        1 => (xi2, -xi),
        2 => (3.0 * xi3 - xi, -3.0 * xi2),
        3 => (15.0 * xi4 - 6.0 * xi2, -15.0 * xi3 + xi),
        4 => (105.0 * xi5 - 45.0 * xi3 + xi, -105.0 * xi4 + 10.0 * xi2),
        5 => (
            945.0 * xi6 - 420.0 * xi4 + 15.0 * xi2,
            -945.0 * xi5 + 105.0 * xi3 - xi,
        ),
        6 => (
            10_395.0 * xi7 - 4_725.0 * xi5 + 210.0 * xi3 - xi,
            -10_395.0 * xi6 + 1_260.0 * xi4 - 21.0 * xi2,
        ),
        7 => (
            135_135.0 * xi8 - 62_370.0 * xi6 + 3_150.0 * xi4 - 28.0 * xi2,
            -135_135.0 * xi7 + 17_325.0 * xi5 - 378.0 * xi3 + xi,
        ),
        8 => (
            2_027_025.0 * xi9 - 945_945.0 * xi7 + 51_975.0 * xi5 - 630.0 * xi3 + xi,
            -2_027_025.0 * xi8 + 270_270.0 * xi6 - 6_930.0 * xi4 + 36.0 * xi2,
        ),
        9 => (
            34_459_425.0 * xi10 - 16_216_200.0 * xi8 + 945_945.0 * xi6 - 13_860.0 * xi4
                + 45.0 * xi2,
            -34_459_425.0 * xi9 + 4_729_725.0 * xi7 - 135_135.0 * xi5 + 990.0 * xi3 - xi,
        ),
        10 => (
            654_729_075.0 * xi11 - 310_134_825.0 * xi9 + 18_918_900.0 * xi7 - 315_315.0 * xi5
                + 1_485.0 * xi3
                - xi,
            -654_729_075.0 * xi10 + 91_891_800.0 * xi8 - 2_837_835.0 * xi6 + 25_740.0 * xi4
                - 55.0 * xi2,
        ),
        _ => panic!("missing asymptotic seed for order {order}"),
    }
}

#[cfg(test)]
mod tests {
    use super::spherical_j;
    use num_complex::Complex64;

    #[derive(Clone, Copy)]
    struct ReferenceCase {
        label: &'static str,
        argument: Complex64,
        expected: [Complex64; 9],
        abs_tol: f64,
        rel_tol: f64,
    }

    #[test]
    fn spherical_j_matches_reference_vectors_for_orders_zero_through_eight() {
        let cases = [
            ReferenceCase {
                label: "small",
                argument: Complex64::new(0.5, 0.2),
                expected: [
                    Complex64::new(9.650_373_098_090_16e-1, -3.263_699_410_204_671e-2),
                    Complex64::new(1.644_826_966_487_594_5e-1, 6.198_417_605_376_515e-2),
                    Complex64::new(1.397_838_193_373_662_3e-2, 1.293_577_372_753_651_7e-2),
                    Complex64::new(6.267_121_853_028_758e-4, 1.329_797_687_975_089e-3),
                    Complex64::new(5.098_796_350_238_35e-6, 8.800_161_207_998_259e-5),
                    Complex64::new(-1.376_029_026_871_489_6e-6, 4.096_866_954_196_076e-6),
                    Complex64::new(-1.162_182_418_308_492e-7, 1.364_296_689_680_800_7e-7),
                    Complex64::new(-5.700_051_002_688_189e-9, 2.996_070_123_676_486e-9),
                    Complex64::new(-2.030_414_420_196_256_3e-10, 2.094_805_136_205_157_6e-11),
                ],
                abs_tol: 1.0e-13,
                rel_tol: 1.0e-12,
            },
            ReferenceCase {
                label: "mid",
                argument: Complex64::new(3.0, 1.5),
                expected: [
                    Complex64::new(-1.925_368_741_310_509_4e-1, -6.063_884_575_042_594e-1),
                    Complex64::new(5.289_009_952_188_587e-1, -3.664_186_721_559_523_6e-1),
                    Complex64::new(4.690_902_014_437_569_6e-1, 1.016_931_216_919_539_5e-1),
                    Complex64::new(1.643_480_211_674_532e-1, 1.892_827_001_160_53e-1),
                    Complex64::new(1.435_662_484_380_512_3e-2, 9.824_309_876_838_865e-2),
                    Complex64::new(-1.200_040_302_025_452_7e-2, 2.927_278_711_551_362e-2),
                    Complex64::new(-6.624_385_933_798_433e-3, 5.224_334_533_491_266e-3),
                    Complex64::new(-1.908_621_692_195_178_2e-3, 3.205_082_191_733_88e-4),
                    Complex64::new(-3.690_843_966_355_044e-4, -1.250_582_724_073_569_7e-4),
                ],
                abs_tol: 1.0e-12,
                rel_tol: 1.0e-11,
            },
            ReferenceCase {
                label: "large",
                argument: Complex64::new(12.0, 4.0),
                expected: [
                    Complex64::new(-5.232_467_468_622_953e-1, 2.093_474_303_977_279),
                    Complex64::new(-2.081_294_439_826_153, -3.520_315_365_975_998_4e-1),
                    Complex64::new(2.855_313_265_659_084_6e-2, -2.016_584_316_724_777_5),
                    Complex64::new(1.839_928_824_981_777_3, -4.077_567_237_562_656_5e-1),
                    Complex64::new(8.660_520_738_014_958e-1, 1.480_524_492_380_927),
                    Complex64::new(-9.222_256_643_800_592e-1, 1.212_249_039_508_055),
                    Complex64::new(-1.293_519_761_050_329_5, -2.268_069_770_822_656e-1),
                    Complex64::new(-4.126_683_701_957_485e-1, -1.012_991_919_821_906_8),
                    Complex64::new(4.493_958_746_468_974_5e-1, -7.580_582_938_939_738e-1),
                ],
                abs_tol: 1.0e-12,
                rel_tol: 1.0e-11,
            },
        ];

        for case in cases {
            for (order, expected) in case.expected.iter().enumerate() {
                let actual = spherical_j(order, case.argument);
                assert_complex_close(
                    &format!("{} order={order}", case.label),
                    *expected,
                    actual,
                    case.abs_tol,
                    case.rel_tol,
                );
            }
        }
    }

    fn assert_complex_close(
        label: &str,
        expected: Complex64,
        actual: Complex64,
        abs_tol: f64,
        rel_tol: f64,
    ) {
        let abs_diff = (actual - expected).norm();
        let rel_diff = abs_diff / expected.norm().max(1.0);

        assert!(
            abs_diff <= abs_tol || rel_diff <= rel_tol,
            "{label} expected=({:.15e},{:.15e}) actual=({:.15e},{:.15e}) abs_diff={:.15e} rel_diff={:.15e} abs_tol={:.15e} rel_tol={:.15e}",
            expected.re,
            expected.im,
            actual.re,
            actual.im,
            abs_diff,
            rel_diff,
            abs_tol,
            rel_tol
        );
    }
}

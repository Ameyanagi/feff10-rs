use crate::common::constants::{FA, PI, THIRD, TWO_THIRDS};
use num_complex::Complex64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExchangeModel {
    HedinLundqvist,
    DiracHara,
    VonBarthHedin,
    PerdewZunger,
}

impl ExchangeModel {
    /// Map FEFF `ixc`/`index` values to local exchange-model families.
    pub fn from_feff_ixc(ixc: i32) -> Self {
        match ixc.rem_euclid(10) {
            0 | 5 => Self::HedinLundqvist,
            1 | 3 | 6 => Self::DiracHara,
            2 | 7 => Self::VonBarthHedin,
            4 | 8 => Self::PerdewZunger,
            _ => Self::HedinLundqvist,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExchangeEvaluationInput {
    pub model: ExchangeModel,
    pub electron_density: f64,
    pub energy: f64,
    pub wave_number: f64,
}

impl ExchangeEvaluationInput {
    pub fn new(model: ExchangeModel, electron_density: f64, energy: f64, wave_number: f64) -> Self {
        Self {
            model,
            electron_density,
            energy,
            wave_number,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExchangeEvaluation {
    pub real: f64,
    pub imaginary: f64,
}

impl ExchangeEvaluation {
    pub const ZERO: Self = Self {
        real: 0.0,
        imaginary: 0.0,
    };
}

pub trait ExchangePotentialApi {
    fn evaluate(&self, input: ExchangeEvaluationInput) -> ExchangeEvaluation;
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ExchangePotential;

impl ExchangePotentialApi for ExchangePotential {
    fn evaluate(&self, input: ExchangeEvaluationInput) -> ExchangeEvaluation {
        evaluate_model(input)
    }
}

pub fn evaluate_exchange_potential(input: ExchangeEvaluationInput) -> ExchangeEvaluation {
    ExchangePotential.evaluate(input)
}

const HL_ALPHA: f64 = 4.0 / 3.0;
const HL_MIN_X: f64 = 1.000_01;
const DEFAULT_RS: f64 = 10.0;
const VBH_GAMMA: f64 = 5.129_762_802_484_097;
const SLATER_F: f64 = -0.687_247_939_924_714;
const PZ_A: f64 = 0.0311;
const PZ_B: f64 = -0.048;
const PZ_C: f64 = 0.0020;
const PZ_D: f64 = -0.0116;
const PZ_GC: f64 = -0.1423;
const PZ_B1: f64 = 1.0529;
const PZ_B2: f64 = 0.3334;

const RCFR: [f64; 24] = [
    -0.173_963,
    -0.173_678,
    -0.142_040,
    -0.101_030,
    -0.083_884_3,
    -0.080_704_6,
    -0.135_577,
    -0.177_556,
    -0.064_580_3,
    -0.073_117_2,
    -0.049_882_3,
    -0.039_310_8,
    -0.116_431,
    -0.090_93,
    -0.088_697_9,
    -0.070_231_9,
    0.079_105_1,
    -0.035_940_1,
    -0.037_958_4,
    -0.041_980_7,
    -0.062_816_2,
    0.066_925_7,
    0.066_711_9,
    0.064_817_5,
];

const RCFL: [f64; 48] = [
    0.590_195e2,
    0.478_860e1,
    0.812_813e0,
    0.191_145e0,
    -0.291_180e3,
    -0.926_539e1,
    -0.858_348e0,
    -0.246_947e0,
    0.363_830e3,
    0.460_433e1,
    0.173_067e0,
    0.239_738e-1,
    -0.181_726e3,
    -0.169_709e2,
    -0.409_425e1,
    -0.173_077e1,
    0.886_023e3,
    0.301_808e2,
    0.305_836e1,
    0.743_167e0,
    -0.110_486e4,
    -0.149_086e2,
    -0.662_794e0,
    -0.100_106e0,
    0.184_417e3,
    0.180_204e2,
    0.450_425e1,
    0.184_349e1,
    -0.895_807e3,
    -0.318_696e2,
    -0.345_827e1,
    -0.855_367e0,
    0.111_549e4,
    0.156_448e2,
    0.749_582e0,
    0.117_680e0,
    -0.620_411e2,
    -0.616_427e1,
    -0.153_874e1,
    -0.609_114e0,
    0.300_946e3,
    0.109_158e2,
    0.120_028e1,
    0.290_985e0,
    -0.374_494e3,
    -0.535_127e1,
    -0.261_260e0,
    -0.405_337e-1,
];

fn evaluate_model(input: ExchangeEvaluationInput) -> ExchangeEvaluation {
    if !input.electron_density.is_finite()
        || !input.energy.is_finite()
        || !input.wave_number.is_finite()
    {
        return ExchangeEvaluation::ZERO;
    }

    let rs = electron_density_to_rs(input.electron_density);
    let xk = local_momentum(input.wave_number, input.energy, rs);

    match input.model {
        ExchangeModel::HedinLundqvist => hedin_lundqvist(rs, xk),
        ExchangeModel::DiracHara => ExchangeEvaluation {
            real: dirac_hara(rs, xk),
            imaginary: 0.0,
        },
        ExchangeModel::VonBarthHedin => ExchangeEvaluation {
            real: von_barth_hedin(rs),
            imaginary: 0.0,
        },
        ExchangeModel::PerdewZunger => ExchangeEvaluation {
            real: perdew_zunger(rs),
            imaginary: 0.0,
        },
    }
}

fn electron_density_to_rs(electron_density: f64) -> f64 {
    if electron_density > 0.0 {
        (3.0 / (4.0 * PI * electron_density)).powf(THIRD)
    } else {
        DEFAULT_RS
    }
}

fn local_momentum(wave_number: f64, energy: f64, rs: f64) -> f64 {
    let k_fermi = FA / rs;
    let energy_adjusted = wave_number.abs().powi(2) + 2.0 * energy.max(0.0);
    let momentum = energy_adjusted.sqrt();
    if momentum.is_finite() && momentum > 0.0 {
        momentum
    } else {
        k_fermi * HL_MIN_X
    }
}

fn hedin_lundqvist(rs: f64, xk: f64) -> ExchangeEvaluation {
    let rkf = FA / rs;
    let ef = rkf.powi(2) / 2.0;
    let wp = (3.0 / rs.powi(3)).sqrt();
    let dwp = wp / 3.0;

    let (imaginary, icusp) = imhl(rs, xk);

    let mut xx = xk / rkf;
    if xx < HL_MIN_X {
        xx = HL_MIN_X;
    }
    let deltae = ((xx.powi(2) - 1.0) * ef - wp - dwp) / dwp;

    let mrs = if rs < 0.2 {
        1
    } else if rs < 1.0 {
        2
    } else if rs < 5.0 {
        3
    } else {
        4
    };

    let mut cright = [0.0; 2];
    for j in 1..=2 {
        cright[j - 1] =
            rcfr(mrs, 1, j) * rs + rcfr(mrs, 2, j) * rs * rs.sqrt() + rcfr(mrs, 3, j) * rs.powi(2);
    }

    let eee = -PI * wp / (4.0 * rkf * ef);
    let mut real = 0.0;
    if icusp != 1 || deltae.abs() < 1.0 {
        let mut cleft = [0.0; 4];
        for j in 1..=4 {
            cleft[j - 1] = rcfl(mrs, 1, j) * rs
                + rcfl(mrs, 2, j) * rs.powf(1.5)
                + rcfl(mrs, 3, j) * rs.powi(2);
        }
        real = cleft[0];
        for j in 2..=4 {
            real += cleft[j - 1] * xx.powi((j - 1) as i32);
        }
    }

    if icusp == 1 || deltae.abs() < 1.0 {
        let mut right_real = eee / xx;
        for j in 1..=2 {
            right_real += cright[j - 1] / xx.powi((j + 1) as i32);
        }
        if deltae.abs() < 1.0 {
            let blend = if deltae < 0.0 {
                (1.0 + deltae).powi(2) / 2.0
            } else {
                1.0 - (1.0 - deltae).powi(2) / 2.0
            };
            real = blend * right_real + (1.0 - blend) * real;
        } else {
            real = right_real;
        }
    }

    ExchangeEvaluation {
        real: real * ef,
        imaginary,
    }
}

fn dirac_hara(rs: f64, xk: f64) -> f64 {
    if rs > 100.0 {
        return 0.0;
    }

    let xf = FA / rs;
    let mut x = xk / xf + 1.0e-5;
    if x < HL_MIN_X {
        x = HL_MIN_X;
    }
    let c = ((1.0 + x) / (1.0 - x)).abs().ln();
    -(xf / PI) * (1.0 + c * (1.0 - x.powi(2)) / (2.0 * x))
}

fn von_barth_hedin(rs: f64) -> f64 {
    if rs > 1_000.0 {
        return 0.0;
    }

    let epc = -0.0504 * vbh_flarge(rs / 30.0);
    let efc = -0.0254 * vbh_flarge(rs / 75.0);
    let xmup = -0.0504 * (1.0 + 30.0 / rs).ln();
    let vu = VBH_GAMMA * (efc - epc);

    let alg = -1.221_774_12 / rs + vu;
    let blg = xmup - vu;

    // FEFF's vbh path uses xmag=1.0 for unpolarized potentials in POT setup.
    (alg + blg) / 2.0
}

fn vbh_flarge(x: f64) -> f64 {
    (1.0 + x.powi(3)) * (1.0 + 1.0 / x).ln() + x / 2.0 - x.powi(2) - THIRD
}

fn perdew_zunger(rs: f64) -> f64 {
    slater_vx(rs) + pz_vc(rs)
}

fn slater_vx(rs: f64) -> f64 {
    (4.0 / 3.0) * SLATER_F * TWO_THIRDS / rs
}

fn pz_vc(rs: f64) -> f64 {
    if rs < 1.0 {
        let lnrs = rs.ln();
        PZ_A * lnrs
            + (PZ_B - PZ_A / 3.0)
            + (2.0 / 3.0) * PZ_C * rs * lnrs
            + ((2.0 * PZ_D - PZ_C) / 3.0) * rs
    } else {
        let rs12 = rs.sqrt();
        let ox = 1.0 + PZ_B1 * rs12 + PZ_B2 * rs;
        let dox = 1.0 + (7.0 / 6.0) * PZ_B1 * rs12 + (4.0 / 3.0) * PZ_B2 * rs;
        let ec = PZ_GC / ox;
        ec * dox / ox
    }
}

fn imhl(rs: f64, xk: f64) -> (f64, i32) {
    let xf = FA / rs;
    let ef = xf.powi(2) / 2.0;
    let mut xk0 = xk / xf;
    if xk0 < HL_MIN_X {
        xk0 = HL_MIN_X;
    }

    let wp = (3.0 / rs.powi(3)).sqrt() / ef;
    let xs = wp.powi(2) - (xk0.powi(2) - 1.0).powi(2);

    let mut eim = 0.0;
    let mut icusp = 0;

    if xs < 0.0 {
        let q2_arg = ((HL_ALPHA.powi(2) - 4.0 * xs).sqrt() - HL_ALPHA) / 2.0;
        let q2 = q2_arg.max(0.0).sqrt();
        let qu = q2.min(1.0 + xk0);
        let d1 = qu - (xk0 - 1.0);
        if d1 > 0.0 {
            eim = ffq(qu, ef, xk, wp, HL_ALPHA) - ffq(xk0 - 1.0, ef, xk, wp, HL_ALPHA);
        }
    }

    let (rad, qplus, qminus) = cubic_roots(xk0, wp, HL_ALPHA);
    if rad <= 0.0 {
        let d2 = qplus - (xk0 + 1.0);
        if d2 > 0.0 {
            eim += ffq(qplus, ef, xk, wp, HL_ALPHA) - ffq(xk0 + 1.0, ef, xk, wp, HL_ALPHA);
        }
        let d3 = (xk0 - 1.0) - qminus;
        if d3 > 0.0 {
            eim += ffq(xk0 - 1.0, ef, xk, wp, HL_ALPHA) - ffq(qminus, ef, xk, wp, HL_ALPHA);
            icusp = 1;
        }
    }

    let ei = quinn(xk0, rs, wp, ef);
    if eim >= ei {
        eim = ei;
    }
    (eim, icusp)
}

fn ffq(q: f64, ef: f64, xk: f64, wp: f64, alph: f64) -> f64 {
    let wq = (wp.powi(2) + alph * q.powi(2) + q.powi(4)).sqrt();
    let value = (wp + wq) / q.powi(2) + alph / (2.0 * wp);
    ((ef * wp) / (4.0 * xk)) * value.ln()
}

fn cubic_roots(xk0: f64, wp: f64, alph: f64) -> (f64, f64, f64) {
    let a2 = (alph / (4.0 * xk0.powi(2)) - 1.0) * xk0;
    let a0 = wp.powi(2) / (4.0 * xk0);

    let q = -(a2.powi(2)) / 9.0;
    let r = -(3.0 * a0) / 6.0 - a2.powi(3) / 27.0;
    let rad = q.powi(3) + r.powi(2);
    if rad > 0.0 {
        return (rad, 0.0, 0.0);
    }

    let s13 = Complex64::new(r, (-rad).sqrt());
    let s1 = s13.powf(THIRD);
    let qplus = (2.0 * s1 - Complex64::new(a2 / 3.0, 0.0)).re;
    let qminus =
        (-(s1 - Complex64::new(3.0_f64.sqrt() * s1.im, 0.0) + Complex64::new(a2 / 3.0, 0.0))).re;
    (rad, qplus, qminus)
}

fn quinn(x: f64, rs: f64, wp: f64, ef: f64) -> f64 {
    let alpha_q = 1.0 / FA;
    let mut pfq = PI.sqrt() / (32.0 * (alpha_q * rs).powf(1.5));
    let temp1 = (PI / (alpha_q * rs)).sqrt().atan();
    let temp2 = (alpha_q * rs / PI).sqrt() / (1.0 + alpha_q * rs / PI);
    pfq *= temp1 + temp2;

    let mut wkc = ((1.0 + wp).sqrt() - 1.0).powi(2);
    wkc = (1.0 + (6.0 / 5.0) * wkc / wp.powi(2)) * wp * ef;
    let ekc = wkc + ef;

    let gamma = (pfq / x) * (x.powi(2) - 1.0).powi(2);
    let eabs = ef * x.powi(2);
    let argument = (eabs - ekc) / (0.3 * ekc);
    let cutoff = if argument < 80.0 {
        1.0 / (1.0 + argument.exp())
    } else {
        0.0
    };
    -gamma * cutoff / 2.0
}

fn rcfr(mrs: usize, rs_power: usize, coefficient: usize) -> f64 {
    let index = (mrs - 1) + 4 * (rs_power - 1) + 12 * (coefficient - 1);
    RCFR[index]
}

fn rcfl(mrs: usize, rs_power: usize, coefficient: usize) -> f64 {
    let index = (mrs - 1) + 4 * (rs_power - 1) + 12 * (coefficient - 1);
    RCFL[index]
}

#[cfg(test)]
mod tests {
    use super::{ExchangeEvaluationInput, ExchangeModel, evaluate_exchange_potential};
    use crate::common::constants::PI;

    #[test]
    fn maps_feff_ixc_to_exchange_models() {
        assert_eq!(
            ExchangeModel::from_feff_ixc(0),
            ExchangeModel::HedinLundqvist
        );
        assert_eq!(
            ExchangeModel::from_feff_ixc(5),
            ExchangeModel::HedinLundqvist
        );
        assert_eq!(
            ExchangeModel::from_feff_ixc(10),
            ExchangeModel::HedinLundqvist
        );

        assert_eq!(ExchangeModel::from_feff_ixc(1), ExchangeModel::DiracHara);
        assert_eq!(ExchangeModel::from_feff_ixc(3), ExchangeModel::DiracHara);
        assert_eq!(ExchangeModel::from_feff_ixc(13), ExchangeModel::DiracHara);

        assert_eq!(
            ExchangeModel::from_feff_ixc(2),
            ExchangeModel::VonBarthHedin
        );
        assert_eq!(
            ExchangeModel::from_feff_ixc(7),
            ExchangeModel::VonBarthHedin
        );
        assert_eq!(ExchangeModel::from_feff_ixc(4), ExchangeModel::PerdewZunger);
        assert_eq!(ExchangeModel::from_feff_ixc(8), ExchangeModel::PerdewZunger);
    }

    #[test]
    fn hedin_lundqvist_matches_feff_reference_samples() {
        let samples: &[(f64, f64, f64, f64)] = &[
            (0.8, 2.0, -8.49656165836650e-01, -5.37284632348553e-11),
            (1.4, 2.7, -4.37314528103467e-01, -2.09949041449695e-01),
            (2.2, 3.5, -1.42636930901624e-01, -1.51019101330210e-01),
            (4.5, 5.0, -3.04153907526486e-02, -5.44375732329782e-02),
        ];

        for &(rs, xk, expected_real, expected_imaginary) in samples {
            let density = density_from_rs(rs);
            let evaluated = evaluate_exchange_potential(ExchangeEvaluationInput::new(
                ExchangeModel::HedinLundqvist,
                density,
                0.0,
                xk,
            ));
            assert!(
                (evaluated.real - expected_real).abs() <= 2.0e-7,
                "HL real mismatch for rs={rs}, xk={xk}: expected {expected_real}, got {}",
                evaluated.real
            );
            assert!(
                (evaluated.imaginary - expected_imaginary).abs() <= 2.0e-7,
                "HL imaginary mismatch for rs={rs}, xk={xk}: expected {expected_imaginary}, got {}",
                evaluated.imaginary
            );
        }
    }

    #[test]
    fn dirac_hara_matches_feff_reference_samples() {
        let samples: &[(f64, f64, f64)] = &[
            (0.8, 2.0, -7.63515499719434e-01),
            (1.4, 2.7, -7.93514058704492e-02),
            (2.2, 3.5, -1.16464733953738e-02),
            (4.5, 5.0, -6.59396163573939e-04),
        ];

        for &(rs, xk, expected_real) in samples {
            let density = density_from_rs(rs);
            let evaluated = evaluate_exchange_potential(ExchangeEvaluationInput::new(
                ExchangeModel::DiracHara,
                density,
                0.0,
                xk,
            ));
            assert!(
                (evaluated.real - expected_real).abs() <= 2.0e-7,
                "DH real mismatch for rs={rs}, xk={xk}: expected {expected_real}, got {}",
                evaluated.real
            );
            assert_eq!(evaluated.imaginary, 0.0, "DH should not set imaginary part");
        }
    }

    #[test]
    fn von_barth_hedin_matches_feff_reference_samples() {
        let samples: &[(f64, f64)] = &[
            (0.8, -8.55605401086108e-01),
            (1.4, -5.14728351946735e-01),
            (2.2, -3.45300361314999e-01),
            (4.5, -1.87082102551233e-01),
        ];

        for &(rs, expected_real) in samples {
            let density = density_from_rs(rs);
            let evaluated = evaluate_exchange_potential(ExchangeEvaluationInput::new(
                ExchangeModel::VonBarthHedin,
                density,
                0.0,
                2.8,
            ));
            assert!(
                (evaluated.real - expected_real).abs() <= 2.0e-7,
                "VBH real mismatch for rs={rs}: expected {expected_real}, got {}",
                evaluated.real
            );
            assert_eq!(
                evaluated.imaginary, 0.0,
                "VBH should not set imaginary part"
            );
        }
    }

    #[test]
    fn perdew_zunger_matches_feff_reference_samples() {
        let samples: &[(f64, f64)] = &[
            (0.8, -8.35873273039178e-01),
            (1.4, -4.95831903962417e-01),
            (2.2, -3.27475420984098e-01),
            (4.5, -1.71352372050846e-01),
        ];

        for &(rs, expected_real) in samples {
            let density = density_from_rs(rs);
            let evaluated = evaluate_exchange_potential(ExchangeEvaluationInput::new(
                ExchangeModel::PerdewZunger,
                density,
                0.0,
                2.8,
            ));
            assert!(
                (evaluated.real - expected_real).abs() <= 2.0e-7,
                "PZ real mismatch for rs={rs}: expected {expected_real}, got {}",
                evaluated.real
            );
            assert_eq!(evaluated.imaginary, 0.0, "PZ should not set imaginary part");
        }
    }

    #[test]
    fn local_exchange_models_select_distinct_kernels() {
        let density = 0.125;
        let vbh = evaluate_exchange_potential(ExchangeEvaluationInput::new(
            ExchangeModel::VonBarthHedin,
            density,
            0.0,
            2.8,
        ));
        let pz = evaluate_exchange_potential(ExchangeEvaluationInput::new(
            ExchangeModel::PerdewZunger,
            density,
            0.0,
            2.8,
        ));
        assert_ne!(
            vbh.real, pz.real,
            "VBH and PZ should evaluate different kernels"
        );
        assert_eq!(vbh.imaginary, 0.0);
        assert_eq!(pz.imaginary, 0.0);
    }

    fn density_from_rs(rs: f64) -> f64 {
        3.0 / (4.0 * PI * rs.powi(3))
    }
}

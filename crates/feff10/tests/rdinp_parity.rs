use std::collections::HashSet;
use std::path::Path;
use std::sync::Mutex;

use feff10::{FeffInput, Stage};

static FEFF_STAGE_LOCK: Mutex<()> = Mutex::new(());

#[cfg(unix)]
fn run_rdinp_raw(work_dir: &Path) -> (bool, String) {
    let _guard = FEFF_STAGE_LOCK.lock().unwrap();
    let pid = unsafe { libc::fork() };
    match pid {
        -1 => (false, "fork failed".to_string()),
        0 => {
            if std::env::set_current_dir(work_dir).is_err() {
                unsafe { libc::_exit(2) };
            }
            unsafe { Stage::Rdinp.call_ffi() };
            unsafe { libc::_exit(0) };
        }
        child => {
            let mut status: libc::c_int = 0;
            let ret = unsafe { libc::waitpid(child, &mut status, 0) };
            if ret == -1 {
                return (false, "waitpid failed".to_string());
            }
            let feff_error = std::fs::read_to_string(work_dir.join(".feff.error"))
                .unwrap_or_default()
                .trim()
                .to_string();
            if libc::WIFEXITED(status) {
                let code = libc::WEXITSTATUS(status);
                (code == 0 && feff_error.is_empty(), feff_error)
            } else {
                (false, feff_error)
            }
        }
    }
}

#[cfg(not(unix))]
fn run_rdinp_raw(_work_dir: &Path) -> (bool, String) {
    (false, "unsupported platform".to_string())
}

#[test]
fn rdinp_parity_rewritten_input_basic() {
    let tmp_raw = tempfile::tempdir().unwrap();
    let tmp_rewrite = tempfile::tempdir().unwrap();
    let raw = "\
; full-line comment
TITLE parity basic
EDGE K 1.0
CONT 1 1 1 1 1 1
PRINT 0 0 0 0 0 0
EXAFS 20.0
RMAX 5.5
POTENTIALS
0 29 Cu
1 29 Cu
ATOMS
0.0 0.0 0.0 0 Cu
1.805 1.805 0.0 1 Cu
END
";
    std::fs::write(tmp_raw.path().join("feff.inp"), raw).unwrap();
    let (ok_raw, err_raw) = run_rdinp_raw(tmp_raw.path());
    assert!(ok_raw, "Fortran rdinp failed on raw input: {err_raw}");

    let parsed = FeffInput::parse(raw).unwrap();
    let mut buf = Vec::new();
    parsed.write_to(&mut buf).unwrap();
    std::fs::write(tmp_rewrite.path().join("feff.inp"), buf).unwrap();
    let (ok_rewrite, err_rewrite) = run_rdinp_raw(tmp_rewrite.path());
    assert!(
        ok_rewrite,
        "Fortran rdinp failed on rewritten input: {err_rewrite}"
    );
}

#[test]
fn rdinp_parity_include_load() {
    let tmp_main = tempfile::tempdir().unwrap();
    let tmp_rewrite = tempfile::tempdir().unwrap();
    let include = "\
EDGE K 1.0
S02 1.0
CONTROL 1 1 1 1 1 1
PRINT 0 0 0 0 0 0
EXAFS 20.0
RPATH 5.5
";
    std::fs::write(tmp_main.path().join("extra_cards.inp"), include).unwrap();
    let main = "\
TITLE include parity
load extra_cards.inp
POTENTIALS
0 29 Cu
1 29 Cu
ATOMS
0.0 0.0 0.0 0 Cu
1.805 1.805 0.0 1 Cu
END
";
    std::fs::write(tmp_main.path().join("feff.inp"), main).unwrap();
    let (ok_raw, err_raw) = run_rdinp_raw(tmp_main.path());
    assert!(ok_raw, "Fortran rdinp failed on include input: {err_raw}");

    let parsed = FeffInput::from_file(tmp_main.path().join("feff.inp")).unwrap();
    let mut buf = Vec::new();
    parsed.write_to(&mut buf).unwrap();
    std::fs::write(tmp_rewrite.path().join("feff.inp"), buf).unwrap();
    let (ok_rewrite, err_rewrite) = run_rdinp_raw(tmp_rewrite.path());
    assert!(
        ok_rewrite,
        "Fortran rdinp failed on rewritten include input: {err_rewrite}"
    );
}

#[test]
fn rdinp_parity_comment_lines() {
    let tmp_raw = tempfile::tempdir().unwrap();
    let tmp_rewrite = tempfile::tempdir().unwrap();
    let raw = "\
; semicolon comment
# hash comment
% percent comment
   * star comment with leading spaces
TITLE comment parity
CONTROL 1 1 1 1 1 1
PRINT 0 0 0 0 0 0
POTENTIALS
0 29 Cu
1 29 Cu
ATOMS
0.0 0.0 0.0 0 Cu
1.805 1.805 0.0 1 Cu
END
";
    std::fs::write(tmp_raw.path().join("feff.inp"), raw).unwrap();
    let (ok_raw, err_raw) = run_rdinp_raw(tmp_raw.path());
    assert!(ok_raw, "Fortran rdinp failed on comment input: {err_raw}");

    let parsed = FeffInput::parse(raw).unwrap();
    let mut buf = Vec::new();
    parsed.write_to(&mut buf).unwrap();
    std::fs::write(tmp_rewrite.path().join("feff.inp"), buf).unwrap();
    let (ok_rewrite, err_rewrite) = run_rdinp_raw(tmp_rewrite.path());
    assert!(
        ok_rewrite,
        "Fortran rdinp failed on rewritten comment input: {err_rewrite}"
    );
}

#[test]
fn rdinp_parity_inline_potentials_comment_is_rejected() {
    let tmp_raw = tempfile::tempdir().unwrap();
    let raw = "\
TITLE inline potential comment
CONTROL 1 1 1 1 1 1
PRINT 0 0 0 0 0 0
POTENTIALS
0 29 Cu * absorber
ATOMS
0.0 0.0 0.0 0 Cu
END
";
    std::fs::write(tmp_raw.path().join("feff.inp"), raw).unwrap();
    let (ok_raw, _err_raw) = run_rdinp_raw(tmp_raw.path());
    assert!(!ok_raw, "Fortran should reject inline POTENTIALS comment");

    let err = FeffInput::parse(raw).unwrap_err().to_string();
    assert!(
        err.contains("l_scmt"),
        "Rust parser should reject inline POTENTIALS comment similarly: {err}"
    );
}

#[test]
fn rdinp_parity_conflicting_spectroscopy_is_rejected() {
    let tmp_raw = tempfile::tempdir().unwrap();
    let raw = "\
TITLE conflict
XANES 8.0
EXAFS 20.0
POTENTIALS
0 29 Cu
ATOMS
0.0 0.0 0.0 0 Cu
END
";
    std::fs::write(tmp_raw.path().join("feff.inp"), raw).unwrap();
    let (ok_raw, _err_raw) = run_rdinp_raw(tmp_raw.path());
    assert!(
        !ok_raw,
        "Fortran should reject incompatible spectroscopy cards"
    );

    let err = FeffInput::parse(raw).unwrap_err().to_string();
    assert!(
        err.contains("more than one type of spectroscopy"),
        "Rust parser should reject incompatible spectroscopy cards: {err}"
    );
}

#[test]
fn rdinp_parity_config_card_block_roundtrip() {
    let tmp_raw = tempfile::tempdir().unwrap();
    let tmp_rewrite = tempfile::tempdir().unwrap();
    let raw = "\
TITLE config parity
CONFIG card 2
first config line
second config line
POTENTIALS
0 29 Cu
1 29 Cu
ATOMS
0.0 0.0 0.0 0 Cu
1.805 1.805 0.0 1 Cu
END
";
    std::fs::write(tmp_raw.path().join("feff.inp"), raw).unwrap();
    let (ok_raw, err_raw) = run_rdinp_raw(tmp_raw.path());
    assert!(
        ok_raw,
        "Fortran rdinp failed on CONFIG card input: {err_raw}"
    );

    let parsed = FeffInput::parse(raw).unwrap();
    let mut buf = Vec::new();
    parsed.write_to(&mut buf).unwrap();
    std::fs::write(tmp_rewrite.path().join("feff.inp"), buf).unwrap();
    let (ok_rewrite, err_rewrite) = run_rdinp_raw(tmp_rewrite.path());
    assert!(
        ok_rewrite,
        "Fortran rdinp failed on rewritten CONFIG card input: {err_rewrite}"
    );
}

#[test]
fn rdinp_parity_elnes_continuation_roundtrip() {
    let tmp_raw = tempfile::tempdir().unwrap();
    let tmp_rewrite = tempfile::tempdir().unwrap();
    let raw = "\
TITLE elnes parity
ELNES 8.0
200 ; averaging line with comment
1.0 0.0 0.0
10.0 20.0
30 40
50.0 60.0
POTENTIALS
0 29 Cu
1 29 Cu
ATOMS
0.0 0.0 0.0 0 Cu
1.805 1.805 0.0 1 Cu
END
";
    std::fs::write(tmp_raw.path().join("feff.inp"), raw).unwrap();
    let (ok_raw, err_raw) = run_rdinp_raw(tmp_raw.path());
    assert!(
        ok_raw,
        "Fortran rdinp failed on ELNES continuation input: {err_raw}"
    );

    let parsed = FeffInput::parse(raw).unwrap();
    let mut buf = Vec::new();
    parsed.write_to(&mut buf).unwrap();
    std::fs::write(tmp_rewrite.path().join("feff.inp"), buf).unwrap();
    let (ok_rewrite, err_rewrite) = run_rdinp_raw(tmp_rewrite.path());
    assert!(
        ok_rewrite,
        "Fortran rdinp failed on rewritten ELNES continuation input: {err_rewrite}"
    );
}

struct Scenario {
    name: &'static str,
    raw: &'static str,
    setup: fn(&Path),
}

fn setup_none(_work_dir: &Path) {}

fn setup_cif(work_dir: &Path) {
    let source = Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../feff10/examples/KSPACE/Cr2GeC/Cr2GeC.cif"
    ));
    assert!(source.exists(), "missing CIF fixture: {}", source.display());
    std::fs::copy(source, work_dir.join("struct.cif")).unwrap();
}

fn run_roundtrip_case(case: &Scenario) {
    let tmp_raw = tempfile::tempdir().unwrap();
    let tmp_rewrite = tempfile::tempdir().unwrap();
    (case.setup)(tmp_raw.path());
    (case.setup)(tmp_rewrite.path());
    std::fs::write(tmp_raw.path().join("feff.inp"), case.raw).unwrap();
    let (ok_raw, err_raw) = run_rdinp_raw(tmp_raw.path());
    assert!(
        ok_raw,
        "Fortran rdinp failed on raw input for case '{}': {}",
        case.name, err_raw
    );

    let parsed = FeffInput::parse(case.raw)
        .unwrap_or_else(|err| panic!("Rust parser failed for case '{}': {err}", case.name));
    let mut buf = Vec::new();
    parsed.write_to(&mut buf).unwrap();
    std::fs::write(tmp_rewrite.path().join("feff.inp"), buf).unwrap();
    let (ok_rewrite, err_rewrite) = run_rdinp_raw(tmp_rewrite.path());
    assert!(
        ok_rewrite,
        "Fortran rdinp failed on rewritten input for case '{}': {}",
        case.name, err_rewrite
    );
}

fn scenario_token_set(raw: &str) -> HashSet<i16> {
    FeffInput::parse(raw)
        .unwrap()
        .cards
        .iter()
        .map(|card| card.token.id())
        .filter(|id| *id > 0)
        .collect()
}

fn all_card_scenarios() -> Vec<Scenario> {
    vec![
        Scenario {
            name: "core_cards_a",
            setup: setup_none,
            raw: r#"TITLE core cards A
HOLE 1 0.9
CONTROL 1 1 1 1 1 1
EXCHANGE 0 0.0 0.0 2
ION 0 0.0
FOLP 0 1.1
RPATH 6.0
DEBYE 300.0 450.0 0
RMULTIPLIER 1.0
SS 1 0 2.0 2.5
PRINT 0 0 0 0 0 0
NLEG 6
CRITERIA 2.5 3.0
NOGEOM
IORDER 2
PCRITERIA 1.0 2.0
SIG2 0.003
CORRECTIONS 0.1 0.2
AFOLP 1.2
EXAFS 12.0
POLARIZATION 1.0 0.0 0.0
ELLIPTICITY 0.0 0.0 1.0 0.0
RGRID 0.05
RPHASES
NSTAR
NOHOLE 1
SIG3 0.01
JUMPRM
MBCONV
SPIN 1 0.0 0.0 1.0
EDGE K
SCF 4.5 1 20 0.2 3 -40.0 0
FMS 6.0 1 0 0.01 0.01 12.0
LDOS -10.0 10.0 0.5 50 0
INTERSTITIAL 1 0.0
CFAVERAGE 0 1 2.0
S02 1.0
RSIGMA
XNCD
MULTIPOLE 1 1
UNFREEZEF
TDLDA 1
PMBSE 1 1 1 1
MPSE 1 2
SFCONV
SELF
SFSE 2.0
MAGIC 35.0
ABSOLUTE
SYMMETRY 4
REAL
COORDINATES 3
EXTPOT
CHBROADENING 1
CHSHIFT 1
DIMS 150 4
SETEDGE
EPS0 2.0
PREPS
EGAP 1.2
CHWIDTH 0.8
RESTART
SCREEN rfms 5.5
EQUIVALENCE 1
TEMP 300.0 11
RIXS 0.1 0.2 0.0
RLPRINT
ICORE 1
SCXC 21
HIGHZ
SCFTH 1 6.0 200 60 1.0e-4
WARNION
SCFRAMP 2.0 4
TOLSCF 1.0e-4 1.0e-4 1.0e-4
POTENTIALS
0 29 Cu
1 29 Cu
ATOMS
0.0 0.0 0.0 0 Cu
1.8 1.8 0.0 1 Cu
END
"#,
        },
        Scenario {
            name: "core_cards_b",
            setup: setup_none,
            raw: r#"TITLE core cards B
CONTROL 1 1 1 1 1 1
PRINT 0 0 0 0 0 0
EXAFS 10.0
OPCONS
NUMDENS 0 0.05
COMPTON 2.0 10 1
RHOZZP
CGRID 2.0 5 8 8 8
CORVAL -30.0
SIGGK 0.1
HUBBARD 5.0 0.5 0.0 3
CRPA 2 3.0
FULLSPECTRUM
POTENTIALS
0 29 Cu
ATOMS
0.0 0.0 0.0 0 Cu
END
"#,
        },
        Scenario {
            name: "overlap_section",
            setup: setup_none,
            raw: r#"TITLE overlap mode
CONTROL 1 1 1 1 1 1
PRINT 0 0 0 0 0 0
OVERLAP 0
1 1 2.0
OVERLAP 1
0 1 2.0
POTENTIALS
0 29 Cu
1 29 Cu
SS 1 1 1.0 2.0
END
"#,
        },
        Scenario {
            name: "config_card",
            setup: setup_none,
            raw: r#"TITLE config matrix
CONFIG card 2
first config line
second config line
POTENTIALS
0 29 Cu
ATOMS
0.0 0.0 0.0 0 Cu
END
"#,
        },
        Scenario {
            name: "nrixs_mdff",
            setup: setup_none,
            raw: r#"TITLE nrixs mdff
XANES 8.0
NRIXS 2 0.0 0.0 1.0 1.0
0.0 1.0 0.0 1.0
LJMAX 3
LDEC 2
MDFF 2
POTENTIALS
0 29 Cu
1 29 Cu
ATOMS
0.0 0.0 0.0 0 Cu
1.8 0.0 0.0 1 Cu
END
"#,
        },
        Scenario {
            name: "xes_spectroscopy",
            setup: setup_none,
            raw: r#"TITLE xes
XES -20.0 20.0 0.1
POTENTIALS
0 29 Cu
ATOMS
0.0 0.0 0.0 0 Cu
END
"#,
        },
        Scenario {
            name: "danes_spectroscopy",
            setup: setup_none,
            raw: r#"TITLE danes
DANES 8.0 0.1 0.2
POTENTIALS
0 29 Cu
ATOMS
0.0 0.0 0.0 0 Cu
END
"#,
        },
        Scenario {
            name: "fprime_spectroscopy",
            setup: setup_none,
            raw: r#"TITLE fprime
FPRIME -5.0 30.0 0.5
POTENTIALS
0 29 Cu
ATOMS
0.0 0.0 0.0 0 Cu
END
"#,
        },
        Scenario {
            name: "elnes_continuation",
            setup: setup_none,
            raw: r#"TITLE elnes
ELNES 8.0
200 ; comment on first continuation
1.0 0.0 0.0
10.0 20.0
30 40
50.0 60.0
POTENTIALS
0 29 Cu
ATOMS
0.0 0.0 0.0 0 Cu
END
"#,
        },
        Scenario {
            name: "exelfs_continuation",
            setup: setup_none,
            raw: r#"TITLE exelfs
EXELFS 8.0
200 0 0 0 0 0
1.0 0.0 0.0
10.0 20.0
30 40
50.0 60.0
POTENTIALS
0 29 Cu
ATOMS
0.0 0.0 0.0 0 Cu
END
"#,
        },
        Scenario {
            name: "reciprocal_lattice",
            setup: setup_none,
            raw: r#"TITLE reciprocal lattice
RECIPROCAL
LATTICE P 1.0
1.0 0.0 0.0
0.0 1.0 0.0
0.0 0.0 1.0
KMESH 2 2 2 1 0
TARGET 1
SGROUP 1
STRFAC 0.01 8.0 6.0
BANDSTRUCTURE -5.0 5.0 0.5 1 20 T
COREHOLE FSR
COORDINATES 3
POTENTIALS
0 29 Cu
1 29 Cu
ATOMS
0.0 0.0 0.0 1 Cu
END
"#,
        },
        Scenario {
            name: "reciprocal_cif",
            setup: setup_cif,
            raw: r#"TITLE reciprocal cif
RECIPROCAL
CIF struct.cif
KMESH 2 2 2
TARGET 1
EQUIVALENCE 1
END
"#,
        },
        Scenario {
            name: "egrid_block",
            setup: setup_none,
            raw: r#"TITLE egrid block
EGRID
-10.0 -5.0 0.5
-5.0 0.0 0.2
POTENTIALS
0 29 Cu
ATOMS
0.0 0.0 0.0 0 Cu
END
"#,
        },
        Scenario {
            name: "density_block",
            setup: setup_none,
            raw: r#"TITLE density block
DENSITY
Cu 0.5
Zn 0.5
POTENTIALS
0 29 Cu
ATOMS
0.0 0.0 0.0 0 Cu
END
"#,
        },
    ]
}

#[test]
fn rdinp_parity_all_cards_matrix_roundtrip() {
    let cases = all_card_scenarios();

    let mut covered = HashSet::new();
    for case in &cases {
        covered.extend(scenario_token_set(case.raw));
    }

    let expected: HashSet<i16> = (1..=112)
        // FEFF10's `itoken.f90` effectively leaves token 55 (RCONV) unreachable.
        .filter(|id| !matches!(*id, 55 | 69 | 70))
        .map(|id| id as i16)
        .collect();
    let mut missing: Vec<i16> = expected.difference(&covered).copied().collect();
    missing.sort_unstable();
    assert!(
        missing.is_empty(),
        "card matrix is missing coverage for token(s): {missing:?}"
    );

    for case in &cases {
        run_roundtrip_case(case);
    }
}

#[test]
fn rdinp_parity_config_card_block_skips_comment_lines() {
    let raw = r#"TITLE config comment handling
CONFIG card 2
; comment before first line
first payload line
# comment before second line
second payload line
POTENTIALS
0 29 Cu
ATOMS
0.0 0.0 0.0 0 Cu
END
"#;

    let parsed = FeffInput::parse(raw).unwrap();
    let config = parsed
        .cards
        .iter()
        .find(|card| card.token.id() == 90)
        .expect("CONFIG card missing");
    assert_eq!(config.continuation.len(), 2);
    assert_eq!(config.continuation[0], "first payload line");
    assert_eq!(config.continuation[1], "second payload line");

    let case = Scenario {
        name: "config_comment_lines",
        raw,
        setup: setup_none,
    };
    run_roundtrip_case(&case);
}

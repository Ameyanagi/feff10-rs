#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeffCall {
    Rdinp,
    Ffmod1,
    Ffmod7,
    Ffmod8,
    Ffmod2,
    Ffsort {
        absorber_index: usize,
        nss: i32,
        ceels: bool,
    },
    Ffmod3,
    Ffmod4,
    Ffmod5,
    Ffmod6 {
        absorber_index: usize,
    },
    Ffmod9,
    Eelsmod,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeffMainPlan {
    pub nabs: usize,
    pub nss: i32,
    pub ceels: bool,
    pub calls: Vec<FeffCall>,
}

pub fn feff_main_plan(nabs: usize, nss: i32, ceels: bool) -> FeffMainPlan {
    let mut calls = vec![
        FeffCall::Rdinp,
        FeffCall::Ffmod1,
        FeffCall::Ffmod7,
        FeffCall::Ffmod8,
        FeffCall::Ffmod2,
    ];

    for absorber_index in 1..=nabs {
        if nabs > 1 {
            calls.push(FeffCall::Ffsort {
                absorber_index,
                nss,
                ceels,
            });
        }

        calls.push(FeffCall::Ffmod3);
        calls.push(FeffCall::Ffmod4);
        calls.push(FeffCall::Ffmod5);
        calls.push(FeffCall::Ffmod6 { absorber_index });
        calls.push(FeffCall::Ffmod9);
        calls.push(FeffCall::Eelsmod);
    }

    FeffMainPlan {
        nabs,
        nss,
        ceels,
        calls,
    }
}

#[cfg(test)]
mod tests {
    use super::{FeffCall, feff_main_plan};

    #[test]
    fn single_absorber_matches_legacy_call_order_without_ffsort() {
        let plan = feff_main_plan(1, 2, true);
        assert_eq!(plan.calls.len(), 11);
        assert_eq!(plan.calls[0], FeffCall::Rdinp);
        assert_eq!(plan.calls[1], FeffCall::Ffmod1);
        assert_eq!(plan.calls[2], FeffCall::Ffmod7);
        assert_eq!(plan.calls[3], FeffCall::Ffmod8);
        assert_eq!(plan.calls[4], FeffCall::Ffmod2);
        assert!(
            plan.calls
                .iter()
                .all(|call| !matches!(call, FeffCall::Ffsort { .. }))
        );
    }

    #[test]
    fn multi_absorber_plan_inserts_ffsort_per_absorber() {
        let plan = feff_main_plan(3, 4, false);

        let sorts = plan
            .calls
            .iter()
            .filter_map(|call| match call {
                FeffCall::Ffsort {
                    absorber_index,
                    nss,
                    ceels,
                } => Some((*absorber_index, *nss, *ceels)),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(sorts, vec![(1, 4, false), (2, 4, false), (3, 4, false)]);

        let ffmod6 = plan
            .calls
            .iter()
            .filter_map(|call| match call {
                FeffCall::Ffmod6 { absorber_index } => Some(*absorber_index),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(ffmod6, vec![1, 2, 3]);
    }

    #[test]
    fn zero_absorbers_keeps_startup_calls_only() {
        let plan = feff_main_plan(0, 1, false);
        assert_eq!(
            plan.calls,
            vec![
                FeffCall::Rdinp,
                FeffCall::Ffmod1,
                FeffCall::Ffmod7,
                FeffCall::Ffmod8,
                FeffCall::Ffmod2
            ]
        );
    }
}

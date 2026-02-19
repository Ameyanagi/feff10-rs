#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AtomicProgramContext {
    pub master: bool,
    pub worker: bool,
    pub mpot: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AtomicProgramPlan {
    pub initialize_dimensions: bool,
    pub open_log_file: bool,
    pub read_pot_inputs: bool,
    pub run_atomic_potentials: bool,
    pub close_log_file: bool,
}

pub fn plan_atomic_program(context: AtomicProgramContext) -> AtomicProgramPlan {
    if context.worker {
        return AtomicProgramPlan {
            initialize_dimensions: false,
            open_log_file: false,
            read_pot_inputs: false,
            run_atomic_potentials: false,
            close_log_file: false,
        };
    }

    AtomicProgramPlan {
        initialize_dimensions: true,
        open_log_file: context.master,
        read_pot_inputs: true,
        run_atomic_potentials: context.mpot == 1,
        close_log_file: context.master,
    }
}

#[cfg(test)]
mod tests {
    use super::{AtomicProgramContext, AtomicProgramPlan, plan_atomic_program};

    #[test]
    fn worker_process_skips_atomic_body() {
        let plan = plan_atomic_program(AtomicProgramContext {
            master: false,
            worker: true,
            mpot: 1,
        });
        assert_eq!(
            plan,
            AtomicProgramPlan {
                initialize_dimensions: false,
                open_log_file: false,
                read_pot_inputs: false,
                run_atomic_potentials: false,
                close_log_file: false,
            }
        );
    }

    #[test]
    fn master_process_runs_atomic_module_when_mpot_enabled() {
        let plan = plan_atomic_program(AtomicProgramContext {
            master: true,
            worker: false,
            mpot: 1,
        });
        assert_eq!(
            plan,
            AtomicProgramPlan {
                initialize_dimensions: true,
                open_log_file: true,
                read_pot_inputs: true,
                run_atomic_potentials: true,
                close_log_file: true,
            }
        );
    }

    #[test]
    fn master_process_skips_atomic_module_when_mpot_disabled() {
        let plan = plan_atomic_program(AtomicProgramContext {
            master: true,
            worker: false,
            mpot: 0,
        });
        assert!(!plan.run_atomic_potentials);
        assert!(plan.read_pot_inputs);
    }
}

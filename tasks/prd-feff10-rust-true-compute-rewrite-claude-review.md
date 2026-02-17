

# PRD Review: FEFF10 True-Compute Rust Rewrite

## 1. Critical Gaps

**CG-1: No numeric tolerance thresholds defined anywhere.**
The PRD references "approved numeric tolerances" (Section 1), "approved policy" (US-005), and "oracle/tolerance gates" (US-024) repeatedly, but never defines what these tolerances are. An autonomous implementer cannot build the comparator (US-003), write module parity tests (US-007–022), or configure CI gates (US-024) without concrete numbers. This is the single largest blocker.

**CG-2: "Compatibility matrix" is referenced but never provided.**
US-007, US-008, FR-4, and others depend on a compatibility matrix that specifies required output artifacts, CLI contracts, and exit-code categories. Without this artifact, every module story is unimplementable — the implementer cannot know what "required outputs" means for any given module.

**CG-3: "Approved fixtures" are undefined.**
Every module story (US-007–022) gates acceptance on "approved fixtures," but there is no list of fixture names, no path to fixture data, and no process for adding or approving new fixtures. The implementer has no way to determine the test surface.

**CG-4: Module dependency ordering is absent.**
Sixteen module rewrites (US-007–022) are listed flatly with no dependency graph. `RDINP` feeds `POT`, `POT` feeds `XSPH`, etc. Without explicit ordering, an autonomous agent may attempt modules in wrong order, producing cascading integration failures.

**CG-5: No definition of "baseline-copy execution" vs. legitimate test fixture usage.**
US-023 requires removing "baseline-copy module runtime behavior," and multiple module stories prohibit "baseline-copy" or "baseline materialization." However, there is no precise technical definition distinguishing prohibited runtime copying from permitted test-reference comparison. An implementer could over- or under-remove.

## 2. Medium Improvements

**MI-1: US-007 through US-022 are nearly identical templates.**
Sixteen module stories share the same four-bullet acceptance criteria pattern with only module names swapped. Each should include at minimum: (a) the specific input artifacts consumed, (b) the specific output artifacts produced, (c) known algorithmic complexity or edge cases unique to that module. Without this, the stories are effectively placeholders, not actionable work items.

**MI-2: No performance or resource requirements.**
OQ-2 acknowledges this gap but leaves it fully open. At minimum, the PRD should state whether v1 must be no worse than Nx Fortran runtime for end-to-end workflows, or whether any regression is acceptable. Without this, an implementer may produce a correct but unusably slow implementation.

**MI-3: Oracle pipeline assumes Fortran compiler availability in CI.**
US-003 requires executing Fortran reference runs in validation tooling, and SM-6 says Fortran is not required in the "primary" build/test pipeline. The PRD should clarify where and how the Fortran oracle binary is built, cached, or distributed. This is an infrastructure prerequisite that blocks US-003.

**MI-4: No error-catalog or exit-code table.**
FR-5 requires preserving "deterministic diagnostic formatting and exit-code categories," but no enumeration of these categories exists in the PRD. The implementer needs a reference table.

**MI-5: Binary artifact format specifications missing.**
US-006 and FR-6 reference "binary artifact contracts." Scientific codes commonly use unformatted Fortran I/O with platform-dependent record markers. The PRD should specify whether Rust must replicate Fortran's unformatted I/O byte layout or whether a format migration is permitted.

**MI-6: No cross-platform determinism scope.**
Section 5 excludes "bitwise-identical outputs across all architectures," and Section 7 mentions "deterministic floating-point handling." These statements are in tension. The PRD should specify the target platform(s) for which determinism is guaranteed.

## 3. Suggested Rewrites

**SR-1: Add a Tolerance Specification section (new Section 4.1 or standalone artifact).**
```markdown
## 4.1 Numeric Tolerance Policy
- Relative tolerance for floating-point output values: ≤ [X] (e.g., 1e-6 relative, 1e-10 absolute).
- Text output field-width and decimal-place formatting must match Fortran output within [N] ULP / [N] decimal places.
- Binary artifacts must be byte-identical / identical within [tolerance].
- Tolerance exceptions per module are documented in [location].
```

**SR-2: Add a Module Dependency DAG to Section 6 or a new Section.**
```markdown
## Module Execution Order
RDINP → POT → XSPH → PATH → FMS
                   ↘ SCREEN
              POT → LDOS
              POT → BAND
        XSPH + PATH + FMS → FULLSPECTRUM
        ...
Implementation must follow this order. Each module's acceptance tests may only depend on upstream modules that have already passed parity.
```

**SR-3: Rewrite a representative module story (e.g., US-008) with specificity.**
```markdown
### US-008: Rewrite POT as true-compute Rust implementation
**Inputs consumed:** `mod1.inp`, `pot.inp`, global.dat (from RDINP)
**Outputs produced:** `pot.bin`, `phase.bin`, `pot.dat`
**Acceptance Criteria:**
- [ ] Rust POT reads RDINP-generated inputs and computes Coulomb, exchange-correlation, and muffin-tin potentials.
- [ ] Output artifacts `pot.bin` and `phase.bin` pass dual-run oracle parity within approved tolerances (see §4.1).
- [ ] Text output `pot.dat` matches Fortran field-width formatting.
- [ ] Edge case: single-atom cluster produces valid degenerate output.
- [ ] No runtime reads of baseline fixture files.
- [ ] Typecheck/lint passes.
```

**SR-4: Replace OQ-3 with a concrete fixture-expansion prerequisite story.**
```markdown
### US-002.1: Expand fixture coverage for under-tested modules
**Acceptance Criteria:**
- [ ] RIXS, FULLSPECTRUM, and COMPTON each have ≥ [N] fixtures covering [specific edge cases].
- [ ] New fixtures are captured from verified Fortran oracle runs and committed before module rewrite begins.
```

## 4. Final Verdict

**REVISE**

The PRD establishes a clear strategic direction and strong constraint boundaries (no Fortran runtime, oracle-only validation, MPI deferral). However, it is not autonomously implementable in its current form due to five critical gaps: undefined tolerance thresholds, a missing compatibility matrix, unspecified fixture inventory, absent module dependency ordering, and an imprecise definition of prohibited baseline-copy behavior. The sixteen module stories are structurally identical templates that lack per-module specificity. Resolving the critical gaps and enriching even 2-3 representative module stories would make this PRD implementation-ready.

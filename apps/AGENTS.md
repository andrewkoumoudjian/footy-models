# Application instructions

Apps are thin orchestration binaries around library crates.

- Keep football/model logic in crates, not `main.rs`.
- CLI/config parsing may depend on small boundary libraries if justified; do not contaminate core crates.
- Fail loudly on schema/version mismatches and partial writes.
- Prefer explicit input/output paths and versioned artifacts over hidden global state.
- All long-running jobs should print enough structured progress/summary information to reproduce what was run, without logging sensitive/raw licensed data unnecessarily.

# Evidence binding

manifest.json maps every hard contract group to implementation, actual tests and limitations. ci-observation.json and ci-green-*.log preserve the observed green repaired-implementation run at its exact commit. Initial stable failure remains in ci-initial-stable-failure.log; falsification.md records diagnosis and repairs.

Local logs bind Linux x86_64, Rust 1.89.0/1.99.0, default/fault builds, package verification, true kernel short write, and the full stress campaign. Benchmark environment includes real per-source checksums; CSVs preserve individual samples, summaries are descriptive. The CPU/filesystem are a virtualized host and measurements imply no general hardware performance claim.

files.sha256 covers every authored repository file except itself. Its own bytes are anchored by the enclosing Git commit. target/.git/__pycache__ and ephemeral evidence/ci outputs are excluded. Run `python3 scripts/evidence.py` from any directory to validate. CI validates this list before building a fresh checkout. Source/test hashes in manifest.json identify the observed implementation separately from later evidence-only changes. Final commit/run URLs can always be resolved from main and Actions; no guessed future commit hashes are embedded.

Power-cut experiments, other platforms, hostile directory ownership, and performance superiority remain explicitly unverified. No critical known engine defect remains concealed as a TODO. No release publication or deployment was performed.

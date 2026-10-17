# Verification evidence

This directory binds BastionKV's correctness claims to concrete implementation, tests, benchmark data, historical CI observations, and file-integrity hashes.

- `manifest.json` maps the evergreen correctness contract to current implementation and evidence.
- `verification-report.md` summarizes the retained verification scope and claim boundaries.
- `adversarial-validation.md` records failure modes that led to material repairs.
- `benchmarks/` contains raw samples, environment metadata, and descriptive summaries.
- `ci-green-*.log`, `ci-initial-stable-failure.log`, and `ci-observation.json` preserve historical workflow observations at their recorded revisions.
- `local/` contains retained local verification output.
- `files.sha256` covers authored repository files except itself.

Run:

```sh
python3 scripts/evidence.py
```

to validate the integrity list. The script excludes `.git`, `target`, `__pycache__`, the hash list itself, and ephemeral `evidence/ci` outputs. CI performs this check before compilation.

Historical logs intentionally remain immutable evidence even when their wording reflects the environment or revision that produced them. Current acceptance always requires the live workflow to pass for the current revision.

The evidence does not establish physical power-loss certification, hostile directory-owner safety, non-Linux durability, production certification, or performance superiority.

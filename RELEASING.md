# Releasing

An alpha is not full CLINS certification. Update `docs/verification.md` with actual test results
and review the current rule inventory. Use explicit paths when staging and include no downloaded
packages or private chart data.

1. Regenerate dependency notices with `python3 scripts/third-party-licenses.py`.
   Run `scripts/check.sh` and all SDK/browser checks from `docs/verification.md`.
2. `cargo package -p clins-core --locked` verifies the standalone crate archive.
3. Build the release CLI, Python abi3 wheel, WASM npm tarball, Kotlin jar/native classifier and
   sources. Retain Apache/MIT notices inside redistributed binaries and SDK packages.
4. Commit source, pass the published CI, then tag `v0.1.0-alpha.1` and create a GitHub prerelease.
5. Attach source/artifacts with a SHA256SUMS manifest. List actual OS/CPU and package channels.
6. Upload to crates.io / PyPI / npm / Maven Central only with the appropriate publisher identity.
   Registry names and credentials must be established before claiming those channels are published.

Never run an upstream Helios workspace release command; this is an independent workspace.

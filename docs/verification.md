# Verification record

Local verification: macOS / Apple Silicon, Rust 1.99.1, Bun 1.4.4,
Python 3.14 with a Python 3.10+ abi3 wheel, Node 24.22.2, Kotlin 2.4.21 / JDK 21 / Gradle 9.7.0.

- Rust formatting, warning-free clippy and unit/integration gate, including Core decimal
  precision, oversized/malformed/duplicate JSON, nested finding paths, CLI exit codes, R4 integer domains,
  HTTP OperationOutcome/Parameters and incomplete outcomes.
- Actual loopback HTTP process smoke test (`python3 scripts/smoke-http.py`), without a database.
- Checksum-pinned official patient examples and mutations; all eight independent synthetic
  document/information fixtures and their negative mutations.
- Python wheel installation and R4 + CLINS positive/negative SDK tests.
- TypeScript declarations, Bun and Node ESM + WASM tests and Chromium browser execution of R4
  and all eight CLINS/R4 fixtures.
- Kotlin generated API, native library loading, JVM tests, jar/native classifier,
  sources and local Maven repository preparation. The JVM tests load the native library
  from the classifier jar on their classpath rather than a developer library path.

Run the Rust gate:

```sh
./scripts/check.sh
cargo package --locked -p clins-core
```

Run the SDK gate after preparing/exporting the public definitions:

```sh
maturin build --locked --manifest-path bindings/python/Cargo.toml --out dist
python -m pip install --force-reinstall --no-deps dist/clins_py-*.whl
python -m unittest discover -s bindings/python/tests
./scripts/build-wasm.sh
bun test bindings/typescript/tests/validator.test.ts
node --test bindings/typescript/tests/node.test.mjs
(cd bindings/typescript && bun install --frozen-lockfile && bunx tsc && bunx playwright install chromium && bunx playwright test)
(cd bindings/kotlin && ./gradlew test jar nativeJar publishAllPublicationsToLocalBuildRepository --no-daemon)
```

CI runs Rust on Linux/macOS and the full SDK/browser pipeline on Linux. Local execution
on macOS does not establish Windows, Android or Kotlin Multiplatform support. Test counts
are test functions, not numbers of CLINS rules. Registry publication and national service
connection testing are not implied by successful builds. Consult release notes for actual
CI run links, attached artifact classifiers and registry channels.

## Engine completion pass after v0.1.0-alpha.1

Unicode character-count maxLength and inclusive integer/decimal minValue/maxValue
checks have been added to the structural walker. Regression cases cover repeat elements,
choice branches, extension values, equal boundaries and decimal distinctions beyond f64.
Date/time and Quantity constraints remain explicitly incomplete. Published alpha.1 artifacts
are unchanged; a new release is required to distribute the new behavior.

CLINS profile/binding discriminators are executed against actual profile content and
installed ValueSets, with populated referral Observation/reslicing, discharge Bundle,
and radiology category positive/negative tests. JP Core / CLINS definitions under
jpfhir.jp have no conversion warnings. Unknown classifications defer their checks
without generating false closed-slicing or minimum-cardinality violations.

Local FHIRPath resolve/memberOf/conformsTo, resource/datatype conformance, recursive
budgets and element/resource/contained-root contexts have dedicated regression cases.
The shared `crates/core/tests/fixtures/engine-cases.tsv` corpus runs the same 28 cases
through Rust, Python, Bun, Node, Chromium and Kotlin. It complements the eight CLINS/R4
fixtures; case counts are not counts of all specification rules.

Optional independent reference comparison (Java is a development oracle only):

```sh
python3 scripts/compare-reference.py --java /path/to/java \
  --validator /path/to/validator_cli-7.0.6.jar --cache-home /path/to/reference-cache
```

The script verifies the pinned validator SHA-256 and public package checksums, disables
HTTP and terminology network access, permits example URLs for synthetic resources,
and compares eight positive examples and eight negative mutations. The Java public
support-package cache must be prepared beforehand. The reference validator loads its
additional standard support packages; this is not a proof that every dependency graph
or terminology service configuration behaves identically.

The current comparison report uses schema version 2: Java's absence of error/fatal
is recorded as `no-errors`, not as a claim of complete validation. Verdict agreement
and normalized diagnostic location differences are reported separately. See the
[upstream reuse and comparison policy](reference-validation.ja.md) for provenance,
scope and limitations. A subsequent [146-case upstream audit](audits/2026-11-19-validation-audit.ja.md)
executed the supported-configuration subset and found confirmed gaps; the remaining
452 R4 JSON candidates requiring additional settings have not yet been run.

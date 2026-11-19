# Architecture and decisions (2026-11-18)

## Boundaries

Core owns its `Service`, `Report`, package identity and JSON entry points. Internal
schema/resolver/converter/walk types are private. No Helios crate appears in its dependency
graph. Pure schema loading accepts in-memory resources or an exported definition bundle;
HTTP, TLS downloads, tar I/O, filesystem paths and process exits live in adapters.
The Core's async FHIRPath evaluator is driven synchronously only with in-memory providers;
there is no Tokio runtime requirement at the API boundary. Tokio's synchronization module
is a transitive dependency of OctoFHIR, so “no Tokio dependency anywhere” is not claimed.

## Selected crates

| Concern | Choice | Reason |
|---|---|---|
| JSON | serde 1.0.229 / serde_json 1.0.154, arbitrary_precision | Preserve source numeric tokens, own reports, common language contract |
| Structural FHIR schema | Adapted Helios 0.2.7 MIT source, private modules | Reuse reviewed algorithms and tests without binding the public API or serde version to Helios |
| FHIRPath | octofhir-fhirpath 0.5.10 | Direct JSON evaluator, current R4 model provider; exercised against CLINS fixtures |
| FHIR type model | octofhir-fhirschema 0.3.42, default features disabled | Embedded R4 type information; no HTTP model provider |
| HTTP / CLI | axum 0.8 / tokio 1 / clap 4 | Transport and command adapters |
| Assets | reqwest rustls / tar / flate2 / sha2 | Explicit, bounded, checksum-verified preparation; no validation-time fetch |
| Python | PyO3 0.30.1 / maturin | abi3 native wheels; Rust Core shared with all SDKs |
| TypeScript | wasm-bindgen 0.2.130 | Same Rust evaluator in browsers and ESM runtimes |
| Kotlin/JVM | UniFFI 0.31.2 / JNA 5.18.1 / Kotlin 2.4.21 / Gradle 9.7.0 | Generated Kotlin boundary and native library distribution |

`jsonschema`, `schemars`, `garde` and `validator` do not implement FHIR's complete profile,
slicing, terminology and FHIRPath semantics. Adding them as a nominal validator would not
replace that work, so unused generic validation crates are deliberately absent.
The Cargo lockfile is the definitive dependency record. This is a tested selection for this
implementation, not a claim that one crate is universally fastest or most mature.

## Integration corrections

OctoFHIR's high-level `FhirPathEngine` initializes native-clock Moka caches and panics on
`wasm32-unknown-unknown`. This adapter uses its public `Evaluator` and bounded AST cache
instead, with the same evaluator on native and WASM. No dependency source is patched.
Structural obligation paths select concrete JSON nodes directly: evaluating the path first
can turn a FHIR Quantity into a System.Quantity and lose its JSON child fields. This was
caught by the laboratory fixture and is covered by the shared SDK matrix.

JSON is not round-tripped through a JavaScript Number or Python float at binding boundaries.
The evaluator's exact numeric parser refuses values beyond rust_decimal's supported range;
those FHIRPath obligations become incomplete instead of rounding or substituting zero.
Preserving arbitrary source precision does not mean arbitrary-precision FHIRPath arithmetic
has been implemented. A future BigDecimal evaluator would require separate semantic tests.

Core/API evolution is independent of the adapted algorithm's original names. Keeping the MIT
modules private allows incremental replacement without changing Python/TS/Kotlin signatures.

## Primary references

- https://docs.rs/octofhir-fhirpath/0.5.10/
- https://docs.rs/octofhir-fhirschema/0.3.42/
- https://crates.io/crates/helios-fhir-validator/0.2.7
- https://pyo3.rs/
- https://mozilla.github.io/uniffi-rs/
- https://kotlinlang.org/docs/gradle-configure-project.html

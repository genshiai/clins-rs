# clins-rs

Offline Rust validation for **FHIR R4 4.0.1 / JP Core 1.1.2-clins / JP-CLINS 1.13.0**.
One validation Core, with CLI, HTTP, Python, TypeScript/WebAssembly and Kotlin/JVM adapters.

**Experimental alpha.** `valid` means the applicable implemented machine checks passed.
It is not certification of every CLINS requirement or acceptance by the national service.
Unsupported rules, unknown definitions and unavailable terminology produce **`incomplete`**;
observed violations produce **`invalid`**, including when other checks are incomplete.
See [coverage](docs/coverage.md) and the [implementation plan](docs/plan.md).

## Architecture

```text
clins-core             pure in-memory validation; no network, filesystem or database
├── clins-packages     checked package archives, lockfile and explicit HTTPS preparation
├── clins-server       Axum HTTP adapter
├── clins-cli          clins validate / serve / prepare / export-definitions
├── clins-py           Python 3.10+ (PyO3 + maturin)
├── clins-ts           browser / Bun / Node-compatible ESM (wasm-bindgen)
└── clins-kt           Kotlin/JVM 21 (UniFFI + JNA)
```

The Core has no Helios crate/type/API dependency. Its **internal structural algorithms
are adapted MIT source from Helios 0.2.7**, with attribution preserved. FHIRPath uses
OctoFHIR 0.5.10's pure evaluator and the embedded R4 model provider. There is no Java
FHIR Validator process, JVM or PostgreSQL requirement for the Rust/Python/WASM products.
Kotlin/JVM applications use their normal JVM and a native Rust library.
See [technical decisions](docs/architecture.md) and [third party notices](THIRD_PARTY_NOTICES.md).

## Rust / CLI / HTTP

Rust **1.98+** is required. The lockfile pins the tested dependency graph.

```sh
cargo build --locked --release -p clins-cli
# Embedded base R4 only; no package downloads.
target/release/clins validate examples/patient.json
# Explicitly download public definitions/examples and verify every SHA-256.
target/release/clins --packages clins-packages.lock.json prepare
# All subsequent validation is offline.
target/release/clins --packages clins-packages.lock.json validate crates/core/tests/fixtures/patient.json
target/release/clins --packages clins-packages.lock.json serve --bind 127.0.0.1:8091
```

CLI exit codes: **0 valid, 1 invalid, 2 input/operational error, 3 incomplete**.
Stdin is supported; `--profile CANONICAL` selects an additional profile.
`--definition path.json` can load operator-selected local R4 definitions instead of packages.

HTTP: `POST /validate` returns a report. `POST /$validate` and
`POST /Patient/$validate` return FHIR `OperationOutcome`; the standard operation accepts
a resource or `Parameters` with `resource`, optional `profile`, and `mode=general`.
`GET /health`, `/metadata` and `/capabilities` expose installed coverage.
The default bind is loopback; HTTP has no built-in authentication. Input is limited to
2 MiB and responses have `Cache-Control: no-store`. Validation never follows patient URLs.

```rust,ignore
use clins_core::Validator;
let validator = Validator::r4()?;
let report = validator.validate_json(r#"{"resourceType":"Patient","active":true}"#, None)?;
assert_eq!(report.status, "valid");
```

## The same CLINS definitions in every SDK

Export a verified definition bundle once. It contains **public definitions**, not patient data.
Bundles are separate assets and are not embedded in language packages.

```sh
target/release/clins --packages clins-packages.lock.json export-definitions > data/definitions.json
```

Pass the bundle JSON string to the SDK constructor. Without it the SDK validates embedded R4.
Pass **resource JSON strings** to preserve decimal precision across language boundaries.

Python ([build instructions](bindings/python/README.md)):

```python
from pathlib import Path
from clins_py import Validator
v = Validator(Path("data/definitions.json").read_text())
report = v.validate('{"resourceType":"Patient","active":true}')
print(report["status"])
```

TypeScript ([build instructions](bindings/typescript/README.md)):

```typescript
import init, { Validator } from 'clins-ts';
await init();
const validator = new Validator(definitionsJson);
const report = validator.validate(resourceJson);
validator.free();
```

Kotlin ([build instructions](bindings/kotlin/README.md)):

```kotlin
import jp.genshi.clins.Validator
Validator(definitionsJson).use { validator ->
    val reportJson = validator.validateJson(resourceJson, null)
}
```

## Verify and distribute

Run `scripts/check.sh` for the Rust gate, including checksum-verified official and synthetic
CLINS fixtures. See [SDK verification](docs/verification.md) for Python, WASM/browser and Kotlin.
CI builds each adapter against the same source and fixture versions.
[Releasing](RELEASING.md) documents source/crate/wheel/npm/Maven artifact preparation.
Registry uploads require the corresponding registry's publisher identity; GitHub release
assets are independently usable. Check release notes for which channels are actually published.

## License

Original project contributions: **Apache-2.0**. Adapted Helios files: **MIT**.
Compiled redistributions retain both notices (`Apache-2.0 AND MIT` metadata).
FHIR packages and terminology retain their own upstream terms. This is an independent,
unofficial project and does not imply endorsement by HL7 Japan or the CLINS service.

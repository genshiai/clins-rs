# Remaining work after the first alpha

The published `v0.1.0-alpha.1` is an experimental working distribution, not a
completed CLINS implementation. This ledger distinguishes work we can implement
from external publication and connection prerequisites.

| Work | Current state | Required to close |
|---|---|---|
| maxLength; integer/decimal minValue/maxValue | Added in development after alpha.1 | Boundary/regression tests and cross-SDK CI; distribute in a subsequent release |
| Date/time and Quantity bounds | Incomplete | Partial-date/time comparison, clock policy and canonical unit conversion with tests |
| CLINS profile slicing | Five converter warnings across three profiles remain | Referral Observation/reslicing (2), discharge nested Bundle profile slices (2), radiology category ValueSet classification (1); test present/missing/misclassified cases |
| General FHIRPath integration | Partial | Local resolve/memberOf/conformsTo, nested resource contexts and numeric precision; unknown results must remain incomplete |
| Terminology and reference handling | Partial | Additional required filters and a documented policy/provider for external targets; no implicit network requests |
| Rule-by-rule conformance evidence | Not complete | Explicit applicable-rule inventory with positive/negative test IDs, independently checked expected outcomes; sample acceptance is insufficient |
| Stable release | Pending engine/conformance work | Repeat CLI/HTTP/Python/WASM/Kotlin gates and publish versioned artifacts with accurate scope claims |
| crates.io / PyPI / npm / Maven Central | Not published | Publisher account/organization, ownership of names/namespaces, secure authentication; do not send secrets in chat |
| National service connection trial | Not run | Participating institution's usable connection/test environment and credentials; test the integrating application |

The five converter warnings do not represent all remaining validation rules.
SDKs already call the same Rust Core, so further rules belong in the Core; separate
validation algorithms for Python, TS and Kotlin are not needed.

## Publisher authentication observation

At the time of this development pass, npm `whoami` returned HTTP 401. No Cargo token
file/environment token or PyPI environment token/pypirc was found by existence-only
checks. Secret contents were not printed. Other credential stores and Maven publisher
configuration have not been audited, so their availability is not asserted.

## Platform scope

The delivered distribution covers Linux x86_64, macOS arm64 and WASM as stated in
the release notes. Windows, Android and Kotlin Multiplatform require explicit platform
build/runtime/distribution work and are not established by the current CI matrix.

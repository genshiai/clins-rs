# Coverage and release claims

The [2026-11-19 upstream audit](audits/2026-11-19-validation-audit.ja.md) found missing
checks that currently return `valid`, including invalid Resource.id syntax, empty arrays,
and disconnected document resources. Zero conversion warnings and sample acceptance
did not expose these gaps. Current `valid` results cannot serve as the sole conformance
or transmission gate. Recognized deferred obligations still produce `incomplete`, but
the release does not yet recognize every missing check.

This alpha validates JSON against embedded FHIR R4 schemas and pinned JP Core / JP-CLINS
packages. It checks structure, primitive shapes, cardinality, choices, fixed/pattern values,
supported slicing, inherited profiles and extensions, FHIRPath invariants, offline required
terminology bindings, Bundle/contained references and resolvable target profiles.

Tests cover official patient examples and mutations, plus synthetic patient, conditions,
allergies, laboratory/infection, referral, discharge, patient summary and medication data.
Medication examples are generic R4 collections; they are not claimed to be a separate CLINS
five-information Bundle profile. Must Support does not automatically mean every element is
required on every instance. National service transport, authentication, institution enrollment
and service-side acceptance are outside instance-validation coverage.

## Three-valued reporting

- `invalid`: an applicable implemented rule was violated. Other checks may still be deferred.
- `incomplete`: no observed violation, but one or more necessary checks could not be completed.
- `valid`: no applicable implemented rule violation or known pending check for this input.

Warnings for narrative recommendations can coexist with valid. Unknown profiles, external
reference targets, unavailable terminology, unresolved converter rules, unsupported functions
and evaluator numeric limits, when recognized as obligations, are not promoted to valid. On FHIR `$validate`, an incomplete
report adds an error issue and a validation-status extension.

## Known limits

The pinned JP Core / CLINS profiles now compile without conversion warnings for
`http://jpfhir.jp/` definitions. Profile discriminators validate actual content,
including referral Observation reslicing and discharge nested Bundles. Required
ValueSet discriminators classify radiology categories using the installed terminology.
A `meta.profile` claim alone never proves membership in a slice.

`resolve()`, `memberOf()` and `conformsTo()` use local resources, terminology and profile
definitions. Resource and datatype conformance are checked recursively with a bounded
request budget. Missing knowledge, unsupported terminology filters, external reference
targets and evaluator numeric limits remain incomplete. `%context`, `%resource` and
`%rootResource` preserve their different meanings; Bundle entry resources have their own
FHIR root, while contained resources keep their containing resource as the root.

Unicode `maxLength` and numeric `minValue[x]` / `maxValue[x]` use inclusive, exact-decimal
comparisons. R4 signed, unsigned and positive integer domains are enforced. Generic
Date/time and Quantity bounds, including relative clock bounds and arbitrary canonical
unit conversion, remain incomplete. No date/time or Quantity min/max constraint occurs
in the differential definitions of the currently pinned packages; this limitation still
applies when a caller loads additional profiles using those constraints.

The two discharge slices reference the same generic JP Bundle profile. Their full
machine-readable profile requirements are checked; prose descriptions are not used to
invent a distinction between them. Zero conversion warnings and passing examples do
not prove exhaustive conformance to every FHIR/CLINS rule or clinical interpretation.
An independent Java reference comparison is available as a development check, with a
pinned validator digest, offline terminology and synthetic examples only.

Generate the actual inventory from checksum-verified packages:

```sh
cargo run --locked -p clins-cli --example rule_inventory > data/rule-inventory.json
cargo run --locked -p clins-cli --example audit_clins > data/official-example-audit.json
```

The Java reference validator is not a runtime or CI dependency. A separately operated
reference comparison can supplement the corpus, but is not a reason to claim certification.

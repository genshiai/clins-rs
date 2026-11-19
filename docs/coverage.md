# Coverage and release claims

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
and evaluator numeric limits are never promoted to valid. On FHIR `$validate`, an incomplete
report adds an error issue and a validation-status extension.

## Known limits

The pinned definition inventory currently retains **five conversion warnings across three
profiles**: referral Observation/profile reslicing (2), discharge nested Bundle profile slices
(2), and JP Core radiology category ValueSet discrimination (1). These warnings are conditional
on the affected populated element/resource type. Their count is not a count of all unfinished
FHIR semantics. `resolve()`, general `memberOf()` and `conformsTo()` invariants without an
appropriate local implementation, nested root-resource context, arbitrary terminology filters,
and unresolvable external targets remain incomplete.

The two discharge slices reference the same generic JP Bundle profile, so their descriptions
alone are not treated as a machine-readable discriminator. The implementation does not invent
a clinical classification from prose. Full conformance requires rule-by-rule positive/negative
coverage, a broader independent corpus and interpretation of ambiguous profile definitions.
Passing sample documents is not proof that every CLINS rule is implemented.

Generate the actual inventory from checksum-verified packages:

```sh
cargo run --locked -p clins-cli --example rule_inventory > data/rule-inventory.json
cargo run --locked -p clins-cli --example audit_clins > data/official-example-audit.json
```

The Java reference validator is not a runtime or CI dependency. A separately operated
reference comparison can supplement the corpus, but is not a reason to claim certification.

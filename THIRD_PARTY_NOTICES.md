# Third party notices

The structural schema model, converter, cooperative tree walk, resolver and embedded
R4 pack in `crates/core/src/{schema,resolver,converter,engine,packs}` and
`crates/core/packs` are derived from **helios-fhir-validator 0.2.7**
(https://crates.io/crates/helios-fhir-validator/0.2.7), Helios Software, MIT.
Its full license is retained in `LICENSES/Helios-MIT.txt`. Modifications remove the
Helios runtime/model/API dependency, limit versions to R4 and integrate the CLINS rules.
This project includes adapted source; it is not a wholly novel validation algorithm.

The Apache-2.0 JP additions originate from `genshiai/helios-jp`, commit
20976320ca9918423bc6cb2cbe40dbfd88ccb44f. No private EMR implementation is included.

The embedded R4 schema pack is generated from HL7 FHIR R4 4.0.1 definitions.
FHIR is a registered trademark of HL7. Consult https://hl7.org/fhir/R4/license.html
for the specification's CC0 notice and separately licensed terminology content.
Downloaded JP Core, CLINS and terminology packages remain separately licensed
and are not committed or relicensed by this repository.

Full locked-workspace dependency license notices are included in `THIRD_PARTY_LICENSES.txt`.
UniFFI components are MPL-2.0 (file-level terms); their unmodified exact source packages
are linked in that inventory. The original binding implementation remains Apache-2.0.

The optional audit metadata in `scripts/upstream-tests.lock.json` is selected from
FHIR/fhir-test-cases, commit `2cfcc672d39fe85dcf358106969cf63e185fd926`,
https://github.com/FHIR/fhir-test-cases, Apache-2.0. Its license is retained in
`LICENSES/FHIR-Test-Cases-Apache-2.0.txt`. The selection and SHA-256 asset lock are
project additions. Downloaded upstream fixtures and expected outcomes are kept in
ignored `data/`; individual terminology/content terms remain applicable. This audit
corpus is not embedded in the Rust Core or language SDK runtime packages.

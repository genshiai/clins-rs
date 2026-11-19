# clins-py

Python 3.10+ binding to the same Rust Core. No JVM or database.

```sh
python -m pip install maturin
maturin build --release --manifest-path bindings/python/Cargo.toml --out dist
python -m pip install dist/clins_py-*.whl
```

```python
from clins_py import Validator
v = Validator()  # embedded R4
report = v.validate('{"resourceType":"Patient","active":true}')
assert report["status"] == "valid"
```

For CLINS, pass the text of the bundle produced by `clins export-definitions` to `Validator`.
`validate_json(resource_json, profile=None)` returns report JSON; `validate(...)` returns a dict.
Malformed JSON or constructor input raises `ValueError`; invalid FHIR returns an invalid report.
Validation releases the GIL. Use input JSON strings to preserve numeric source precision.
Alpha coverage and package licenses are described in the repository documentation.

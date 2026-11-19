#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Optional offline differential check of this repository's synthetic fixtures only.

No downloads, registry authentication or private application files are used. Prepare
the Java validator's *public definition* cache separately; this is a development
oracle, never a runtime dependency of clins-rs.
"""
import argparse
import copy
import hashlib
import json
import re
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
REFERENCE_SHA256 = "2686590539f0b3ba7683149b25d78249f6886c04e2c326e884c5049d8947df89"
NAMES = ["patient", "conditions", "allergies", "laboratory", "referral", "discharge", "patient_summary", "medications"]


def unreachable_organization_probe():
    """Preserve the valid referral; add one otherwise-valid unreferenced resource."""
    value = json.loads((ROOT / "crates/core/tests/fixtures/referral.json").read_text())
    entry = copy.deepcopy(next(e for e in reversed(value["entry"]) if e["resource"]["resourceType"] == "Organization"))
    entry["fullUrl"] = "https://example.org/fhir/Organization/aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa"
    entry["resource"]["id"] = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa"
    value["entry"].append(entry)
    return value


def error_locations(issues, resource_type):
    """Normalize concrete diagnostic paths, retaining genuine location differences."""
    paths = set()
    for issue in issues:
        if issue.get("severity") not in ("fatal", "error"):
            continue
        for path in issue.get("expression", issue.get("location", [])):
            path = re.sub(r"/\*.*?\*/", "", path)
            path = re.sub(r"\.(\d+)(?=\.|$)", r"[\1]", path)
            path = re.sub(r"^Resource(?=\.|\[|$)", resource_type, path)
            paths.add(path)
    return paths


def compare_case(name, expected, native, reference, resource_type):
    errors = [i for i in reference.get("issue", []) if i["severity"] in ("fatal", "error")]
    native_errors = [i for i in native["outcome"].get("issue", []) if i["severity"] in ("fatal", "error")]
    native_paths = error_locations(native["outcome"].get("issue", []), resource_type)
    reference_paths = error_locations(reference.get("issue", []), resource_type)
    return dict(case=name, expected=expected, native=native["status"],
                reference="errors" if errors else "no-errors",
                verdict_agrees=(native["status"] == expected and bool(errors) == (expected == "invalid")),
                reference_errors=errors,
                native_errors=native_errors,
                error_locations=dict(shared=sorted(native_paths & reference_paths),
                                     native_only=sorted(native_paths - reference_paths),
                                     reference_only=sorted(reference_paths - native_paths)),
                native_deferred_checks=native.get("deferred_checks"))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--java", required=True)
    parser.add_argument("--validator", required=True, type=Path)
    parser.add_argument("--cache-home", required=True, type=Path)
    parser.add_argument("--clins", type=Path, default=ROOT / "target/debug/clins")
    parser.add_argument("--include-reachability-probe", action="store_true", help="Add an independently invalid document probe; current alpha.2 is known to miss it, so this run exits 1")
    args = parser.parse_args()
    if hashlib.sha256(args.validator.read_bytes()).hexdigest() != REFERENCE_SHA256:
        parser.error("reference jar must be the pinned, unmodified HL7 validator 7.0.6")
    lock = json.loads((ROOT / "clins-packages.lock.json").read_text())
    for package in lock["packages"]:
        actual = hashlib.sha256((ROOT / package["file"]).read_bytes()).hexdigest()
        if actual != package["sha256"]:
            parser.error("public package checksum mismatch: " + package["name"])
    with tempfile.TemporaryDirectory(prefix="clins-reference-") as directory:
        directory = Path(directory)
        cases = []
        for name in NAMES:
            good = json.loads((ROOT / f"crates/core/tests/fixtures/{name}.json").read_text())
            bad = copy.deepcopy(good)
            if name == "patient":
                bad["identifier"][1]["value"] = "invalid"
            elif name == "conditions":
                del bad["entry"][1]["resource"]["onsetDateTime"]
            elif name in ("allergies", "laboratory"):
                del bad["entry"][1]["resource"]["code"]
            elif name == "medications":
                bad["entry"][1]["resource"]["status"] = "invalid-status"
            else:
                del bad["entry"][0]["resource"]["section"]
            for value, expected in [(good, "valid"), (bad, "invalid")]:
                path = directory / f"{name}-{expected}.json"
                path.write_text(json.dumps(value, ensure_ascii=False))
                cases.append((path, expected))
        if args.include_reachability_probe:
            path = directory / "referral-unreachable-organization-invalid.json"
            path.write_text(json.dumps(unreachable_organization_probe(), ensure_ascii=False))
            cases.append((path, "invalid"))
        outcome_path = directory / "outcomes.json"
        command = [args.java, "-Xmx2g", f"-Duser.home={args.cache_home.resolve()}", "-jar", str(args.validator.resolve())]
        command += [str(path) for path, _ in cases]
        command += ["-version", "4.0.1", "-tx", "n/a", "-no-http-access", "-disable-default-resource-fetcher", "-allow-example-urls=true", "-output", str(outcome_path)]
        for package in lock["packages"]:
            if package["name"] != "hl7.fhir.r4.core":
                command += ["-ig", str(ROOT / package["file"])]
        reference = subprocess.run(command, capture_output=True, text=True, cwd=ROOT)
        if not outcome_path.exists():
            raise RuntimeError("reference validator did not produce outcomes: " + reference.stdout[-4000:] + reference.stderr[-1000:])
        bundle = json.loads(outcome_path.read_text())
        outcomes = {}
        for entry in bundle.get("entry", []):
            resource = entry["resource"]
            filename = next(e["valueString"] for e in resource["extension"] if e["url"].endswith("/operationoutcome-file"))
            outcomes[Path(filename).name] = resource
        results = []
        for path, expected in cases:
            result = subprocess.run([str(args.clins.resolve()), "--packages", str(ROOT / "clins-packages.lock.json"), "validate", str(path)], capture_output=True, text=True, cwd=ROOT)
            native = json.loads(result.stdout)
            typ = json.loads(path.read_text())["resourceType"]
            results.append(compare_case(path.stem, expected, native, outcomes[path.name], typ))
        agreed = all(r["verdict_agrees"] for r in results)
        print(json.dumps(dict(schema_version=2, comparison_level="expected verdict and error presence; normalized error locations reported separately", reference_version="7.0.6", reference_sha256=REFERENCE_SHA256, terminology_network=False, example_urls_allowed=True, scope="Eight independent synthetic examples and eight negative mutations, plus an optional isolated document reachability probe; not exhaustive FHIR conformance or national certification. No Java errors is not a proof of complete validation. Java additionally loads its cached standard support packages.", all_agreed=agreed, packages=lock["packages"], results=results), ensure_ascii=False, indent=2))
        if not agreed:
            raise SystemExit(1)


if __name__ == "__main__":
    main()

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
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
REFERENCE_SHA256 = "2686590539f0b3ba7683149b25d78249f6886c04e2c326e884c5049d8947df89"
NAMES = ["patient", "conditions", "allergies", "laboratory", "referral", "discharge", "patient_summary", "medications"]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--java", required=True)
    parser.add_argument("--validator", required=True, type=Path)
    parser.add_argument("--cache-home", required=True, type=Path)
    parser.add_argument("--clins", type=Path, default=ROOT / "target/debug/clins")
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
            reference_errors = [i for i in outcomes[path.name].get("issue", []) if i["severity"] in ("fatal", "error")]
            result = subprocess.run([str(args.clins.resolve()), "--packages", str(ROOT / "clins-packages.lock.json"), "validate", str(path)], capture_output=True, text=True, cwd=ROOT)
            native = json.loads(result.stdout)
            reference_status = "invalid" if reference_errors else "valid"
            results.append(dict(case=path.stem, expected=expected, native=native["status"], reference=reference_status, reference_errors=reference_errors))
        agreed = all(r["native"] == r["reference"] == r["expected"] for r in results)
        print(json.dumps(dict(reference_version="7.0.6", reference_sha256=REFERENCE_SHA256, terminology_network=False, example_urls_allowed=True, scope="Eight independent synthetic examples and eight negative mutations; acceptance/error agreement, not exhaustive FHIR conformance or national certification. Java additionally loads its cached standard support packages.", all_agreed=agreed, packages=lock["packages"], results=results), ensure_ascii=False, indent=2))
        if not agreed:
            raise SystemExit(1)


if __name__ == "__main__":
    main()

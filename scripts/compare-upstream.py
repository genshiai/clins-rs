#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Pinned, offline R4 differential audit; run with the Python SDK installed.

`lock` explicitly downloads public upstream fixtures and writes their SHA-256 lock.
`prepare` fetches only locked public files. `run` never downloads or sends resources.
The initial subset excludes cases needing additional manifest settings, not failures.
"""
import argparse
import concurrent.futures
from collections import Counter
import hashlib
import importlib.util
import json
import multiprocessing
from pathlib import Path, PurePosixPath
import subprocess
import sys
import time
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
REVISION = "2cfcc672d39fe85dcf358106969cf63e185fd926"
MANIFEST_SHA = "9fbaef7b2317581ca2f7271b3b568bd96e96120a0c193fa789438efe64cd8d25"
LOCK = ROOT / "scripts/upstream-tests.lock.json"
CACHE = ROOT / "data/upstream-tests" / REVISION
BASE = f"https://raw.githubusercontent.com/FHIR/fhir-test-cases/{REVISION}/"
ALLOWED = {"name", "file", "description", "explanation", "version", "module", "java", "firely-sdk-current", "firely-sdk-wip", "use-test"}
spec = importlib.util.spec_from_file_location("reference", ROOT / "scripts/compare-reference.py")
reference = importlib.util.module_from_spec(spec)
spec.loader.exec_module(reference)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def safe_path(name):
    path = PurePosixPath(name)
    if path.is_absolute() or ".." in path.parts:
        raise ValueError("unsafe corpus path")
    return CACHE / name


def fetch(name, digest=None):
    target = safe_path(name)
    if target.exists():
        data = target.read_bytes()
    else:
        for attempt in range(3):
            try:
                with urllib.request.urlopen(BASE + name, timeout=45) as response:
                    data = response.read(16 * 1024 * 1024 + 1)
                break
            except Exception:
                if attempt == 2:
                    raise
                time.sleep(1)
        if len(data) > 16 * 1024 * 1024:
            raise ValueError("upstream asset too large")
        if digest and sha(data) != digest:
            raise ValueError("upstream checksum mismatch: " + name)
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
    if digest and sha(data) != digest:
        raise ValueError("upstream checksum mismatch: " + name)
    return data


def select_cases(manifest):
    candidates, selected, skipped = [], [], []
    for case in manifest["test-cases"]:
        if str(case.get("version")) not in ("4.0", "4.0.1") or not case.get("file", "").endswith(".json") or case.get("use-test") is False:
            continue
        candidates.append(case)
        settings = sorted(set(case) - ALLOWED)
        if settings or not isinstance(case.get("java"), str):
            skipped.append(dict(name=case["name"], reason="additional manifest configuration", settings=settings))
        else:
            selected.append(case)
    return candidates, selected, skipped


def prepare(update):
    if update:
        manifest = json.loads(fetch("validator/manifest.json", MANIFEST_SHA))
        candidates, selected, skipped = select_cases(manifest)
        paths = {"LICENSE.txt", "validator/manifest.json"}
        for case in selected:
            paths.update(["validator/" + case["file"], "validator/outcomes/" + case["java"]])
        with concurrent.futures.ThreadPoolExecutor(max_workers=8) as pool:
            assets = dict(zip(sorted(paths), pool.map(fetch, sorted(paths))))
        lock = dict(schema_version=1, repository="https://github.com/FHIR/fhir-test-cases", revision=REVISION,
                    license="Apache-2.0; retain upstream and individual content notices", r4_json_candidates=len(candidates),
                    selected=selected, skipped=skipped, files={p: sha(data) for p, data in assets.items()})
        LOCK.write_text(json.dumps(lock, ensure_ascii=False, indent=2) + "\n")
    else:
        lock = json.loads(LOCK.read_text())
        if lock["revision"] != REVISION:
            raise ValueError("unexpected corpus revision")
        with concurrent.futures.ThreadPoolExecutor(max_workers=8) as pool:
            list(pool.map(lambda item: fetch(*item), lock["files"].items()))
    print(json.dumps(dict(candidates=lock["r4_json_candidates"], selected=len(lock["selected"]), skipped=len(lock["skipped"]))))


def worker(connection, definitions):
    from clins_py import Validator, __version__, _clins
    validator = Validator(Path(definitions).read_text())
    connection.send(dict(ready=True, engine_version=__version__, native_binary_sha256=sha(Path(_clins.__file__).read_bytes())))
    while True:
        value = connection.recv()
        if value is None:
            return
        try:
            report = validator.validate(value)
            connection.send({k: report[k] for k in ("status", "deferred_checks", "outcome")})
        except ValueError as error:
            connection.send(dict(status="input-rejected", error=str(error)))
        except BaseException as error:
            connection.send(dict(status="runtime-error", error=type(error).__name__ + ": " + str(error)))


def has_errors(outcome):
    return any(i.get("severity") in ("error", "fatal") for i in outcome.get("issue", []))


def classification(native, java):
    if native["status"] not in ("valid", "invalid"):
        return native["status"]
    if native["status"] == "invalid":
        return "both-reject" if java else "rust-only-rejects"
    return "java-only-rejects" if java else "both-no-errors"


def summarize(results):
    counts = Counter(r["classification"] for r in results)
    comparable = sum(counts[k] for k in ("both-reject", "both-no-errors", "rust-only-rejects", "java-only-rejects"))
    agreed = counts["both-reject"] + counts["both-no-errors"]
    return dict(counts=dict(counts), total=len(results), comparable=comparable,
                verdict_agreements=agreed, agreement_percent_comparable=round(100 * agreed / comparable, 2) if comparable else None,
                observed_agreements_percent_all=round(100 * agreed / len(results), 2) if results else None,
                live_reference_matches_recorded=sum(r.get("live_java_matches_recorded_error_presence", False) for r in results))


def run(args):
    lock = json.loads(LOCK.read_text())
    for name, digest in lock["files"].items():
        if sha(safe_path(name).read_bytes()) != digest:
            raise ValueError("cached upstream checksum mismatch: " + name)
    if sha(args.validator.read_bytes()) != reference.REFERENCE_SHA256:
        raise ValueError("reference jar checksum mismatch")
    package_lock = json.loads((ROOT / "clins-packages.lock.json").read_text())
    core = next(p.copy() for p in package_lock["packages"] if p["name"] == "hl7.fhir.r4.core")
    if sha((ROOT / core["file"]).read_bytes()) != core["sha256"]:
        raise ValueError("R4 package checksum mismatch")
    core["file"] = str(ROOT / core["file"])
    run_dir = ROOT / "data/upstream-audit"
    run_dir.mkdir(parents=True, exist_ok=True)
    core_lock = run_dir / "r4.lock.json"
    core_lock.write_text(json.dumps(dict(packages=[core])))
    definitions = run_dir / "r4-definitions.json"
    with definitions.open("w") as output:
        subprocess.run([str(args.clins.resolve()), "--packages", str(core_lock), "export-definitions"], stdout=output, check=True)
    cases = lock["selected"]
    output = run_dir / "java-outcomes.json"
    output.unlink(missing_ok=True)
    command = [args.java, "-Xmx2g", f"-Duser.home={args.cache_home.resolve()}", "-jar", str(args.validator.resolve())]
    command += [str(safe_path("validator/" + c["file"])) for c in cases]
    command += ["-version", "4.0.1", "-tx", "n/a", "-no-http-access", "-disable-default-resource-fetcher", "-allow-example-urls=true", "-best-practice", "ignore", "-language", "en", "-output", str(output)]
    print(f"Running Java reference on {len(cases)} upstream cases", file=sys.stderr, flush=True)
    with (run_dir / "java.log").open("w") as log:
        java = subprocess.run(command, stdout=log, stderr=subprocess.STDOUT, timeout=900)
    if not output.exists():
        raise RuntimeError("Java did not produce outcomes; inspect data/upstream-audit/java.log")
    bundle = json.loads(output.read_text())
    outcomes = {}
    for entry in bundle.get("entry", []):
        outcome = entry["resource"]
        filename = next(e["valueString"] for e in outcome.get("extension", []) if e["url"].endswith("/operationoutcome-file"))
        outcomes[str(Path(filename).resolve())] = outcome
    results = []
    process = connection = None
    try:
        for number, case in enumerate(cases, 1):
            path = safe_path("validator/" + case["file"])
            oracle = json.loads(safe_path("validator/outcomes/" + case["java"]).read_text())
            live = outcomes.get(str(path.resolve()))
            if live is None:
                results.append(dict(case=case["name"], classification="missing-reference-outcome"))
                continue
            if not process or not process.is_alive():
                connection, child = multiprocessing.Pipe()
                process = multiprocessing.Process(target=worker, args=(child, str(definitions)))
                process.start()
                if not connection.poll(60):
                    raise RuntimeError("Rust SDK worker initialization failed")
                ready = connection.recv()
                sdk_version = ready["engine_version"]
                print("Rust SDK " + ready["engine_version"], file=sys.stderr, flush=True)
            connection.send(path.read_text())
            if connection.poll(args.timeout):
                try:
                    native = connection.recv()
                except EOFError:
                    native = dict(status="worker-crash")
            else:
                process.terminate()
                process.join(5)
                native = dict(status="timeout")
            kind = classification(native, has_errors(live))
            result = dict(case=case["name"], file=case["file"], module=case.get("module"), classification=kind,
                          native=native, java_errors=[i for i in live.get("issue", []) if i.get("severity") in ("fatal", "error")],
                          upstream_expected_errors=[i for i in oracle.get("issue", []) if i.get("severity") in ("fatal", "error")],
                          live_java_matches_recorded_error_presence=(has_errors(live) == has_errors(oracle)))
            results.append(result)
            print(f"{number}/{len(cases)} {case['name']}: {kind}", file=sys.stderr, flush=True)
    finally:
        if process and process.is_alive():
            process.terminate()
            process.join(5)
    report = dict(schema_version=1, corpus_revision=REVISION, source_revision=subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
                  r4_json_candidates=lock["r4_json_candidates"], selected=len(cases), skipped=len(lock["skipped"]),
                  skipped_cases=lock["skipped"], **summarize(results), java_exit_code=java.returncode, native_sdk_version=sdk_version,
                  reference_sha256=reference.REFERENCE_SHA256, corpus_lock_sha256=sha(LOCK.read_bytes()),
                  audit_runner_sha256=sha(Path(__file__).read_bytes()), native_binary_sha256=ready["native_binary_sha256"],
                  reference_outcome_sha256=sha(output.read_bytes()),
                  settings=dict(fhir_version="4.0.1", terminology_network=False, reference_fetching=False, additional_igs=False,
                                examples=True, best_practice="ignore in Java; warning in Rust", language="Java en", native_case_timeout_seconds=args.timeout),
                  caveats=["Subset selected by supported manifest configuration, not observed verdicts", "Java CLI offline environment differs from upstream's complete test harness and terminology services", "No-errors is not proof of completed validation", "Java cached standard support packages are additional reference dependencies"], results=results)
    print(json.dumps(report, ensure_ascii=False, indent=2))
    if args.fail_on_divergence and any(r["classification"] not in ("both-reject", "both-no-errors") for r in results):
        raise SystemExit(1)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=["lock", "prepare", "run"])
    parser.add_argument("--java")
    parser.add_argument("--validator", type=Path)
    parser.add_argument("--cache-home", type=Path)
    parser.add_argument("--clins", type=Path, default=ROOT / "target/debug/clins")
    parser.add_argument("--timeout", type=int, default=20)
    parser.add_argument("--fail-on-divergence", action="store_true", help="Exit 1 for divergent, incomplete, input-rejected or operational-error cases; by default exit 0 means the audit completed, not conformance")
    args = parser.parse_args()
    if args.mode == "run":
        if not (args.java and args.validator and args.cache_home):
            parser.error("run requires --java, --validator and --cache-home")
        run(args)
    else:
        prepare(args.mode == "lock")


if __name__ == "__main__":
    main()

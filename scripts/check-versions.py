#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Reject releases whose native library and language package versions disagree."""
import json
from pathlib import Path
import re
import tomllib

root = Path(__file__).resolve().parents[1]
workspace = tomllib.loads((root / "Cargo.toml").read_text())["workspace"]
version = workspace["package"]["version"]
for name in ("clins-core", "clins-packages", "clins-server"):
    assert workspace["dependencies"][name]["version"] == version, name
assert json.loads((root / "bindings/typescript/package.json").read_text())["version"] == version
python_version = re.sub(r"-alpha\.(\d+)$", r"a\1", version)
assert tomllib.loads((root / "bindings/python/pyproject.toml").read_text())["project"]["version"] == python_version
gradle_version = re.search(r'^version = "([^"]+)"$', (root / "bindings/kotlin/build.gradle.kts").read_text(), re.M)[1]
assert gradle_version == version
print("Consistent Rust/Python/TS/Kotlin versions:", version)

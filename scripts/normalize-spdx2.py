#!/usr/bin/env python3
import hashlib
import json
import re
import sys
from pathlib import Path


def walk_strings(value):
    if isinstance(value, dict):
        for item in value.values():
            yield from walk_strings(item)
    elif isinstance(value, list):
        for item in value:
            yield from walk_strings(item)
    elif isinstance(value, str):
        yield value


def replace_strings(value, replacements):
    if isinstance(value, dict):
        return {key: replace_strings(item, replacements) for key, item in value.items()}
    if isinstance(value, list):
        return [replace_strings(item, replacements) for item in value]
    return replacements.get(value, value)


def canonical_ids(document):
    identifiers = sorted(
        {value for value in walk_strings(document) if value.startswith("SPDXRef-")}
    )
    replacements = {}
    used = set()
    for identifier in identifiers:
        candidate = "SPDXRef-" + re.sub(r"[^A-Za-z0-9.-]", "-", identifier[8:])
        candidate = re.sub(r"-+", "-", candidate).rstrip("-")
        unique = candidate
        suffix = 2
        while unique in used:
            unique = f"{candidate}-{suffix}"
            suffix += 1
        replacements[identifier] = unique
        used.add(unique)
    return replacements


def main():
    if len(sys.argv) != 4:
        raise SystemExit("usage: normalize-spdx2.py INPUT OUTPUT CARGO_LOCK")

    source, output, lockfile = map(Path, sys.argv[1:])
    document = json.loads(source.read_text(encoding="utf-8"))
    document = replace_strings(document, canonical_ids(document))
    lock_hash = hashlib.sha256(lockfile.read_bytes()).hexdigest()
    document["documentNamespace"] = (
        "https://github.com/themoretheless/camera-man/sbom/" + lock_hash
    )

    creators = document.setdefault("creationInfo", {}).setdefault("creators", [])
    organization = "Organization: CameraMan contributors"
    if organization not in creators:
        creators.insert(0, organization)

    for package in document.get("packages", []):
        location = package.get("downloadLocation")
        if isinstance(location, str) and location.startswith("registry+"):
            package["downloadLocation"] = location.removeprefix("registry+")
        if package.get("name") == "camera-man" and not package.get("externalRefs"):
            package["externalRefs"] = [
                {
                    "referenceCategory": "PACKAGE-MANAGER",
                    "referenceType": "purl",
                    "referenceLocator": (
                        f"pkg:cargo/camera-man@{package.get('versionInfo', '0.0.0')}"
                    ),
                }
            ]

    output.write_text(
        json.dumps(document, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )


if __name__ == "__main__":
    main()

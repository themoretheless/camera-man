#!/usr/bin/env python3
import plistlib
import sys
from pathlib import Path


PLACEHOLDER = "__CAMERAMAN_APP_GROUP__"


def substitute(value, application_group):
    if isinstance(value, dict):
        return {key: substitute(item, application_group) for key, item in value.items()}
    if isinstance(value, list):
        return [substitute(item, application_group) for item in value]
    return application_group if value == PLACEHOLDER else value


def main():
    if len(sys.argv) != 4:
        raise SystemExit(
            "usage: compare-entitlements.py ACTUAL EXPECTED APPLICATION_GROUP"
        )

    actual_path = Path(sys.argv[1])
    expected_path = Path(sys.argv[2])
    application_group = sys.argv[3]
    actual = plistlib.loads(actual_path.read_bytes())
    expected = substitute(
        plistlib.loads(expected_path.read_bytes()), application_group
    )
    if actual == expected:
        print(f"entitlements match {expected_path.name}")
        return

    actual_keys = set(actual)
    expected_keys = set(expected)
    details = []
    if missing := sorted(expected_keys - actual_keys):
        details.append(f"missing keys: {', '.join(missing)}")
    if extra := sorted(actual_keys - expected_keys):
        details.append(f"unexpected keys: {', '.join(extra)}")
    for key in sorted(actual_keys & expected_keys):
        if actual[key] != expected[key]:
            details.append(
                f"{key}: expected {expected[key]!r}, found {actual[key]!r}"
            )
    raise SystemExit("entitlement allowlist mismatch: " + "; ".join(details))


if __name__ == "__main__":
    main()

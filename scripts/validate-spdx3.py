#!/usr/bin/env python3
import json
import sys
from pathlib import Path


CONTEXT = "https://spdx.org/rdf/3.0.1/spdx-context.jsonld"


def creation_infos(value):
    if isinstance(value, dict):
        if "specVersion" in value:
            yield value
        for item in value.values():
            yield from creation_infos(item)
    elif isinstance(value, list):
        for item in value:
            yield from creation_infos(item)


def main():
    if len(sys.argv) != 3:
        raise SystemExit("usage: validate-spdx3.py SPDX3_JSONLD SPDX2_JSON")

    document = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
    source = json.loads(Path(sys.argv[2]).read_text(encoding="utf-8"))
    if document.get("@context") != CONTEXT:
        raise SystemExit("SPDX 3 document does not use the official 3.0.1 context")

    graph = document.get("@graph")
    if not isinstance(graph, list) or not graph:
        raise SystemExit("SPDX 3 graph is empty")
    identifiers = [element.get("@id") for element in graph if element.get("@id")]
    if len(identifiers) != len(set(identifiers)):
        raise SystemExit("SPDX 3 graph contains duplicate @id values")

    documents = [item for item in graph if item.get("@type") == "SpdxDocument"]
    if len(documents) != 1:
        raise SystemExit("SPDX 3 graph must contain exactly one SpdxDocument")
    if any(info["specVersion"] != "3.0.1" for info in creation_infos(document)):
        raise SystemExit("SPDX 3 graph contains a creationInfo other than 3.0.1")

    packages = [item for item in graph if item.get("@type") == "Package"]
    if len(packages) != len(source.get("packages", [])):
        raise SystemExit("SPDX package count changed during 2.3 to 3.0.1 conversion")
    if not any(item.get("name") == "camera-man" for item in packages):
        raise SystemExit("SPDX graph does not describe the CameraMan package")
    for package in packages:
        if "declaredLicense" not in package or "concludedLicense" not in package:
            raise SystemExit(f"package lacks license fields: {package.get('name')}")

    relationships = [
        item
        for item in graph
        if item.get("@type") in ("Relationship", "SoftwareDependencyRelationship")
    ]
    relationship_edges = sum(len(item.get("to", [])) for item in relationships)
    if relationship_edges != len(source.get("relationships", [])):
        raise SystemExit("SPDX relationship edge count changed during conversion")
    print(
        f"validated SPDX 3.0.1 JSON-LD: {len(packages)} packages, "
        f"{relationship_edges} relationship edges"
    )


if __name__ == "__main__":
    main()

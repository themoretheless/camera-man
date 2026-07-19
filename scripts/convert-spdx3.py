#!/usr/bin/env python3
import contextlib
import io
import json
import sys
from pathlib import Path

from spdx_tools.spdx.parser.parse_anything import parse_file
from spdx_tools.spdx.validation.document_validator import validate_full_spdx_document
from spdx_tools.spdx.spdx_element_utils import get_full_element_spdx_id
from spdx_tools.spdx3.bump_from_spdx2.license_expression import (
    bump_license_expression_or_none_or_no_assertion,
)
from spdx_tools.spdx3.bump_from_spdx2.spdx_document import bump_spdx_document
from spdx_tools.spdx3.writer.json_ld.json_ld_converter import (
    convert_payload_to_json_ld_list_of_elements,
)


CONTEXT = "https://spdx.org/rdf/3.0.1/spdx-context.jsonld"


def promote_spec_version(value):
    if isinstance(value, dict):
        return {
            key: (
                "3.0.1"
                if key == "specVersion" and item in ("3.0.0", "3.0.1")
                else promote_spec_version(item)
            )
            for key, item in value.items()
        }
    if isinstance(value, list):
        return [promote_spec_version(item) for item in value]
    return value


def main():
    if len(sys.argv) != 3:
        raise SystemExit("usage: convert-spdx3.py INPUT.spdx.json OUTPUT.jsonld")

    source, output = map(Path, sys.argv[1:])
    document = parse_file(str(source))
    messages = validate_full_spdx_document(document, "SPDX-2.3")
    if messages:
        details = "\n".join(message.validation_message for message in messages[:20])
        raise SystemExit(f"normalized SPDX 2.3 input is invalid:\n{details}")

    converter_log = io.StringIO()
    with contextlib.redirect_stdout(converter_log), contextlib.redirect_stderr(
        converter_log
    ):
        payload = bump_spdx_document(document)

    namespace = document.creation_info.document_namespace
    for package in document.packages:
        spdx_id = get_full_element_spdx_id(
            package, namespace, document.creation_info.external_document_refs
        )
        converted = payload.get_element(spdx_id)
        converted.declared_license = bump_license_expression_or_none_or_no_assertion(
            package.license_declared, document.extracted_licensing_info
        )
        converted.concluded_license = bump_license_expression_or_none_or_no_assertion(
            package.license_concluded, document.extracted_licensing_info
        )

    result = {
        "@context": CONTEXT,
        "@graph": promote_spec_version(
            convert_payload_to_json_ld_list_of_elements(payload)
        ),
    }
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()

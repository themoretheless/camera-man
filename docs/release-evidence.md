# Release evidence model

Release evidence is layered. Passing one layer never implies the others.

| Evidence | What it proves | What it does not prove |
|---|---|---|
| Apple code signature | Bundle bytes were signed by the displayed Team ID and nested code satisfies signing rules | Apple reviewed the binary, the archive is current, or dependencies are safe |
| Stapled notarization ticket | Apple accepted the submitted bundle and Gatekeeper can validate the ticket offline | Source provenance, reproducibility or future revocation status |
| `CameraMan.zip.sha256` | The downloaded archive matches one published digest | Who published the digest or whether the matching archive is trustworthy |
| SPDX 2.3/3.0.1 SBOM | Declared locked components and relationships at build time | Absence of vulnerabilities or proof that the SBOM matches a binary by itself |
| SBOM provenance attestation | GitHub OIDC-bound workflow attested the exact SBOM digest | A detached author signature or correctness of every SBOM field |
| Binary/archive provenance attestation | The release workflow attested exact artifact digests for one commit/run | Reproducibility on unrelated infrastructure or product correctness |
| `release-manifest.json` | Commit, toolchain, SDK, bundle IDs, Team ID and evidence hashes agree in one machine-readable index | A signature by itself; verify its hosted attestation too |

Production CI builds host and extension twice with remapped paths and fixed
inputs, compares unsigned Mach-O bytes, signs one compared copy, then audits the
binaries extracted from the final ZIP. It scans the normalized SPDX 2.3 SBOM
with OSV-Scanner and publishes the SPDX 3.0.1 representation as well.

Any ignored vulnerability requires an owner, review/expiry date, mitigation and
removal condition in `docs/dependency-exceptions.md` plus the corresponding
machine-readable scanner configuration. Source-only `cargo audit` is useful but
is not accepted as final-artifact evidence.

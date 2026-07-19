# Release rollback and revocation

This procedure is for a defective or compromised signed release. It does not
silently downgrade user data and never asks users to disable Gatekeeper.

## Immediate response

1. Stop publication and mark the affected version withdrawn in the release channel.
2. Preserve the ZIP, manifest, SBOM, attestations, notary response and CI logs as incident evidence.
3. Classify whether the signing identity, notarization credential, GitHub OIDC/workflow, dependency graph or application behavior is compromised.
4. Rotate/revoke only affected credentials; revoke a Developer ID certificate through Apple when signature trust is in doubt.
5. Publish exact affected versions/digests, impact, safe workaround and the expected replacement path. Never reuse the withdrawn version.

## Replacement release

1. Branch from the last trusted commit and apply the smallest reviewed fix.
2. Keep `SCENE_SCHEMA_VERSION`, preferences schema and transport migrations able to read data written by the withdrawn release. If impossible, ship a tested export/recovery tool before replacement.
3. Run the complete release workflow with fresh version/build numbers and fresh provenance. A copied old notarization ticket, checksum or manifest is invalid.
4. Test install over the withdrawn build and over the previous good build, then verify scene/config preservation and explicit output-off startup.
5. Publish the replacement digest and attestation links, then update the incident with revocation and recovery status.

## User recovery

Users may quit CameraMan, remove only the affected application bundle and
install the replacement. Scene and preference files remain in Application
Support and are migrated forward by normal validated readers. Removing the
system extension uses the documented macOS flow; do not delete shared state as
an installation shortcut. A rollback to an older app is allowed only when that
version can read the current persisted schema or after a verified export.

## Closure

The incident closes only after credentials and hosted artifacts are accounted
for, the bad release is no longer offered, recovery has been exercised on a
clean Mac, and preventive tests or policy changes are merged.

# Release readiness gate

Automation may build a candidate, but distribution remains blocked until every
row has evidence tied to the same commit and version.

| Gate | Required evidence |
|---|---|
| AC performance baseline | ARM64 and x86_64 process distributions meet the frame budget while connected to power |
| Stability | Eight-hour soak report has bounded memory/handles, no output stall and valid frame integrity |
| Installation | Paid Developer ID profiles install and activate the nested system extension on a clean supported Mac |
| Apple trust | Notarization accepted, ticket stapled, `spctl` assessment passed |
| Real consumer | FaceTime/OBS/QuickTime or equivalent CMIO client receives live frames and survives stop/start/reconnect |
| Accessibility | VoiceOver smoke checklist passes Start, edit, self-test, Stop and failure recovery |
| Visual QA | All deterministic 1x/2x fixtures pass and representative quality review is recorded |
| Supply chain | Final ZIP binary audit, SBOM scan, manifest, checksums and attestations are published |

Missing external credentials or hardware is a visible blocked gate, never a
reason to mark the milestone complete from unit tests alone.

# Self-update threat model gate

CameraMan has no updater, download client, TLS stack or remote release identity.
Adding any of them is a security-sensitive product change, not release plumbing.
The default and experimental feature graphs must remain network-free until this
document is replaced by an approved design and adversarial review.

## Blocked design questions

An updater cannot ship until one reviewed proposal answers all of these:

| Threat | Required control and recovery evidence |
|---|---|
| Rollback attack | Monotonic trusted version plus an explicit, audited emergency downgrade ceremony |
| Freeze attack | Expiring metadata, trusted time behavior and an offline/manual update escape hatch |
| Online key compromise | Threshold roles, short-lived online metadata and fast revocation without replacing the root blindly |
| Root/threshold rotation | Tested old-root to new-root transition, quorum loss procedure and durable recovery media |
| Mirror/CDN compromise | Hash- and length-bound targets signed independently of transport |
| Partial install or power loss | Staged verification, atomic activation and automatic boot of the last verified application |
| Local privilege crossing | Helper boundary, authorization scope, audit log and least-privilege installation path |
| Endless retry or disk exhaustion | Bounded download, retry, cache and rollback storage budgets |
| Schema incompatibility | Forward-compatible scenes/preferences or preflight refusal before activation |
| Telemetry/privacy leak | No update telemetry by default; any request inventory and retention require separate approval |

## Acceptance evidence

The proposal must include protocol test vectors, compromised-key and expired
metadata simulations, rollback/freeze tests, interrupted-install fault
injection, root-rotation rehearsal, independent security review and a manual
recovery path. `rustls`, TUF libraries or an update UI are not accepted before
the threat model because dependencies cannot create the missing trust policy.

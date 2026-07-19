# Remote source threat model

Status: release gate; no remote-source feature exists yet

## Assets and trust boundaries

- Camera frames and metadata are private user content.
- Peer identity, credentials, pairing state, and discovery names are sensitive.
- Local UI/process, local network, remote peer, credential store, and parser are
  separate trust zones.
- A discovered endpoint is untrusted until authenticated; network reachability
  is never identity.

## Required controls

| Threat | Required control before implementation can ship |
|---|---|
| Peer impersonation | authenticated pairing, explicit identity display, key rotation and removal |
| Passive capture | encrypted transport with current protocol versions and no plaintext fallback |
| Replay/reordering | session nonce, authenticated sequence window, generation-aware restart rules |
| Parser exploitation | Sans-I/O parser, bounded bytes/depth/items/allocations, fuzz corpus, no unsafe parser code |
| CPU/memory exhaustion | pre-auth rate limit, bounded queues, frame/format caps, deadlines and disconnect policy |
| Discovery abuse | opt-in local-network permission, bounded records, escaped names, no automatic connection |
| Credential disclosure | Keychain storage, redacted diagnostics, no secrets in scene/preferences/export |
| Downgrade | authenticated capability transcript and explicit minimum protocol/security version |
| Stale producer | monotonic heartbeat, generation restart, stale timeout, visible recovery state |
| Compromised key | revocation/removal workflow, re-pair requirement, audit event without secret material |

## Acceptance gates

The feature remains non-default until parser/state fuzzing, resource-limit
tests, replay and reconnect histories, slow-consumer tests, credential redaction,
and an independent security review pass. None of these gates may be replaced by
repository popularity or transport-library defaults.

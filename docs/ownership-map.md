# State ownership and reading route

Read the project from stable data toward effects. Each module has one primary
reason to change; adapters may depend inward, while domain modules do not know
about UI, CMIO, capture, or transport.

| Class | Source of truth | Derived/runtime consumers | Start here |
|---|---|---|---|
| Media domain | `frame.rs`, `media_contract.rs`, `format_negotiation.rs` | compositor, transport, extension | `Frame` -> `FrameContract` -> `StreamCapability` |
| Scene persistence | `scene_schema.rs`, `app_preferences.rs` | `CameraManApp`, undo snapshots | `SceneDocument::from_json` -> validation/migration -> app restore |
| Scene runtime | `CameraManApp` fields | `SceneSnapshot`, `InvalidationTargets`, `RenderJob` | UI command -> reducer/history -> invalidation -> latest render job |
| Capture runtime | one `ThreadedNokhwaFrameSource` per selected device | `CapturedFrame`, per-source health | discovery message -> source worker -> integrity/health -> composition |
| Composition | `Compositor` plus explicit `VideoFormat` | preview conversion, sink publication | layout -> source transforms -> canonical BGRA output |
| IPC protocol | mmap header/slot state in `shared_memory_transport` | borrowed reader frame, producer/consumer progress | writer CAS -> payload -> READY -> reader lease -> acknowledgement |
| Extension runtime | `StreamLifecycle` and `LatestFrameReader` | CVPixelBuffer/sample delivery | transport observation -> integrity state -> pacing -> CMIO sample |
| Diagnostics | atomic counters and bounded event ring | diagnostics export, soak/bench reports | stage/drop/copy record -> snapshot -> redacted JSON |

Persisted/source-of-truth values are versioned and validated before use.
Runtime-owned state is confined to its worker or coordinator. Derived state can
be discarded and rebuilt; it must never be written back as an independent
second truth.

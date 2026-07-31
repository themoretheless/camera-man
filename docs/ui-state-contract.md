# UI state and recovery contract

The workspace hierarchy is Sources -> Preview -> Output/Status. Setup is a
separate task surface for scenes, extension activation, self-test and support
exports. Runtime state must not turn these sections into nested cards.

| State | Primary signal | Recovery |
|---|---|---|
| Empty | `No sources selected`, disabled Start | Select a source in Sources |
| Permission denied | Sticky operation/cause message | Allow CameraMan in Privacy & Security, retry |
| Disconnected | Named source plus disconnected preview/status | Reconnect, refresh cameras, retry |
| Not responding | `Camera is not responding` on the overlay, the preview's accessible name and the status line (the sticky timeout message takes the line until dismissed), plus a RETRY badge | Retry the source, reconnect the camera, export diagnostics if it repeats |
| Stale | Named source health and stale counter | Retry source or choose missing-source policy |
| Extension missing | Setup readiness checklist | Install a signed bundle in `/Applications` |
| Activation pending | Static/animated progress plus text | Approve in System Settings or close Setup |
| Install failed | Cause and Retry before technical details | Correct signing/profile issue, retry |
| Running | Stop command, LIVE badge and status text | Stop releases cameras and transport |

All dynamic source rows use stable source IDs for egui/AccessKit identity.
Pointer editing has keyboard or numeric equivalents. Color is supplementary to
text, shape and accessible descriptions. Reduced Motion replaces indefinite
spinners with a static progress symbol.

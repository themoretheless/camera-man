# CameraMan Recommendations

Exactly 700 numbered items: done work, improvements, problems, mistakes, design notes, and research-backed next steps.

## Iteration 1: Rust Core, SOLID, DRY

1. Сделано: проект переведён в Rust-only основу.
2. Сделано: `Cargo.toml` описывает библиотеку и бинарники.
3. Сделано: `src/lib.rs` экспортирует публичные модули.
4. Сделано: `src/main.rs` содержит CLI-вход.
5. Сделано: `VideoFormat` централизует размер, fps и pixel format.
6. Сделано: `VirtualCameraConfig` хранит имя и UID виртуальной камеры.
7. Сделано: `PixelFormat::Bgra8` задаёт layout пикселя.
8. Сделано: `Frame::new_checked` валидирует размеры и длину буфера.
9. Сделано: `FrameMetadata` отделяет метаданные от пикселей.
10. Сделано: `CapturedFrame` связывает кадр и метаданные.
11. Сделано: `CompositionLayout` описывает grid, row и column.
12. Сделано: `GridLayoutCalculator` отделён от рендера.
13. Сделано: `Compositor` занимается только композицией.
14. Сделано: `FrameSource` отделяет захват от рендера.
15. Сделано: `VirtualCameraSink` отделяет вывод от рендера.
16. Сделано: `MemorySink` даёт тестируемую sink-границу.
17. Сделано: `PipelineEngine` собирает source -> compose -> sink.
18. Сделано: `PpmSequenceSink` добавляет debug-output без платформенных API.
19. Сделано: `SyntheticFrameSource` даёт стабильные тестовые источники.
20. Сделано: CLI `status` показывает текущий backend.
21. Сделано: CLI `demo` пишет один PPM.
22. Сделано: standalone Rust example `pipeline_demo` пишет последовательность PPM вне production app runtime.
23. Сделано: CLI `check` печатает нормализованный config.
24. Сделано: тесты покрывают frame validation.
25. Сделано: тесты покрывают layout calculation.
26. Сделано: тесты покрывают composition.
27. Сделано: тесты покрывают PPM writer.
28. Сделано: тесты покрывают pipeline ticks.
29. Сделано: тесты покрывают failing source degradation.
30. Сделано: тесты покрывают capture error classification.
31. Сделано: тесты покрывают frame-spool round trip.
32. Сделано: deterministic property sweep проверяет bounds и non-overlap layout.
33. Сделано: добавлен libFuzzer target для `Frame::new_checked`.
34. Сделано: tiny P3 golden image фиксирует результат compositor.
35. Сделано: stable Rust benchmark измеряет full-HD compositor.
36. Сделано: `PipelineMetrics` измеряет render duration.
37. Сделано: `PipelineMetrics` измеряет суммарный source read duration.
38. Сделано: `PipelineMetrics` измеряет sink send duration.
39. Сделано: dropped source frames считаются отдельно от source errors.
40. Сделано: `PipelineMetrics` доступен в report, summary и engine.
41. Сделано: rough nearest scaling больше не единственный доступный режим.
42. Сделано: bilinear scaling доступен через API и `Smooth` control в app.
43. Сделано: nearest-neighbor сохранён default-режимом `Fast`.
44. Проверено: explicit SIMD отсутствует, но exact copy использует optimized slice copy.
45. Решение: unsafe SIMD отложен, пока profiling не покажет выигрыш против 0.27–4.42 ms paths.
46. Сделано: composition test покрывает 5 источников.
47. Сделано: composition test покрывает 7 источников.
48. Сделано: composition test покрывает 8 источников.
49. Сделано: generated layout properties покрывают odd counts до 64.
50. Сделано: tiny output dimensions покрыты layout и compositor regression tests.
51. Сделано: tiny layout не должен выходить за bounds.
52. Сделано: zero-sized frame отвергается.
53. Сделано: wrong buffer length отвергается.
54. Сделано: слишком большой buffer отвергается.
55. Сделано: грубый `MAX_FRAME_BYTES` заменён явным `FrameLimits` policy.
56. Сделано: default limit равен 1/16 physical RAM в границах 64 MiB–1 GiB.
57. Сделано: `CameraManError` разделяет user message и diagnostic chain.
58. Сделано: UI использует `user_message`, CLI/logging сохраняют полный `Display`.
59. Сделано: добавлен стабильный machine-readable `ErrorCode`.
60. Сделано: `.context(...)` и `Error::source` дают chaining без внешнего crate.
61. Ограничение: часть строковых ошибок nokhwa всё ещё требует эвристической классификации.
62. Сделано частично: typed nokhwa unsupported variants мапятся без text matching.
63. Сделано: классификация capture errors покрыта тестами.
64. Ошибка исправлена: broad `already` мог перекрывать not-found cases.
65. Сделано: representative AVFoundation/nokhwa messages покрыты table test.
66. Сделано: core library entrypoints документированы rustdoc и `cargo doc` проходит.
67. Сделано: lifecycle contracts публичных source/sink traits описаны rustdoc.
68. Сделано: `examples/custom_source.rs` компилируется и запускается.
69. Сделано: `examples/custom_sink.rs` компилируется и запускается.
70. Решение: boxed sources оставлены для heterogeneous camera backends.
71. Проверено: typed homogeneous list не подходит multi-backend pipeline без усложнения API.
72. Ограничение: dynamic dispatch остаётся, но capture/render cost существенно выше одного vtable call.
73. Сделано: boxed API документирован как plugin-like extension boundary.
74. Решение: core `PipelineEngine` оставлен синхронным SDK/example harness и удалён из production CLI-маршрута; app runtime имеет один путь через `RenderWorker`.
75. Сделано: `MediaClock` владеет cadence, а отдельный `RenderWorker` владеет app compositor и virtual sink.
76. Сделано частично: worker имеет stop state и latest-job cancellation; in-flight compose не прерывается внутри строки.
77. Сделано: один replaceable pending slot ограничивает очередь render jobs.
78. Сделано: backpressure заменяет старый pending job самым новым.
79. Сделано: latest-job strategy описана и покрыта детерминированным тестом.
80. Сделано: `PipelineMetrics` считает capture-to-sink latency total/max/samples.
81. Сделано: повтор sequence считается отдельным `stale_source_frames` metric.
82. Сделано: `Frame` делит `Arc<Vec<u8>>` и отделяет буфер copy-on-write при записи.
83. Сделано: `FrameView<'a>` представляет borrowed pixels без копии.
84. Сделано: `FrameView` валидирует row stride и исключает padding из active row.
85. Решение: owned `Frame` остаётся tightly packed, padded input живёт в `FrameView`.
86. Сделано: tight BGRA и copy-on-write contracts описаны rustdoc.
87. Сделано: padded `FrameView` конвертируется в owned tightly packed `Frame`.
88. Документировано: `set_bgra` clips в release и asserts в debug.
89. Сделано: `try_set_bgra` возвращает structured out-of-bounds coordinates.
90. Сделано: resilient clipping API сохранён для renderer hot paths.
91. Сделано: PPM sequence хранит structured frame metadata.
92. Сделано: рядом с timestamped PPM пишется serde-backed `.json` sidecar.
93. Сделано: sequence filename содержит sequence и capture timestamp.
94. Сделано: `write_ppm` сравнивается с checked-in valid P6 file.
95. Сделано: tiny P6 golden test фиксирует header и RGB bytes.
96. Сделано: PPM writer использует buffering.
97. Сделано: `tests/cli_smoke.rs` запускает реальный compiled binary.
98. Сделано: status/check/help покрыты integration smoke tests.
99. Сделано: CI запускает `cargo run -- check`.
100. Сделано: CI запускает `cargo run -- status`.
101. Сделано: `.github/workflows/ci.yml` описывает macOS verification flow.
102. Сделано: GitHub Actions запускает fmt, strict clippy и all-target tests.
103. Сделано: macOS CI строит bundle и проверяет plist/codesign.
104. Ограничение: Linux CI не может проверить CoreMediaIO; release smoke остаётся macOS-only.
105. Сделано: Objective-C dependencies target-scoped, macOS `cfg` ограничен platform adapters.
106. Сделано: extension binary имеет non-mac fallback.
107. Проблема: docs раньше ссылались на `arhitecture.md`.
108. Сделано: создан правильный `architecture.md`.
109. Сделано: активные docs используют только `architecture.md`; typo остаётся лишь historical finding.
110. Проверено: `App/`, `Extension/`, `Shared/` не содержат legacy source files.
111. Сделано: legacy directories audited; empty/user-metadata directories не входят в tracked tree.
112. Ограничение: `.DS_Store` может появляться локально, но никогда не входит в tracked tree.
113. Сделано: `.DS_Store` глобально игнорируется и проверяется hygiene test.
114. Проверено: `.idea` содержит только user-local IDE metadata.
115. Решение: `.idea` остаётся ignored и не хранится в repository.
116. Сделано: `target/` ignored и запрещён hygiene test.
117. Сделано: generated build output не должен попадать в commit.
118. Сделано: `signing/` игнорируется как private/generated boundary.
119. Сделано: private key/cert/profile extensions игнорируются в любом каталоге и проверяются test.
120. Сделано: signing identity/profile env vars документированы в README и CLI diagnostics.
121. Сделано: UID виртуального устройства больше не продублирован в extension process.
122. Сделано: device/stream names and UIDs вынесены в public constants `config.rs`.
123. Сделано: `EXTENSION_BUNDLE_ID` имеет один источник истины в library.
124. Сделано: app and extension bundle ids each have a single source of truth.
125. Исправлено: Info.plist больше не создаётся вручную собранной XML-строкой.
126. Сделано: четыре plist строятся как typed `plist::Value` dictionaries.
127. Сделано: generated plist round-trip покрыт tests, а release files проходят `plutil` в CI.
128. Исправлено: signing helper больше не скрывает release failure.
129. Сделано: release signing failure возвращается как error и останавливает bundling.
130. Сделано: только debug ad-hoc signing может продолжить с явным warning.
131. Исправлено: `codesign` failure больше не сводится к безымянному сообщению.
132. Сделано: error содержит exact command context, exit status и stderr.
133. Проверено: release/debug bundle mode выбирается одним `is_release_build()`.
134. Сделано: release bundle embeds release extension binary.
135. Сделано: debug/release binary path и embedded bundle path закреплены regression test.
136. Решение: hand-written CLI остаётся допустимым при текущих десяти простых commands.
137. Улучшение отложено: parser dependency добавлять при появлении options/subcommands beyond help.
138. Исправлено: signing env vars теперь видны в `bundle --help`.
139. Сделано: `cargo run -- bundle --help` печатает usage и release checklist.
140. Исправлено: CLI теперь имеет стабильную version command.
141. Сделано: `cargo run -- version` возвращает Cargo package version.
142. Исправлено: добавлена отдельная диагностика system extension.
143. Сделано: `cargo run -- diagnose-extension` работает даже при missing artifacts.
144. Сделано: diagnose печатает expected и embedded app entitlements.
145. Сделано: diagnose печатает expected и embedded extension entitlements.
146. Сделано: diagnose включает результат `systemextensionsctl list`.
147. Сделано: missing/invalid/valid provisioning profile различаются явно.
148. Исправлено: release instructions доступны как executable CLI checklist.
149. Сделано: checklist запускает bundle, `plutil`, strict `codesign` и diagnostics.
150. Сделано: README содержит recovery flow для `No matching profile found`.
151. Проблема: README earlier claimed remaining bridge work after bridge existed.
152. Сделано: README rewritten to current truth.
153. Проблема: old recommendations exceeded 500 numbered items.
154. Сделано на первом цикле: recommendation list был normalized to exactly 500 items.
155. Сделано на первом цикле: detailed review findings оставались ненумерованными, сохраняя тогда ровно 500 items.
156. Риск: docs могут drift, поэтому release checklist требует синхронного обновления трёх файлов.
157. Сделано: `CONTRIBUTING.md` содержит executable documentation/release checklist.
158. Сделано: фактический test count обновляется после каждого review pass.
159. Исправлено: добавлен `CHANGELOG.md` с текущим unreleased milestone.
160. Сделано: changelog фиксирует additions, behavioral changes и known limitations.
161. Ограничение: open-source license пока не выбрана владельцем.
162. Решение: не назначать лицензию автоматически; required owner decision явно документирован.
163. Исправлено: добавлен короткий contribution guide.
164. Сделано: dev workflow включает module boundaries, tests, docs и fuzz checks.
165. Решение: отдельный diagram asset не нужен, пока схема живёт рядом с архитектурой.
166. Сделано: точные text flows сохранены для plain Markdown readers.
167. Сделано: GitHub-compatible Mermaid overview добавлен в `architecture.md`.

## Iteration 2: App, Capture, Product Design

168. Сделано: app built with `eframe`/`egui`.
169. Сделано: app uses dense utility layout.
170. Сделано: app avoids landing-page structure.
171. Сделано: preview is first-class UI surface.
172. Сделано: source controls live in left panel.
173. Сделано: status bar reports live state.
174. Сделано: synthetic mode uses checkboxes.
175. Сделано: real mode now supports multiple checkboxes.
176. Сделано: layout mode supports grid/row/column.
177. Сделано: output toggle controls the selected RAM/file transport sink.
178. Сделано: install-extension button requests activation.
179. Сделано: activation status is shown in the UI.
180. Сделано: PPM export is available.
181. Сделано: FPS presets and Auto mode exist.
182. Сделано: active fps передаётся worker compositor и transport header.
183. Сделано: render cadence follows active fps.
184. Сделано: virtual sink stores target fps in transport header.
185. Сделано: extension can adapt cadence from transport fps.
186. Сделано: install button disabled when running bundle/location/entitlement preflight fails.
187. Сделано: preflight parses the signed app entitlement plist before enabling install.
188. Сделано частично: UI shows whether system-extension signing is usable, but not the Team ID yet.
189. Сделано: UI различает missing, valid и invalid provisioning profile.
190. Сделано: signing state links directly to matching troubleshooting section.
191. Исправлено: bounded app preferences persist through eframe storage.
192. Сделано: schema v4 сохраняет единый ordered list типизированных source descriptors; legacy input mode и отдельные id-list мигрируются последовательно.
193. Сделано: selected layout and scaling filter persist.
194. Сделано: Auto/fixed fps mode persists with 1–240 validation.
195. Сделано: export path editable and persists; empty stored path is repaired.
196. Сделано: virtual output toggle persists without serializing transport runtime.
197. Исправлено: left panel width is no longer fixed.
198. Сделано: native egui left `Panel` is resizable from 260 to 420 px.
199. Исправлено: resolved transport endpoint is actionable, not text-only.
200. Сделано: Copy action sends the RAM/file endpoint to clipboard.
201. Исправлено: camera errors no longer overwrite transient confirmations.
202. Сделано: sticky error, transient notice and derived state are separate.
203. Исправлено: switching Synthetic/Real no longer stops without feedback.
204. Сделано: mode switch posts an explicit stopped-preview confirmation.
205. Сделано: mode switch clears preview and advances the render epoch.
206. Сделано: stale in-flight results cannot overwrite a newer UI state.
207. Сделано: measured fps больше не считает scheduling ticks.
208. Сделано: fps считает только успешно завершённые live compositions.
209. Исправлено: stale previous frame is visibly dimmed while camera warms up.
210. Сделано: waiting overlay appears only during an active real preview.
211. Исправлено: camera discovery has an in-panel progress spinner.
212. Сделано: `CameraDiscoveryWorker` enumerates devices off the egui thread.
213. Исправлено: Refresh cannot block the UI or start a second concurrent request.
214. Сделано: refresh result is polled at a bounded 50 ms UI interval.
215. Исправлено: permission denial now identifies the exact macOS settings pane.
216. Сделано: recovery text points to Privacy & Security > Camera.
217. Исправлено: backend diagnostic strings stay out of the main UI message.
218. Сделано: stable `user_message` routes common capture failures to actions.
219. Сделано: capture errors have typed `CaptureErrorKind`.
220. Сделано: real-camera errors are prefixed with the source name.
221. Смягчено: sticky bar remains summary-only, while every source has independent health.
222. Сделано: real source rows show idle/waiting/healthy/failing color indicators.
223. Исправлено: source selection order is visible beside every selected source.
224. Сделано: `#1`, `#2`, ... match the composition cell order.
225. Решение: drag reorder отложен; явные controls дают доступный deterministic порядок.
226. Сделано: selected synthetic/real sources имеют bounded up/down controls.
227. Исправлено: первый выбранный source становится primary в PiP.
228. Сделано: `CompositionLayout::PictureInPicture` доступен в core и UI.
229. Сделано: primary занимает output, остальные sources идут stacked overlays.
230. Сделано: equal Grid уже покрыт regression tests для 5/7/8 sources.
231. Проблема: no labels on rendered cells.
232. Улучшение: optional source labels overlay.
233. Проблема: no crop/fit mode controls.
234. Улучшение: add fit/fill/crop choice.
235. Сделано: source можно отключить независимо во время running preview.
236. Сделано: uncheck не останавливает остальные sources.
237. Сделано: unchecked real source is released.
238. Исправлено: Stop -> immediate Start больше не opens same id concurrently.
239. Сделано: process-local camera lease serves as asynchronous release acknowledgement.
240. Сделано: replacement worker waits off-thread until old worker drops its lease.
241. Проблема: capture timeout helper can leave detached work.
242. Улучшение: design cancellable capture open.
243. Проблема: nokhwa cancellation hooks may be limited.
244. Улучшение: document backend limitation clearly.
245. Проблема: Continuity Camera behavior may differ from built-in camera.
246. Улучшение: add device-specific diagnostics.
247. Исправлено: capture больше не принимает backend default или абсолютный max-FPS; target-aware negotiation проверяет output geometry/rate для каждого декодируемого формата.
248. Улучшение: позже показать пользователю только подтверждённые hardware presets и понятную ошибку renegotiation; текущий runtime автоматически следует output contract.
249. Проблема: `SOURCE_WIDTH`/`SOURCE_HEIGHT` fixed for synthetic.
250. Улучшение: allow synthetic test resolution presets.
251. Сделано: app preview больше не загружает full 1080p texture.
252. Улучшение: measure texture upload cost.
253. Сделано: preview capped at 960x540/30 fps while output remains 1080p.
254. Сделано: default transport no longer writes a full 1080p frame to disk.
255. Сделано частично: dev uses POSIX shm, install uses App Group mmap; IOSurface remains zero-copy target.
256. Сделано: three slot states provide reader/writer backpressure without torn frames.
257. Сделано частично: writer PID detects process death; frame-age timeout is still needed.
258. Сделано: transport includes sequence and timestamp.
259. Сделано: transport version is explicit.
260. Улучшение: add CRC or checksum for debug validation.
261. Проблема: malformed file-fallback spool currently becomes None.
262. Улучшение: count malformed spool events.
263. Проблема: fallback file path still depends on the OS temp dir.
264. Сделано: UI and `status` show the selected transport/endpoint.
265. Проблема: explicit file fallback does not clean a stale spool on exit.
266. Улучшение: optionally remove the fallback spool on disconnect.
267. Проблема: removing a fallback spool could blank the extension unexpectedly.
268. Сделано: extension retains the latest valid frame between transport polls.
269. Сделано частично: direct Glow framebuffer screenshots есть, но ещё не запускаются в CI.
270. Сделано: screenshot smoke проверен без macOS Screen Recording permission.
271. Решение: mobile viewport не применим к native macOS utility.
272. Сделано: minimum 920x560 проверен framebuffer screenshot.
273. Исправлено: dynamic real-device labels truncate before fixed health/order controls.
274. Сделано: min-width audit uses full-name hover and framebuffer screenshots.
275. Проблема: buttons use text where icons may help.
276. Улучшение: add icons if egui icon source is chosen.
277. Проблема: color palette is serviceable but plain.
278. Улучшение: refine contrast and semantic states.
279. Смягчено: warning/error дополняются текстом, status dot и source-health shape.
280. Сделано: semantic state никогда не передаётся только цветом.
281. Проблема: no dark/light theme switch.
282. Улучшение: keep dark-only until product stabilizes.
283. Проблема: no keyboard shortcuts list.
284. Улучшение: show shortcuts via tooltips only.
285. Сделано: Space toggles start/stop when keyboard is free.
286. Исправлено: Space shortcut не срабатывает, пока любой widget focused.
287. Сделано: shortcut требует both no keyboard capture and no focused widget.
288. Проблема: no menu bar.
289. Улучшение: add menu only when commands grow.
290. Проблема: no crash reporting.
291. Улучшение: log to file in app support directory.
292. Проблема: no structured tracing.
293. Улучшение: add `tracing` later if needed.
294. Исправлено: UI формирует copyable nonblocking diagnostics summary.
295. Сделано: `Copy diagnostics` включает launch/signing/profile/activation/transport state.
296. Сделано частично: signing troubleshooting открывается напрямую; общего Help view нет.
297. Улучшение: add Help menu after signing flow is stable.
298. Исправлено: bare executable, development bundle и Applications bundle различаются.
299. Сделано: launch context виден в System Extension section и copy report.
300. Проверено: activation location соответствует Apple Applications-directory policy.
301. Сделано: non-bundle/development location disables activation with exact reason.
302. Проблема: `/Applications` install is manual.
303. Улучшение: add `cargo run -- install-app` later.
304. Проблема: overwriting `/Applications/CameraMan.app` can be destructive.
305. Улучшение: backup existing install before replace.
306. Исправлено: UI показывает actual code-signing Team ID или unavailable.
307. Сделано: CLI prints app/extension/profile Team IDs and pairwise match state.
308. Сделано: provisioning validator also extracts explicit Team ID structurally.
309. Сделано: entitlement, bundle id and signing/profile Team ID проверяются before embed.
310. Проблема: Xcode-managed signing is outside current CLI.
311. Улучшение: document manual signing path.
312. Сделано: Install cannot be clicked repeatedly while activation is in progress.
313. Сделано: activation button is disabled while Requesting or awaiting approval.
314. Сделано: delegate/request are retained while installer lives.
315. Сделано: repeated activation no longer replaces the active request.
316. Сделано: `ExtensionInstaller` rejects duplicate activation requests by state.
317. Проблема: delegate queue is main dispatch queue.
318. Улучшение: evaluate serial queue for CLI diagnose/install.
319. Проблема: activation status is not persisted.
320. Сделано для diagnostics: `diagnose-extension` queries `systemextensionsctl list`.
321. Проблема: `systemextensionsctl` output is not parsed.
322. Решение: raw output остаётся diagnostics-only до стабильного parser contract.
323. Проблема: no uninstall helper.
324. Улучшение: document `systemextensionsctl uninstall`.
325. Исправлено частично: uninstall всё ещё manual, но required Team ID теперь виден.
326. Сделано: Team ID печатается после inspection and in diagnostics.
327. Ограничение: real install cannot be fully automated without user approval.
328. Сделано: README/UI явно отделяют build readiness от macOS approval.
329. Сделано: README now states provisioning requirement.
330. Исправлено: ad-hoc/bare/development bundle cannot request activation.
331. Сделано: capability preflight проверяет location and signed entitlement.
332. Проблема: no smoke test for activation failure UI.
333. Улучшение: add test seam around `ExtensionInstaller`.
334. Проблема: egui app state is monolithic.
335. Улучшение: split controls, preview, and status widgets.
336. Проблема: splitting too early can obscure learning path.
337. Улучшение: split after the current feature set stabilizes.
338. Проблема: `CameraManApp` owns too many concerns.
339. Улучшение: extract `RealSourceManager`.
340. Улучшение: extract `VirtualOutputController`.

## Iteration 3: Extension, Packaging, Review, Release

341. Сделано: Rust CoreMediaIO provider source exists.
342. Сделано: Rust CoreMediaIO device source exists.
343. Сделано: Rust CoreMediaIO stream source exists.
344. Сделано: stream declares 1920x1080 BGRA format.
345. Сделано: stream sends `CMSampleBuffer` frames.
346. Сделано: placeholder fallback keeps stream alive.
347. Сделано: extension reads shared memory by default and file spool only on request.
348. Сделано: extension can scale incoming frame to output size.
349. Сделано: extension uses host-time timestamps.
350. Сделано: extension has a run loop.
351. Сделано: video format description is cached.
352. Сделано: creation failure is logged instead of hidden panic.
353. Проблема: extension FFI code is unsafe-heavy.
354. Улучшение: isolate more unsafe blocks behind small wrappers.
355. Сделано: objc2 0.6.4 generates `extern "C-unwind"` methods, so callback panics unwound into CoreMediaIO instead of aborting; every callback body now runs inside `contain_panic`.
356. Сделано: callback bodies are wrapped in `catch_unwind` behind `panic_boundary::contain_panic`, which logs the callback name and returns a conservative default (deny, empty collection, refused start) instead of unwinding; the fallback runs contained too and aborts rather than resuming the unwind, because no return value can be fabricated for the platform.
357. Проблема: stream handle stores raw pointer-like value.
358. Улучшение: replace with retained object managed safely across thread.
359. Сделано: client authorization is enforced by a pure signing-identity policy consulted by connect and stream-start callbacks.
360. Сделано: CMIO signingID and pid are read; clients without an establishable signing identity are denied and every decision is logged.
361. Сделано: client id logging exists.
362. Проблема: client id is not security identity.
363. Улучшение: document that logging is not authorization.
364. Проблема: Core Foundation Create Rule is easy to violate.
365. Улучшение: audit every `from_raw`.
366. Проблема: extension error reporting is mostly `eprintln!`.
367. Улучшение: add structured extension logging.
368. Проблема: extension has no health endpoint.
369. Сделано частично: transport exposes writer PID liveness; frame age remains to add.
370. Сделано: default reader maps a shared three-slot region instead of reading a whole frame file per tick.
371. Сделано: POSIX shm for bare dev and App Group mmap for signed bundles share one protocol.
372. Сделано: default path removes hundreds of MB/s of per-frame disk I/O.
373. Сделано: shared App Group is extracted, validated, signed, and used for production IPC.
374. Сделано частично: shared memory is implemented; IOSurface remains for zero-copy output.
375. Проблема: transport schema is custom.
376. Улучшение: keep header tiny and versioned.
377. Сделано: header has magic and version.
378. Проблема: no backward compatibility for v1/v2 transport.
379. Улучшение: parse old version for smoother dev upgrades.
380. Сделано: extension reports transport errors before using its last frame/placeholder.
381. Сделано: duplicate transport errors are suppressed until the message changes.
382. Проблема: fallback placeholder may hide app failure.
383. Улучшение: encode "source is fallback" visually in placeholder.
384. Проблема: virtual output can appear alive while app is dead.
385. Улучшение: include stale-frame age in extension logic.
386. Исправлено: extension нормализует transport fps в объявленный CMIO range.
387. Сделано: transport now carries fps.
388. Улучшение: extension should smooth fps changes.
389. Исправлено: dimensions/pixel format fixed, а frame-duration contract явно допускает 15–60 fps.
390. Сделано: cadence меняется внутри объявленного range без смены pixel format.
391. Проблема: no integration test with real CoreMediaIO client.
392. Улучшение: test with OBS/QuickTime/FaceTime after signing.
393. Проблема: system extension is not listed without valid install.
394. Улучшение: add release checklist for System Settings approval.
395. Проблема: ad-hoc signing cannot install extension.
396. Сделано: docs now state that clearly.
397. Проблема: Apple Development cert without provisioning profile gets killed.
398. Сделано: this was verified through AMFI `No matching profile found`.
399. Сделано: bundler embeds separate host and extension profiles before signing.
400. Сделано: bundler leaves restricted entitlement out when no profile is provided.
401. Сделано: bundler no longer creates an unlaunchable entitlement-bearing app from identity alone.
402. Сделано: host and `CAMERAMAN_EXTENSION_PROVISIONING_PROFILE` env inputs supported.
403. Сделано: verify profile contains bundle id.
404. Сделано: verify profile contains system-extension entitlement.
405. Сделано: actual app/extension signature Team IDs must match each other and both profiles.
406. Сделано: extension grants sandbox and both signed targets carry one validated App Group.
407. Проверено по Apple CMIO guidance: host/extension need matching App Group for IPC.
408. Исправлено: signed-build Mach service is prefixed by the selected App Group.
409. Сделано: Mach service is generated from validated profile metadata, not guessed Team/App ID prefixes.
410. Сделано: common App Group config is embedded into both plist and entitlement sets.
411. Сделано: App Group is added only for the installable two-profile path.
412. Проблема: release notarization not implemented.
413. Улучшение: add notarization script after signing stabilizes.
414. Проблема: no hardened runtime config.
415. Улучшение: add hardened runtime for release.
416. Проблема: no versioned release artifact naming.
417. Улучшение: output `CameraMan-<version>.zip` later.
418. Проблема: no app icon.
419. Улучшение: add `.icns` before user-facing release.
420. Проблема: Info.plist lacks document/help metadata.
421. Улучшение: keep plist minimal until release.
422. Проблема: `NSCameraUsageDescription` may be too technical.
423. Улучшение: rewrite permission string in user language.
424. Проблема: app category is generic video.
425. Улучшение: revisit category for distribution.
426. Проблема: no localization.
427. Улучшение: keep English-only until workflows stabilize.
428. Проблема: no Russian UI despite Russian project conversation.
429. Улучшение: consider localization after core is stable.
430. Проблема: no install guide screenshots.
431. Улучшение: add screenshots after real signing works.
432. Проблема: no code review checklist in repo.
433. Улучшение: add review checklist to architecture.
434. Проблема: current docs are long.
435. Улучшение: keep README short and architecture detailed.
436. Сделано: README is now short.
437. Сделано: architecture now holds deeper decomposition.
438. Сделано на третьем проходе: recommendations были нормализованы до 500 items.
439. Проблема: old `arhitecture.md` typo could confuse users.
440. Сделано: canonical file renamed to `architecture.md`.
441. Проблема: deleting old file can break external links.
442. Улучшение: mention rename in commit message.
443. Проблема: untracked `src/system_extension.rs` was easy to miss.
444. Улучшение: stage new files explicitly before commit.
445. Проблема: `git diff --stat` hides untracked files.
446. Улучшение: always run `git status --short` before final.
447. Проблема: dirty main branch increases merge risk.
448. Улучшение: use feature branch for future large tasks.
449. Проблема: user requested merge/push from main.
450. Улучшение: commit on main only after tests pass.
451. Проблема: pushing main directly is risky.
452. Улучшение: future work should open PR.
453. Проблема: no branch protection knowledge available.
454. Улучшение: inspect GitHub settings before release workflow.
455. Проблема: no remote changes were pulled this run.
456. Сделано: `git fetch origin` confirmed main equals origin/main.
457. Проблема: old docs said one-camera real mode.
458. Сделано: README now says multi-camera real mode.
459. Проблема: old docs said frame bridge remained future work.
460. Сделано: README now says frame bridge exists.
461. Проблема: recommendation backlog mixed done/open states beyond 500.
462. Сделано: backlog normalized and reviewed.
463. Проблема: tests do not cover app UI.
464. Улучшение: add UI state unit tests where possible.
465. Проблема: extension binary tests are zero.
466. Улучшение: move pure extension helpers into testable module.
467. Проблема: direct CoreMediaIO tests need macOS and signing.
468. Улучшение: gate integration tests behind env var.
469. Проблема: `capture-demo` assumes camera id `0`.
470. Улучшение: allow camera id argument.
471. Проблема: `list-cameras` output lacks JSON mode.
472. Улучшение: add `--json` for tooling.
473. Проблема: CLI output and README can drift.
474. Улучшение: generate command docs from constants later.
475. Сделано: the panic policy is documented in `docs/unsafe-invariants.md`: no unwind across the platform boundary, contain-log-degrade, and the per-callback defaults.
476. Сделано: the no-panic-across-FFI rule is enforced, not only written down: `every_objc_callback_body_contains_its_panics` scans every source under `src/` (CoreMediaIO callbacks and the SystemExtensions delegate alike) and, with `panic_containment_requires_unwinding_profiles`, fails if a callback is left uncontained or a profile sets `panic = "abort"`.
477. Проблема: no unsafe audit comments for every block.
478. Улучшение: add focused safety comments in extension code.
479. Проблема: too many manual Objective-C method signatures.
480. Улучшение: wrap each protocol implementation in smaller module.
481. Сделано: app and extension share frames through RAM by default, not the filesystem.
482. Сделано: `FrameTransportSink`/`Reader` isolate RAM and file implementations.
483. Проблема: extension cannot tell user-facing app status.
484. Улучшение: add reverse status channel later.
485. Проблема: no source-level latency display.
486. Улучшение: show per-source freshness.
487. Проблема: app cannot save/restore camera choices robustly.
488. Улучшение: persist by stable device id and name fallback.
489. Проблема: no handling for camera unplug mid-stream beyond source error.
490. Улучшение: show unplugged source tile.
491. Проблема: no release smoke script.
492. Улучшение: add script for fmt/clippy/test/bundle/codesign.
493. Проблема: no final visual verification in this run yet.
494. Улучшение: run app screenshot check before release.
495. Проблема: no generated architecture diagram.
496. Улучшение: add Mermaid diagram if docs renderer supports it.
497. Проблема: no owner notes for risky files.
498. Улучшение: add comments in architecture for `extension_main.rs`.
499. Проблема: production virtual camera is blocked by signing/provisioning.
500. Сделано: validate provisioning profile contents and disable impossible install states.

## Iteration 4: Research-Backed Hardening

501. P0: заменить `CVPixelBufferCreate` на ограниченный `CVPixelBufferPool`, созданный один раз на формат потока.
502. P0: прикреплять к каждому выходному `CVPixelBuffer` явные SDR color tags и проверить их распространение в реальном consumer app.
503. P0: передавать capture timestamp между процессами в общей monotonic host-time шкале, а wall clock оставить только для журналов.
504. P0: заменить `work + sleep(period)` в extension на deadline-based pacing, чтобы время обработки не добавлялось к каждому frame interval.
505. P0: ввести stale-frame timeout; после остановки producer extension не должен бесконечно показывать последний живой кадр.
506. P0: передавать generation, sequence и discontinuity reason, а при reconnect/format change отправлять корректный CMIO discontinuity flag.
507. P0: добавить случайный writer generation/nonce в mmap header и сбрасывать reader cache при смене generation.
508. P0: дополнить `writer_pid` start token или nonce, чтобы повторное использование PID не оживляло устаревшего владельца.
509. P0: проверить и зафиксировать поведение нескольких одновременных CMIO clients, включая независимые start/stop и disconnect.
510. P0: описать extension lifecycle конечным автоматом и возвращать NSError при невозможном переходе вместо безусловного `true`.
511. P1: завести измеряемый copy ledger для capture decode, compose, mmap publish/read, CVPixelBuffer fill и preview upload.
512. P1: вернуть из mmap reader RAII borrowed slot view и копировать из него прямо в output buffer, исключив промежуточный owned `Frame`.
513. P1: добавить `compose_into` для sink-owned writable frame/slot и убрать копирование готового `Frame` в mmap.
514. P1: повторно использовать CPU frame buffers по ключу format вместо выделения full-HD `Vec` на каждом composition tick.
515. P1: кешировать x/y sample maps для одинаковых source/destination dimensions и scaling filter.
516. P1: сравнить текущий scaler с pure-Rust `fast_image_resize`; подключать SIMD adapter только при измеримом выигрыше на ARM64 и x86_64.
517. P2: распараллеливать независимые cells/rows через Rayon только выше профилированного порога, сохраняя последовательный путь для малых кадров.
518. P2: сделать экспериментальный pure-Rust `wgpu` compositor с теми же golden contracts и обязательным CPU fallback.
519. P2: прототипировать IOSurface/CVMetalTextureCache через Rust `objc2` bindings, не добавляя Swift, и измерить реальное сокращение копий.
520. P1: перенести preview downscale/BGRA-to-RGBA из UI thread в render worker и повторно использовать preview buffer.
521. P0: добавить тип `Colorimetry` с primaries, transfer, matrix/range и alpha mode вместо неявного значения `BGRA8`.
522. P1: определить canonical internal color contract и правила преобразования source color metadata на входе compositor.
523. P1: сравнить обычный bilinear и linear-light resize на SSIM/VMAF и не смешивать gamma-encoded RGB без осознанного решения.
524. P0: определить straight или premultiplied alpha; resize/blend должны умножать и делить alpha согласованно.
525. P1: включить pixel aspect ratio и clean aperture в frame/output contract до поддержки нестандартных sources.
526. P1: хранить orientation/mirror как transform metadata и применять его один раз в compositor, а не менять bytes в capture backend.
527. P2: рекламировать несколько проверенных output formats только после реализации format negotiation; до этого явно сохранять один фиксированный contract.
528. P1: проводить format renegotiation атомарно через новый epoch, сбрасывая pools, caches и stale frames вместе.
529. P0: разделить media time, monotonic latency time и human wall time в API и diagnostics.
530. P1: дополнить exact pixel goldens perceptual quality gates на controlled fixtures; VMAF оставить offline test tool, не runtime dependency.
531. P1/SOLID: разделить `app.rs` на controller/state, capture coordination, extension readiness и отдельные `ui` surfaces.
532. P1/SOLID: разделить `main.rs` на `cli`, `bundle`, `signing`, `diagnostics` и typed plist modules.
533. P1/SOLID: разделить `extension_main.rs` на provider/device/stream objects, frame pump, timing и pixel-buffer adapter.
534. P1/SOLID: разделить `shared_memory_transport.rs` на protocol layout, mapped backend, writer, reader и platform helpers.
535. P1: зафиксировать dependency rule: domain не импортирует UI/CMIO/CLI, adapters зависят от domain contracts, composition не знает о transport.
536. P1: заменить прямые мутации UI workflow на небольшой `AppCommand`/`AppEvent` boundary с детерминированными reducer tests.
537. P1: ввести узкий `Clock` trait для pacing, stale detection и latency tests без реального сна и wall-clock jumps.
538. P1: добавить schema version и последовательные migrations для persisted preferences и scene files.
539. P1: отделить wire DTO mmap/JSON/plist от domain types, чтобы versioning transport не менял `Frame` API.
540. P1: записывать короткие ADR для color contract, clock, IPC ownership, buffer pool и CPU/GPU решения.
541. P1: добавить `tracing` spans `capture`, `compose`, `publish`, `consume`, `pixel_buffer`, `send_sample` со стабильными fields.
542. P1: продублировать ключевые interval spans нативными macOS signposts для анализа в Instruments.
543. P1: хранить bounded latency histograms и показывать p50/p95/p99/max, а не только sum/max.
544. P1: разделить drop counters на source missing, late, queue replacement, all-slots-busy, stale, pool exhausted и discontinuity.
545. P1: считать full-frame copies и allocations per output frame; оптимизация принимается только если эти числа уменьшаются.
546. P1: держать локальный bounded diagnostic event ring без telemetry и персональных данных.
547. P1: экспортировать versioned JSON diagnostics с build hash, platform, formats, counters и redacted paths.
548. P1: расширить benchmark matrix на 720p/1080p, 1/2/4/8 sources, fit/PiP, nearest/bilinear, ARM64/x86_64.
549. P1: задать hardware-labeled performance budgets и проверять regression относительно сохранённого baseline, а не универсального числа.
550. P1: добавить 8-hour soak profile с RSS high-water, allocations, dropped frames, wakeups и energy impact.

### Статус пакета 501-550

| Пункт | Статус | Проверяемый результат |
|---:|---|---|
| 501 | Готово | Один ограниченный `OutputPixelBufferPool` живёт весь срок stream worker. |
| 502 | Код готов, внешний тест ожидается | Rec.709 primaries/transfer/matrix прикрепляются с `ShouldPropagate`; реальный consumer требует signed install. |
| 503 | Готово | mmap v4 несёт monotonic capture time; wall time используется только в diagnostics. |
| 504 | Готово | `DeadlinePacer` считает абсолютный следующий deadline и пропущенные интервалы. |
| 505 | Готово | Frame pump прекращает replay после bounded stale timeout. |
| 506 | Готово | Generation/sequence классифицируются в restart/drop/format/timing/stale причины и CMIO flags. |
| 507 | Готово | Случайный generation сбрасывает reader sequence cache. |
| 508 | Готово | PID проверяется вместе с generation/start token, поэтому reuse не оживляет writer. |
| 509 | Код и deterministic tests готовы | Client-id lifecycle учитывает перекрывающиеся start/stop; live CMIO smoke требует signed install. |
| 510 | Готово | `StreamLifecycle` отклоняет невозможные переходы, bridge возвращает `NSError`. |
| 511 | Готово | Copy ledger охватывает capture, composition, mmap, spool, Core Video и preview. |
| 512 | Готово | `BorrowedTransportFrame` удерживает `READING` slot через RAII без owned materialization. |
| 513 | Частично | `compose_into` и reuse готовы; один publish copy из CPU output в mmap пока остаётся и явно измеряется. |
| 514 | Готово | Render worker возвращает до трёх CPU output buffers в bounded recycle pool. |
| 515 | Готово | Nearest/bilinear x/y maps кешируются по source/destination/filter. |
| 516 | Решение принято | ARM64 и x86_64: adapter быстрее, но меняет все 518400 pixels; подключение отклонено. |
| 517 | Готово как opt-in | Rayon включается feature-флагом только выше `PARALLEL_RESIZE_PIXEL_THRESHOLD`. |
| 518 | Готово как experiment | Pure-Rust wgpu compute path имеет pixel parity test и обязательный CPU fallback. |
| 519 | Прототип готов | Rust `CVMetalTextureCache` bridge существует; production copy reduction не доказан, поэтому путь не включён. |
| 520 | Готово | Preview resize/BGRA->RGBA выполняется worker’ом с bounded reusable buffers. |
| 521 | Готово | `Colorimetry` типизирует primaries/transfer/matrix/range/alpha. |
| 522 | Готово | Canonical BGRA/Rec.709/full/opaque contract нормализуется compositor’ом. |
| 523 | Готово | Linear-light fixture сравнивается SSIM/PSNR; VMAF вынесен в offline ffmpeg tool. |
| 524 | Готово | Straight/premultiplied/opaque alpha обрабатываются согласованно до opaque output. |
| 525 | Готово | Pixel aspect ratio и clean aperture входят в `FrameContract`. |
| 526 | Готово | Rotation/mirror metadata применяется один раз при composition. |
| 527 | Готово как ограничение | CMIO рекламирует один проверенный BGRA output format. |
| 528 | Готово | `FormatEpochCoordinator` публикует epoch только после успешного resource reset. |
| 529 | Готово | Media, monotonic и wall timestamps являются разными Rust newtypes. |
| 530 | Готово | Exact goldens дополнены MSE/PSNR/SSIM gate и offline VMAF runner. |
| 531 | Готово | `app.rs` разделён на controller, capture coordination, readiness и UI. |
| 532 | Готово | `main.rs` разделён на cli, bundle, signing, diagnostics и typed plist. |
| 533 | Готово | Extension разделён на objects, frame pump, timing и pixel-buffer adapter. |
| 534 | Готово | Shared transport разделён на protocol, mapped backend, writer и reader. |
| 535 | Готово | `tests/architecture_boundaries.rs` запрещает обратные domain/compositor зависимости. |
| 536 | Готово | `AppCommand`/`AppEvent` reducer имеет deterministic transition tests. |
| 537 | Готово | Узкий `Clock` trait отделяет monotonic tests от реального времени. |
| 538 | Готово | Preferences и scenes имеют version и последовательные migrations. |
| 539 | Готово | `wire.rs` отделяет transport codes/DTO от domain enums. |
| 540 | Готово | ADR 0001-0005 фиксируют color, clocks, IPC, pools и CPU/GPU решения. |
| 541 | Готово | Стабильные spans: capture/compose/publish/consume/pixel_buffer/send_sample. |
| 542 | Готово | Тот же RAII interval зеркалируется в нативный macOS signpost. |
| 543 | Готово | Bounded 2048-sample windows экспортируют p50/p95/p99/max. |
| 544 | Готово | Семь независимых drop counters соответствуют пункту без общего catch-all. |
| 545 | Готово | JSON нормализует copies, bytes и allocations на завершённый output frame. |
| 546 | Готово | Локальное кольцо хранит максимум 256 типизированных событий без telemetry/PII. |
| 547 | Готово | `cameraman-diagnostics` экспортирует schema/build/platform/formats/counters/redacted paths. |
| 548 | Готово | Все 32 cases прогнаны на aarch64 и x86_64/Rosetta. |
| 549 | Готово | Apple M4 Max baselines разделены по target arch; comparator допускает 15% regression. |
| 550 | Профиль готов, полный прогон ожидается | Default 8h mmap soak есть; 2s smoke дал 60 frames, 0 late и стабильный ledger. |

Оставшиеся не-внешние хвосты этого пакета намеренно видимы: пункт 513 требует
compositor output surface поверх mmap/IOSurface, а пункт 519 принимается только
после доказанного уменьшения copy ledger. Signed consumer checks из 502/509 и
полный восьмичасовой прогон 550 нельзя честно заменить unit test’ом.

551. P0: вынести atomic slot state machine из mmap и проверить допустимые interleavings через Loom.
552. P1: доказать Kani bounds для `mapped_len`, slot offsets, frame-size arithmetic и resize coordinate math.
553. P1: запускать Miri на pure-Rust unsafe helpers и protocol layout tests, исключив неподдерживаемые macOS FFI cases явно.
554. P0: добавить structure-aware fuzz target для mmap header/slot metadata/version и corpus повреждённых состояний.
555. P1: fuzz provisioning plist/CMS decoding inputs, App Group arrays, entitlement dictionaries и malformed lengths.
556. P1: расширить generated compositor properties на transforms, color/alpha, odd strides и format epochs.
557. P0: сделать настоящий child-process mmap round trip, чтобы тест проверял отдельные address spaces и App Group-style file lifecycle.
558. P0: тестировать kill writer во время publish, PID reuse simulation, restart generation, sleep/wake и clock discontinuity.
559. P1: добавить multi-reader/multi-client stress test с медленным consumer и доказательством отсутствия torn frames.
560. P2: периодически запускать `cargo-mutants` на protocol, provisioning и renderer math и сохранять только полезные surviving mutants.
561. P1/Design: сохранить preview главным полотном, а workflow читать слева направо как Sources -> Preview -> Output.
562. P1/Design: вынести редкий signing/installation checklist в отдельный setup sheet, освободив основной control panel от постоянной сложности.
563. P1/Design: в status bar показывать live state, p95 latency, drops и stale warning; подробности раскрывать tooltip/diagnostics.
564. P1/Design: оформить Start/Stop как одну стабильную primary control с `Play`/`Square` icon и неизменным размером.
565. P1/Accessibility: source health должен иметь icon и текстовое accessible name, а не кодироваться только цветной точкой.
566. P2/Design: показывать transform inspector только для выбранного source, без вложенных cards и повторяющихся controls в каждой строке.
567. P1/Accessibility: проверить полный keyboard traversal, visible focus, accessible labels/roles и отсутствие global shortcuts во вводе текста.
568. P1/Accessibility: выдерживать минимум 24x24 для частых pointer targets и предоставлять keyboard alternative для reorder/drag actions.
569. P1/Accessibility: проверить контраст, high-contrast mode и различимость warning/success/error при распространённых нарушениях color vision.
570. P2/Design: подготовить RU/EN localization, long-text fixtures, Retina 2x и minimum-window visual regression captures.
571. P2/Product: добавить named scenes, чтобы набор sources/layout/transforms/output settings переключался одной командой.
572. P2/Product: добавить per-source crop, fit/fill, mirror, rotate, opacity и position через одну typed transform model.
573. P1/Product: заменить мгновенное удаление проблемной камеры на bounded exponential reconnect с ручным Retry и понятным состоянием.
574. P1/Product: дать scene policy для missing/stale source: freeze briefly, placeholder, hide cell или stop output.
575. P1/Product: добавить Virtual Camera Self-Test, который публикует test pattern и проверяет, что extension действительно читает новые generations.
576. P2/Product: применять scene edits транзакционно и дать Undo/Redo для reorder и transform changes.
577. P2/Product: добавить import/export versioned scene JSON с validation preview и безопасной migration.
578. P1/Product: показывать negotiated source resolution/fps/color рядом со health, не перегружая source row.
579. P2/Product: предлагать только реально поддержанные 720p/1080p и 24/30/60 presets и объяснять renegotiation failures.
580. P1/Product: сделать однократный activation assistant с progress/recovery, оставив постоянные diagnostics доступными из стандартной команды меню.
581. P1/Security: добавить `cargo-deny` policy для advisories, duplicate versions, licenses, git sources и banned crates.
582. P1/Security: запускать RustSec audit в CI и release checklist с явным процессом временных исключений.
583. P1/Release: генерировать SPDX 3.0.1 SBOM для фактического locked dependency graph и прикладывать его к release artifact.
584. P1/Release: публиковать SLSA v1.2 build provenance из hosted CI и связывать её с SHA-256 подписанного app bundle/archive.
585. P1/Security: закрепить third-party GitHub Actions по immutable commit SHA и сузить workflow token permissions.
586. P0/Release: дополнить production pipeline notarization, stapling и `spctl --assess`, а не останавливаться на `codesign --verify`.
587. P1/Release: импортировать signing identity во временный CI keychain, удалить keychain после job и никогда не хранить profile/key в repository.
588. P1/Security: проверять release entitlement diff против минимального allowlist отдельно для host и extension.
589. P1/Maintenance: убрать future-incompatibility warning `block 0.1.6` обновлением/заменой macOS camera backend после совместимого upstream release.
590. P0/Governance: выбрать лицензию проекта и генерировать third-party notices до публичного распространения.
591. Решение: не добавлять Tokio только из-за популярности; текущим frame workers достаточно owned threads, bounded state и явной отмены.
592. Решение: не добавлять FFmpeg/GStreamer runtime dependency, пока нет утверждённой функции decode/encode, которую нельзя закрыть малым Rust adapter.
593. Решение: не заменять egui без usability evidence; сначала разделить app state/UI и измерить реальные ограничения.
594. Решение: не подключать общий zero-copy IPC framework; взять его proven ideas, но сохранить App Group/CMIO protocol узким и подконтрольным проекту.
595. Milestone A: сначала закрыть pixel-buffer pool, color tags, monotonic timing, pacing, stale/restart и multi-client correctness.
596. Milestone B: затем закрыть Loom/Kani/fuzz/subprocess tests, tracing, signposts и percentile metrics.
597. Milestone C: после доказательств убрать measured CPU copies, добавить reusable buffers/maps и сравнить SIMD/Rayon.
598. Milestone D: только затем оценивать wgpu/IOSurface path и расширенные scene/transform workflows.
599. Acceptance gate: на фиксированном hardware profile 1080p30/4-source run должен держать bounded queue, ноль torn frames, устойчивый RSS и задокументированные p95/p99 budgets.
600. Research rule: обновлять source snapshot перед крупным milestone, но принимать решения по коду, maintenance, лицензии и измерениям, а не по stars.

## Implementation Status: 551-600

Статусы ниже отделяют готовый код от проверок, которым нужны восемь часов,
платные Apple credentials или совместимый upstream release.

| Item | Status | Evidence or remaining gate |
|---:|---|---|
| 551 | Done | Atomic slot transitions factored and model-checked with Loom. |
| 552 | Done | Kani harnesses prove bounded metadata and slot-state invariants. |
| 553 | Done | Miri script exercises unsafe mapped-memory and borrowed-read paths. |
| 554 | Done | Structured protocol fuzz target and malformed-state corpus added. |
| 555 | Done | Provisioning/CMS/plist fuzz target covers malformed groups and lengths. |
| 556 | Done | Generated media-contract, transform, odd-stride and format-epoch properties added. |
| 557 | Done | Child-process file-backed mmap round trip covers separate address spaces and cleanup. |
| 558 | Done | Kill/restart generation, PID-start-token reuse, long-sleep and backward-clock cases are tested. |
| 559 | Done | Fast/slow multi-process readers verify complete, untorn frame bytes. |
| 560 | Done | Pinned mutation workflow targets protocol, provisioning and renderer math. |
| 561 | Done | Three-column Sources -> Preview -> Output hierarchy implemented. |
| 562 | Done | Signing, activation, self-test and diagnostics moved into Setup. |
| 563 | Done | Status bar exposes live state, p95, drops and stale state with details. |
| 564 | Done | Stable 112x36 Start/Stop primary control uses play/stop symbols. |
| 565 | Done | Source health combines shape, text, color and accessible descriptions. |
| 566 | Done | One selected-source transform inspector avoids repeated nested controls. |
| 567 | Done | AccessKit, keyboard traversal, visible focus and text-input shortcut suppression enabled. |
| 568 | Done | Frequent targets are at least 28x28 and reorder has keyboard commands. |
| 569 | Done | High-contrast mode and non-color status distinctions implemented. |
| 570 | Done | Bounded RU/EN control vocabulary plus workspace/Setup long-text, Retina and minimum-window fixtures run in CI; machine diagnostics stay stable English. |
| 571 | Done | Named, persisted scenes switch source/layout/transform/output state. |
| 572 | Done | Typed crop, fit/fill, mirror, rotate, opacity and position model implemented. |
| 573 | Done | Source identity survives bounded exponential reconnect; reset requires a real frame and manual Retry is visible. |
| 574 | Done | Placeholder, brief freeze, hide-cell and stop-output policies implemented. |
| 575 | Done | Protocol v4 consumer acknowledgement powers the virtual-camera self-test. |
| 576 | Done | Scene edits use bounded snapshots with Undo/Redo; an entire pointer drag is one transaction. |
| 577 | Done | Versioned JSON validate-preview/import/export and migration are implemented with 1 MiB reads and atomic replacement. |
| 578 | Done | Negotiated dimensions, fps and decoded format appear beside source health. |
| 579 | Done | UI exposes 24/30/60 and only the actually implemented 1080p output profile. |
| 580 | Done | Setup contains one activation assistant with progress and recovery states. |
| 581 | Done | cargo-deny enforces advisories, sources, licenses, bans and duplicate reporting. |
| 582 | Done with exceptions | RustSec runs in CI; exact temporary exceptions have owner, impact and expiry rules. |
| 583 | Done | Locked graph generates structurally validated SPDX 3.0.1 JSON-LD. |
| 584 | Automated, external run pending | Hosted release workflow attests the signed archive and SHA-256. |
| 585 | Done | Third-party Actions use immutable commit SHAs and minimal permissions. |
| 586 | Automated, external run pending | Workflow runs notarytool, stapler and Gatekeeper; credentials are not available locally. |
| 587 | Done | Signing material lives in RUNNER_TEMP and a temporary keychain removed in `always()`. |
| 588 | Done | Host and extension entitlements must exactly match separate minimal allowlists. |
| 589 | Upstream-dependent | Exact `block 0.1.6` guard remains until a compatible nokhwa macOS stack exists. |
| 590 | Done | Project is MIT OR Apache-2.0 and generated third-party notices are tracked. |
| 591 | Accepted | ADR keeps owned bounded threads and rejects an unnecessary Tokio runtime. |
| 592 | Accepted | ADR keeps FFmpeg/GStreamer out until a concrete codec/container requirement exists. |
| 593 | Accepted | egui retained; state and surfaces were split before considering replacement. |
| 594 | Accepted | Narrow App Group/CMIO protocol retained without a general IPC framework. |
| 595 | Done | Milestone A correctness, timing, stale/restart and multi-client work is complete. |
| 596 | Done | Milestone B model checking, fuzz/process tests and observability is complete. |
| 597 | Done | Reusable CPU buffers/maps, copy ledger and measured SIMD/Rayon decisions are complete. |
| 598 | Partial by design | Scene/transform and wgpu experiments exist; production IOSurface remains evidence-gated. |
| 599 | Gate ready, full run pending | Executable M4 Max 1080p30/four-source gate passes smoke; eight-hour RSS qualification remains. |
| 600 | Done | Dated research and refresh policy require code/license/measurement evidence over stars. |

## Additional Survey and Benchmark Findings

Эти два блока по 50 пунктов основаны на репозиториях 101-200, новых
первичных источниках и измерениях от 2026-07-18. Метка `Сделано` означает
изменение, уже присутствующее в Rust-коде или benchmark harness; остальные
пункты являются проверяемыми следующими шагами, а не выданными за готовность
обещаниями.

### Block 601-650: Measurement, Protocols, and Architecture

601. Сделано/Benchmark: заменить три произвольных sample на пять warm-up и 100 measured iterations по умолчанию, оставив явные environment overrides.
602. Сделано/Benchmark: сохранять average, standard deviation, p50, p95, p99, min, max, MPix/s и все raw samples вместо одного среднего значения.
603. Сделано/Benchmark: вращать порядок 32 compositor cases между независимыми процессными запусками, чтобы постоянная позиция не маскировала thermal/cache bias.
604. Сделано/Benchmark: записывать target arch, macOS version, rustc, CPU, power source, число warm-up/iterations и rotation в schema 2 JSON report.
605. Исправлена ошибка: baseline с AC Power больше нельзя сравнить с запуском на батарее; harness завершает сравнение сообщением об environment mismatch.
606. Исправлена ошибка: эксперимент `fast_image_resize` больше не использует однотонный кадр и первый пиксель как ложный quality oracle; теперь проверяются все пиксели patterned fixture.
607. Решение/Quality: не подключать `fast_image_resize`, пока adapter не воспроизводит принятую nearest-coordinate convention; текущий 960x540 эксперимент расходится во всех 518,400 пикселях.
608. Сделано/Benchmark: GPU experiment использует warm-up и 30 измерений, публикует average/p95/ratio и не принимает решение по одному sample.
609. Решение/Performance: сохранить CPU compositor по умолчанию: повторный optimized Apple M4 Max прогон измерил 0.470 ms CPU против 1.780 ms Metal+readback, то есть GPU путь медленнее в 3.79 раза.
610. Сделано/Performance: разделить профилированные Rayon thresholds для nearest и bilinear, чтобы дорогая bilinear работа распараллеливалась раньше, а малый nearest path не платил scheduler overhead.
611. Сделано/Benchmark: Rust runner выполнил три полных schema 3 процесса на стабильном AC Power; 9,600 raw samples проверены, а три native Apple M4 Max baseline обновлены только после bootstrap review без регрессий.
612. Сделано/Benchmark: отдельный summary строит детерминированный bootstrap 95% confidence interval по средним независимых процессов и принимает baseline-решение по интервалу, а не по одному проценту.
613. Сделано/Benchmark: добавлен seeded random case order; mode и seed сохраняются в каждом raw report и manifest.
614. Сделано/Benchmark: environment snapshots до/после фиксируют thermal/performance/CPU pressure, power mode, battery state, display count, load average, process count и background-load note.
615. Сделано/Benchmark: общий representative-patterned-v1 fixture содержит резкие границы, text-like детали, noise, odd dimensions; отдельная операция измеряет padded 641x479 stride -> tight materialization.
616. Частично сделано/Benchmark: отдельный Rust child process измеряет fixture-ready -> compose -> file-backed mmap -> borrowed extension-input acknowledgement; physical capture, CoreVideo/CMIO и внешний consumer честно перечислены как исключённые до signed-extension прогона.
617. Сделано/Performance: compositor, padded materialization и межпроцессный профиль сохраняют allocations, copy operations и bytes, включая нормализацию на output frame.
618. Профиль готов/Performance: soak schema 4 сохраняет wakeups, pageins, raw energy counters, allocator categories, rates и environment до/после; короткий smoke пройден, полный восьмичасовой AC qualification ещё требует времени и питания.
619. Сделано/Benchmark: межпроцессный отчёт раздельно публикует throughput, compose/publish service time, cross-process queue wait и fixture-to-extension-input latency.
620. Сделано/Reproducibility: AC artifact set объединяет raw reports, immutable baseline input, bootstrap summary, SHA-256 reports/summary/benchmark executable, git revision, dirty flag, структурированные argv/env, seeds и environment preflights в одном каталоге с manifest.
621. Решение/Media: RTP-to-host `FrameClock` остаётся requirement-gated; ADR 0006 запрещает смешивать будущие удалённые timestamps с локальными до появления реального network-source сценария.
622. Контракт/SOLID: ADR 0006 фиксирует будущий protocol engine как Sans-I/O state machine с централизованным `&mut self` и тонким runtime adapter.
623. Сделано/Correctness: `frame_integrity.rs` связывает sequence, timestamp, restart generation, discontinuity, stale/transport drop и cumulative counters в одном per-source state.
624. Сделано/Observability: `source_health.rs` хранит freshness, drops, jitter, reconnect, format и consumer acknowledgement; UI сворачивает их в non-color summary, сохраняя полное описание.
625. Сделано/Correctness: diagnostics rate-limit одинаковые drop events до одного сообщения за 100 ms, UI дедуплицирует одинаковый feedback, а счётчики потерь остаются точными.
626. Сделано/YAGNI: architecture test запрещает network-stack dependencies до утверждённого requirement; remote transport описан только как feature/adapter boundary в ADR.
627. Сделано/Media: `format_negotiation.rs` типизированно проверяет dimensions, pixel format, fps, colorimetry, latency и ownership и возвращает конкретную причину отказа.
628. Security gate готов: ADR и threat model требуют отдельные parser/state-machine fuzz corpora и resource assertions до включения будущего remote feature; сетевого parser в production graph пока нет.
629. Сделано/Backpressure: `backpressure.rs` задаёт policy/capacity для render, mmap, discovery и capture каналов; runtime использует bounded/latest-only очереди и проверяет контракт.
630. Сделано/Security: `docs/threat-model-remote-source.md` покрывает identity, encryption, replay, exhaustion, discovery, credentials и обязательные release gates.
631. Сделано/Verification: CI запускает закреплённый `cargo-hack` no-default и feature powerset для ARM64/x86_64; локально проверены все 15 комбинаций на обеих архитектурах.
632. Сделано/Verification: coverage job очищает stale data, объединяет library, bins и integration/process tests с all-features и публикует один LCOV artifact.
633. Сделано/API: release workflow запускает `cargo-semver-checks` относительно предыдущего `v*` tag до сборки подписанного релиза.
634. Сделано/DRY: scheduled/manual maintenance job запускает закреплённый `cargo-udeps`; локальный all-targets/all-features аудит не нашёл лишних зависимостей.
635. Сделано/Verification: mutation workflow ограничен файлами с сильными oracle для media/wire/frame/provisioning invariants, а не raw mutation count всего UI.
636. Сделано/Safety: `docs/unsafe-invariants.md` связывает каждый unsafe source с тестом/model/framework contract; architecture test требует запись для каждого такого файла.
637. Сделано/Concurrency: slot-state docs называют publication и consumer-ack linearization points; Loom проверяет claim/publish/read/release histories.
638. Сделано/Concurrency: Loom и child-process tests моделируют generation, producer restart/death, stale reader, recovery и PID-reuse token как связанные histories.
639. Сделано/Performance: одинаковая 640x360 copy+borrow workload сравнила mmap slot и mutex reference; mmap оказался примерно на 6.2% медленнее, поэтому усложнение не распространяется за IPC boundary.
640. Решение/Concurrency: не добавлять general lock-free queue, если bounded latest-frame slot или обычный lock удовлетворяет deadline и recovery contracts.
641. Сделано/Memory: soak schema 4 раздельно сохраняет allocator allocated/in-use/fragmentation, resident, physical footprint, peak RSS, maxima и signed final growth.
642. Решение/Memory: не менять системный allocator на mimalloc/jemalloc без устойчивого allocation profile и улучшения end-to-end budget.
643. Сделано/Safety: real child-process mmap suite покрывает truncate/SIGBUS containment, atomic replace, permission change и producer death во время чтения.
644. Сделано/Security: `parser_limits.rs` централизует byte/depth/element/string/allocation budgets до JSON/plist/spool/benchmark deserialization.
645. Сделано/Architecture: type/module docs и `docs/ownership-map.md` явно разделяют persisted source-of-truth, runtime-owned и derived snapshots.
646. Сделано/Architecture: `invalidation.rs` задаёт preview/output/persistence/undo graph; scene/UI вызовы передают typed `SceneChange` и пропускают независимый recompute.
647. Решение/YAGNI: не добавлять Salsa до появления сложного измеренного dependency graph; текущие явные reducers и snapshots проще проверять.
648. Сделано/SOLID: workers обмениваются typed jobs/results/snapshots; architecture test запрещает им ссылаться на `CameraManApp`.
649. Сделано/Architecture: `tests/contract_invariants.rs` исполняет wire/media/schema/capability invariants, дополняя protocol/process assertions.
650. Сделано/Learnability: `docs/ownership-map.md` даёт маршрут чтения от domain types к state, workers и platform adapters и фиксирует одну причину изменения для малых модулей.

### Block 651-700: Design, Quality, and Release

651. Сделано/Accessibility: `accessibility.rs` строит стабильные semantic identities для scene, sources, preview и status, а egui/AccessKit переводит их в платформенное дерево.
652. Сделано/Accessibility: source semantic IDs зависят от устойчивого source id; reconnect, reorder и безопасный scene update не пересоздают focus identity.
653. Сделано/Design: `scene_commands.rs` представляет edits/reorder/transforms typed-командами поверх одного bounded undo history.
654. Сделано/Design: gesture transaction обновляет preview непрерывно и coalesce-ит drag/crop в один undo commit.
655. Сделано/Design: edits имеют Undo, imports показывают validation preview, а discovery/self-test/export можно отменить без modal-first UX.
656. Сделано/Design: signing/profile/entitlement детали вынесены из рабочего потока в Setup и redacted diagnostics.
657. Сделано/UX: долгие discovery/self-test/export сразу меняют phase, показывают progress и безопасный Cancel.
658. Сделано/Accessibility: `ui_contracts.rs` автоматически проверяет минимальные pointer targets и устойчивую геометрию controls.
659. Сделано/Accessibility: reorder и transform доступны keyboard-командами и точными numeric controls, сохраняя тот же reducer path.
660. Сделано/Accessibility: status использует текст, icon/shape и accessible label; цвет остаётся вторичным сигналом.
661. Сделано/Visual Design: `ui_tokens.rs` централизует neutral/success/warning/destructive/focus/preview palette без однотонной темы.
662. Сделано/Localization: pseudo-long EN/RU fixture проходит minimum/default/large layouts с bounded wrap и без неоднозначного clipping.
663. Сделано/Visual QA: fixture script сохраняет decoded 1x/2x workspace, Setup, empty, disconnected, install-error и running states.
664. Сделано/Layout: minimum fixture сохраняет preview, Start/Stop, source mode и status; secondary controls прокручиваются без overlap.
665. Сделано/Design: `ui-state-contract.md` и deterministic fixture states задают recovery для empty/disconnected/stale/extension/activation/install failures.
666. Сделано/Accessibility: системные reduced-motion/increase-contrast/differentiate-without-color настройки меняют presentation, а activity не зависит от анимации.
667. Сделано/внешняя проверка: `accessibility-checklist.md` задаёт полный VoiceOver smoke; фактический проход остаётся release-candidate gate.
668. Сделано/Design: transform inspector принадлежит только selected source, а typed updates сохраняют selection, пока source существует.
669. Сделано/Information Architecture: рабочая иерархия остаётся Sources -> Preview -> Output/Status, Setup является отдельной task surface.
670. Сделано/Visual Design: sections остаются unframed bands/tool surfaces; nested и повсеместные floating cards удалены.
671. Сделано/Visual Design: workspace использует компактные rows, стабильные колонки, короткие labels и progressive disclosure.
672. Сделано/UX: source row показывает device name, negotiated format, freshness и non-color health state.
673. Сделано/Setup: activation/install failures показывают cause, следующий recovery action и сохраняют техническую диагностику.
674. Сделано/Supportability: bounded background worker экспортирует redacted versioned diagnostics JSON без device ids, paths, Team IDs или pixels.
675. Сделано/Error Design: `CameraManError::actionable_message` объединяет operation, cause и recovery, а UI не показывает сырой код без контекста.
676. Сделано/UX: discovery, self-test и file export имеют cancel-safe late-result handling и не блокируют egui event loop; activation закрывается как системная task.
677. Сделано/Data Integrity: общий atomic writer полностью пишет, sync-ит, валидирует и заменяет preferences/scenes/imported state, сохраняя старый файл при сбое.
678. Сделано/Recovery: canonical preferences восстанавливают подтверждённую scene/config, но output всегда стартует выключенным до явного Start.
679. Сделано/Quality tooling: exact golden дополнен PSNR/SSIM summary, seeded bootstrap 95% intervals, offline VMAF CLI и субъективным checklist; ручной media pass остаётся release evidence.
680. Сделано/Color: schema-1 primaries/transfer/matrix/range/alpha/8-bit contract проходит Frame, mmap v5, spool v4 и Rec.709 CMIO attachments с fail-closed readers.
681. Сделано/Quality: linear-light bilinear существует только под non-default `linear-light-experiment` и не принят без perceptual/color gate.
682. Сделано/Quality: deterministic odd-size fixture содержит text, 1px edges, diagonals, gradients, skin-like tones, chroma checker и crop markers.
683. Решение/GPU: не принимать GPU path, если upload/readback и synchronization делают end-to-end frame медленнее, даже при быстром kernel time.
684. Частично/Performance: feature-gated Rust CVMetalTextureCache bridge создаёт no-readback texture boundary; device-loss/recovery и real-consumer CPU-fallback measurement остаются аппаратным экспериментом.
685. Решение/Performance: Rayon остаётся non-default; включение ждёт AC process-level ARM64/x86_64 distributions и frame-budget acceptance.
686. Решение/Performance: новый hand-written SIMD не добавлен без bottleneck evidence; resize выбирается на operation boundary, а scalar CPU остаётся oracle.
687. Сделано/Maintenance: `features.md`, cargo-hack powerset CI и all-features Clippy фиксируют обещанные комбинации и политику удаления флагов.
688. Сделано/Governance: `dependency-policy.md` требует requirement, maintenance, license, costs, alternatives, exit strategy, attack surface и measured acceptance.
689. Сделано/Security: ADR и architecture test запрещают rustls/TUF/updater/network features до утверждённой функции и threat model.
690. Сделано/Release gate: `threat-model-self-update.md` блокирует updater до rollback/freeze/key-rotation/partial-install design и fault evidence.
691. Сделано/Reproducibility: pinned release workflow фиксирует commit/epoch/toolchain/lockfile, remap-ит workspace и побайтово сравнивает два чистых unsigned build; локальная пара совпала.
692. Сделано/Supply Chain: `cargo-auditable 0.7.5` встраивает locked graph; `cargo audit bin` извлёк по 232 зависимости из host и extension до и после bundling.
693. Сделано/Supply Chain: финальный ZIP извлекается и audit-ится, SPDX 2.3 сканируется OSV 2.3.8, SPDX 3.0.1 валидируется; исключения имеют owner/expiry.
694. Сделано/Supply Chain: immutable GitHub workflow создаёт hosted attestations отдельно для nested binaries, SBOMs и archive/evidence manifest; уровень SLSA сверх этого не заявляется.
695. Сделано/Release: `release-evidence.md` явно разделяет Apple signature, ticket, checksum, SBOM, SBOM provenance и artifact provenance.
696. Сделано/Release tooling: Rust `cameraman-release-manifest` атомарно индексирует version/commit/target/macOS/Xcode/SDK/bundle ids/Team ID/hashes/SBOM/notary/provenance; paid-signature execution остаётся внешним gate.
697. Сделано/Release: `release-rollback.md` задаёт withdrawal, credential revocation, replacement и schema-compatible user recovery.
698. Сделано/Research: `research-policy.md` требует dated upstream metadata/maintenance snapshots перед milestone и запрещает decisions по кратким колебаниям stars.
699. Сделано/Maintenance: quarterly workflow audit-ит advisories/policy/future incompatibility/features и публикует stale, duplicate, objc2/macOS/Xcode/SDK/release-gate reports.
700. Release milestone: считать приложение готовым к распространению только после AC baseline, восьмичасового soak, paid-profile install, notarization/stapling, реального CMIO consumer test и VoiceOver smoke.

## Implementation Status: 601-700

| Range | Status | Evidence or remaining gate |
|---:|---|---|
| 601-605 | Done | Schema 2 harness uses warm-up, 100 samples, rotated order, rich metadata/statistics, and AC-power baseline guard. |
| 606-607 | Done/decision retained | Patterned full-frame oracle found 518,400 mismatched pixels, so the faster scaler is rejected. |
| 608-609 | Done/decision retained | Thirty-sample GPU experiment measured readback path slower and keeps CPU production code. |
| 610 | Done | Nearest and bilinear use distinct profiled parallel thresholds. |
| 611-620 | Done/external gates | Native AC process series, bootstrap intervals, allocation/copy ledger and separate-process mmap latency are recorded; physical capture/CMIO and full eight-hour soak remain external acceptance gates. |
| 621-650 | Done/requirement-gated | Integrity/health/backpressure/parser contracts, formal concurrency/fault tests, feature/coverage/API/dependency CI, allocator telemetry and ownership map are implemented; remote RTP code remains intentionally absent until approved. |
| 651-678 | Done/external VoiceOver gate | Semantic IDs, commands, cancellation, atomic persistence, state contracts, redacted export and 15 decoded fixture variants are implemented; release-candidate VoiceOver execution remains manual. |
| 679-689 | Done/experiment-gated | Versioned color transport, representative quality/bootstrap tooling and feature/dependency policies pass; Metal device-loss and Rayon/SIMD adoption remain evidence-gated experiments. |
| 690-699 | Done/external release execution | Updater threat gate, reproducible auditable builds, archive/SBOM scanning, attestations, manifest, rollback and quarterly review are implemented; paid signing executes only in protected CI. |
| 700 | Externally blocked | Full AC/8h/paid-profile/notary/real-CMIO/VoiceOver evidence is required for distribution and is not claimed locally. |

## Review Summary

At the first rewrite, the active code review focused on signing/provisioning, stale documentation, app UI state, and unsafe extension boundaries. That pass intentionally held the numbered backlog at 500; two research passes later extend it to 700.

## Second Review Pass

Ran a 15-agent adversarial review targeted at exactly the four areas above, plus a design-critique pass on the current egui UI (via the design-critique skill) and a direct check of this machine's signing state. Every claimed finding below was independently re-verified by a second agent reading the source directly before being accepted.

Confirmed and fixed:

- `extension_main.rs`: `stop_streaming` only flipped a bool; `start_streaming` could race ahead and spawn a second `stream_samples` thread before the first noticed and exited, leaving two threads sending samples to the same `CMIOExtensionStream` concurrently. Fixed by having `start_streaming` join the previous worker thread before spawning a replacement.
- `app.rs`: a permanently broken camera in a multi-camera composite re-posted its error every tick, which reset the message's TTL every time and both blocked any other status-bar message forever and never engaged the auto-stop safety net (that only ever saw the all-cameras-failing case). Fixed with a per-source failure streak that posts once and auto-drops the camera after `MAX_CAPTURE_ERROR_STREAK`.
- `app.rs`: if every selected real camera disappeared while running, the status bar kept showing "Preview running" in green with nothing being captured. Fixed: this path now stops the preview and posts an explicit event.
- `main.rs` / `system_extension.rs`: the extension bundle id was still duplicated across the `Info.plist` template and the `.systemextension` path despite `architecture.md` already listing this as an open DRY violation from an earlier pass; unified behind `EXTENSION_BUNDLE_ID`.
- `app.rs`: the "Install extension" button stayed clickable even on an ad-hoc-signed bundle where activation is guaranteed to fail. It is now gated on `extension_capability()`, which checks both that the process runs from an installed `.app` bundle and that the bundle's own entitlements (read via `codesign`) actually grant `system-extension.install`.
- `app.rs`: the control column had no scroll area, so the Output/System Extension controls were clipped at the bottom even at the default window size. Wrapped in `egui::ScrollArea::vertical()`.
- `app.rs`: the install-extension button and status shared no visual grouping with the section above it. Gave it its own "System Extension" label and separator.
- `architecture.md`: test count claimed 20, actual is 22; bundle-id duplication claimed as an open "Still open" item after it had already been fixed elsewhere in this same pass; Module Map for `system_extension.rs` omitted the `EXTENSION_BUNDLE_ID` constant; the install-extension-button design issue described the pre-fix behavior. All corrected.
- `README.md`: the install-extension-button description and "Current UI gaps" list described the pre-fix behavior. Corrected.

Checked and confirmed NOT a bug (no action taken):

- `extension_main.rs`'s `stream_samples` divides by the transport-reported fps; verified both RAM and file readers clamp it to at least 1, so no division-by-zero path exists.
- The explicit file fallback's temp-file-then-rename strategy remains race-safe: a reader sees a complete pre- or post-rename file, never a torn one.

Fixed in the final publication pass:

- Fixed: `Requesting` now has an accent status, spinner, and disabled state-aware button instead of looking identical to idle.
- Fixed: fixed fps mode warns when the target exceeds the slowest negotiated camera rate and frames may repeat.
- Fixed: provisioning and signed-entitlement checks now use a structured plist parser instead of raw substring matching.
- Fixed: app plist templates now consume shared app-id and entitlement constants, closing another DRY gap.

Still open (see architecture.md's per-iteration "Still open" lists for the full picture):

- A real camera whose `open()` call itself hangs is indistinguishable from one merely warming up, so it never counts toward any failure streak. Needs a bounded, generous open-timeout in the capture worker.
- Client authorization in the CMIO extension is logged but not actually enforced (any local process can still connect).
- Production bundles now validate matching App Group entitlements and resolve a shared container mmap; a real paid-profile machine test remains external.
- Borrowed mmap reads removed the intermediate owned Rust frame; one measured mmap publish copy and one pooled `CVPixelBuffer` upload remain, with IOSurface still evidence-gated.
- Real system-extension install still needs a paid Apple Developer Program membership with the System Extension capability; the free personal-team certificate now present on this machine (`Apple Development: d.o.mezhov@gmail.com`, team `VBA8KCMNX7`) can sign a plain app but cannot carry that capability. Next hardening step: validate Team ID/certificate/profile matching, not just the app id and entitlement.

## Third Review Pass

- At this third pass, exactly 500 numbered items remained in this file.
- `cargo fmt --all --check`, strict Clippy, and all 44 tests pass.
- App and extension bundles build, pass strict code-sign verification, and contain valid plist files.
- Automated window capture is still pending because macOS denied Screen Recording permission to the test process; the native app itself launched.

## Fourth Review Pass: RAM Transport

- At this pass default virtual output used a page-aligned three-slot POSIX ring; the eighteenth pass adds the App Group mmap backing for installed bundles.
- Slot states enforce exclusive writing/reading, and a concurrent stress test verifies that no torn frame is observable.
- Reader ownership includes its PID, so slots held by a crashed extension are reclaimed without touching a live reader.
- Writer ownership uses an atomic PID because macOS returns `ENOTSUP` for `flock` on POSIX shm; dead writers can be replaced safely.
- Graceful disconnect clears ownership and unlinks the shared-memory name; readers detect the stopped writer and reconnect when a new app publisher appears.
- The extension uses a row-copy fast path when the incoming frame is already 1920x1080, avoiding millions of four-byte copy calls per frame.
- The App Group deployment gap is closed in the eighteenth pass; IOSurface remains the data-path optimization.

## Fifth Review Pass: Latency and UI Thread

- `Frame` now uses copy-on-write pixel storage, so capture/UI/worker handoffs do not duplicate full buffers.
- `compose_captured` borrows source pixels; output writes acquire one mutable slice per operation instead of checking COW state for every pixel.
- Horizontal source offsets are precomputed once per destination row width, removing millions of repeated divisions from a full-HD composition.
- `RenderWorker` owns composition and virtual publication. It has one replaceable pending slot, and the concurrency test proves an intermediate queued job is dropped.
- Render epochs prevent an old source/layout result from replacing the current preview; mode changes also clear the old texture immediately.
- Preview conversion/upload is independently limited to 960x540 and 30 fps while full 1080p output remains available to transport and export.
- Successful AVFoundation reads no longer add a fixed 10 ms sleep; only failures use retry backoff.
- Strict all-target Clippy and all 49 tests pass after this review.

## Sixth Review Pass: Metrics and Verification

- Generated property checks cover 2,000 reproducible combinations of dimensions, source counts and layouts.
- The compositor has a checked-in P3 golden image and a stable Rust full-HD benchmark.
- Three 30-frame release benchmark runs measured 0.84 ms, 0.86 ms and 0.88 ms per composed frame on this machine.
- `PipelineMetrics` records source reads/receives/drops/errors and source/render/sink durations per tick and cumulatively.
- `Ok(None)` is tested as a dropped source frame without being misreported as an error.
- The standalone `frame_new_checked` libFuzzer package builds successfully.
- The suite now contains 52 passing tests before the final publication checks.

## Seventh Review Pass: Scaling Quality

- `ScalingFilter` makes quality explicit: `Nearest` remains the fast default and `Bilinear` is the smooth option.
- The egui control panel exposes the choice as a compact `Fast / Smooth` segmented control.
- The worker applies filter changes asynchronously and render epochs reject previous-setting preview and transport publication.
- Exact-size inputs use a row-copy fast path instead of entering either scaler.
- Bilinear interpolation is covered by exact edge-gradient expectations.
- Grid composition now explicitly covers 5, 7 and 8 sources.
- The suite reaches 54 tests before the final strict run.

## Eighth Review Pass: Memory Policy and Error API

- Fixed 1 GiB validation was replaced by `FrameLimits`: one sixteenth of detected physical memory, clamped to 64 MiB–1 GiB, with an environment override and explicit per-call limits.
- Limit failures now report both requested bytes and the active budget through a dedicated structured variant.
- `ErrorCode`, `user_message`, diagnostic `Display`, and recursive `.context(...)` separate UI recovery text from developer detail without adding a dependency.
- Typed nokhwa unsupported variants bypass text classification; representative AVFoundation messages lock the remaining heuristic behavior.
- App status events now use user-facing messages while CLI and logs retain complete context chains.
- Crate-level rustdoc, trait lifecycle contracts, and executable custom source/sink examples were added; `cargo doc --no-deps` succeeds.
- Three 30-frame benchmark runs measured exact copy near 0.27 ms, nearest at 0.85–0.88 ms and bilinear at 4.36–4.42 ms, so unsafe explicit SIMD is not justified by the current 60 FPS budget.
- The suite reaches 60 tests before final strict verification.

## Ninth Review Pass: Frame Views and Latency

- Pipeline metrics now track capture-to-sink latency samples, total and maximum using capture metadata timestamps.
- Per-source sequence history distinguishes a repeated stale frame from both `Ok(None)` and a source error.
- `FrameView<'a>` validates borrowed dimensions, row stride and accessible bytes without allocating or copying.
- Active rows exclude padding; explicit conversion copies only active bytes into a memory-limited tightly packed `Frame`.
- Owned `Frame` remains deliberately tight and copy-on-write, keeping renderer and transport contracts simple.
- `try_set_bgra` returns a structured `PixelOutOfBounds`, while `set_bgra` preserves debug assertion and release clipping behavior.
- Boxed sources remain the deliberate heterogeneous extension boundary; the synchronous core pipeline and asynchronous app worker retain separate responsibilities.
- The suite reaches 64 tests before final strict verification.

## Tenth Review Pass: Debug Artifacts and CI

- `PpmSequenceSink` now gives every frame a sequence/timestamp filename and writes a serde-backed JSON sidecar with source, sequence, dimensions, timestamp and pixel format.
- The low-level `write_ppm` byte stream is locked by a checked-in valid P6 golden whose final newline is actual blue-channel pixel data.
- Sidecar and timestamp naming behavior are exercised through the real sink lifecycle.
- `tests/cli_smoke.rs` invokes the compiled binary and validates `status`, `check` and `help` output.
- A macOS GitHub Actions workflow runs formatting, strict all-target Clippy, tests, fuzz-package checks, CLI smoke, release bundling, plist validation and strict codesign verification.
- The suite reaches 68 tests before final strict verification.

## Eleventh Review Pass: Platform and Repository Hygiene

- Objective-C/CoreMediaIO dependencies are now declared only for macOS targets; platform code remains inside narrow `cfg(target_os = "macos")` adapters with an explicit non-mac extension fallback.
- Legacy `App/`, `Extension/` and `Shared/` directories contain no source files and are absent from the tracked tree.
- `.idea`, `.DS_Store`, build output and the entire signing directory remain ignored rather than deleting user-local state.
- Private key, certificate, CSR and provisioning-profile extensions are ignored globally, not only under `signing/`.
- `tests/repository_hygiene.rs` checks tracked and non-ignored candidate files for Swift sources, Finder/IDE/build metadata and private signing artifacts.
- The suite reaches 69 tests before final strict verification.

## Twelfth Review Pass: Packaging Contracts and Diagnostics

- Virtual camera device/stream identity is shared by the config and CoreMediaIO process instead of repeated literals.
- App and extension Info.plist/entitlement data is built with typed plist dictionaries and round-trip tested.
- Regression tests lock bundle identifiers, Mach service, Cargo version, embedded extension location and debug/release binary selection.
- Release or real-identity `codesign` failures stop bundling with exact command, status and stderr; debug ad-hoc remains warning-only.
- `version`, `bundle --help` and resilient `diagnose-extension` commands are exercised through the compiled CLI binary.
- Diagnostics report profile validation, expected/embedded entitlements, bundle presence and `systemextensionsctl list` state.
- CI runs the new diagnostics before release bundling, plist lint and strict signature verification.
- The suite reaches 78 tests before final strict verification.

## Thirteenth Review Pass: Persistent Utility UI

- A dedicated `AppPreferences` value persists bounded user choices through eframe while camera handles, workers, frames and textures remain runtime-only.
- A one-request `CameraDiscoveryWorker` moves startup/refresh enumeration off egui and exposes progress through a spinner.
- The fixed control column became a resizable 260–420 px panel with scrolling; export destination is editable and the transport endpoint is copyable.
- Sticky errors, transient confirmations and derived live state no longer overwrite one another; long status text truncates before stable right-aligned metrics.
- Real source rows show health and composition order, waiting frames receive an overlay, and capture errors include the source name and actionable permission path.
- Signing controls show provisioning-profile state and link to the exact recovery section.
- Direct Glow framebuffer captures pass at 1120x720 and the 920x560 minimum without overlap or inaccessible fixed content.
- The suite reaches 84 tests before final strict verification.

## Fourteenth Review Pass: Primary Layout and Signing Identity

- Accessible up/down controls persist selected-source order; focused widgets suppress the global Space shortcut.
- `PictureInPicture` renders source zero as the primary full frame and later sources as bounded right-side overlays.
- Dynamic real-device labels truncate before health/order controls and retain their full accessible/hover name.
- Provisioning validation extracts explicit Team IDs, rejects internal conflicts and never mistakes legacy App ID Prefix for Team ID.
- Installable bundling rejects app-signature/profile Team mismatch before embedding; CLI compares app, extension and profile Team IDs.
- The UI exposes launch context and signing Team ID and copies a nonblocking diagnostics summary.
- The suite reaches 91 tests before final strict verification.

## Fifteenth Review Pass: Nested Provisioning

- Installable packaging now accepts a separate extension-target profile through `CAMERAMAN_EXTENSION_PROVISIONING_PROFILE`.
- The nested profile must match the extension bundle id, grant app sandbox and carry the same Team ID as the actual signature.
- Host-profile mode fails early when the extension profile is missing instead of producing a bundle that cannot validate.
- Each profile is embedded in its own bundle before inside-out signing.
- CLI diagnostics prefer embedded profiles and print extension-profile Team match separately; the app report shows whether the nested profile is present.
- At this pass App Group wiring and a paid-profile install were open; the eighteenth pass closes the wiring while the real install remains external.
- The suite remains at 91 tests before final strict verification.

## Sixteenth Review Pass: Camera Reopen Ownership

- Threaded capture owns a process-local lease keyed by stable camera id.
- Drop still avoids a synchronous join, but a replacement worker waits off-thread until the old device owner exits.
- Immediate Stop -> Start can no longer race two CameraMan opens for the same source.
- Stop-aware lease waiting remains cancelable even when another owner is active.
- A backend call permanently wedged inside nokhwa can still delay lease release because no cancellation hook is available.
- Two deterministic lease tests bring the suite to 93 tests before final strict verification.

## Seventeenth Review Pass: Activation Readiness and FPS Contract

- Install activation now checks the nested bundle, strict signatures, host entitlement, both embedded profiles, and every Team ID relationship before enabling the request.
- App bundling rejects a partial profile pair or profiles combined with ad-hoc signing before replacing the previous artifact.
- Extension packaging verifies the final signed bundle and compares its actual Team ID with its own profile in both standalone and embedded modes.
- CoreMediaIO advertises a continuous 15–60 fps duration range; transport values are clamped to it instead of violating a fixed-duration declaration.
- Output dimensions, fps bounds/default, and segmented presets have one Rust source of truth across config, app, transports, and extension.
- Unsupported persisted fixed-fps values recover to Auto so runtime state always has a visible selected control.
- Four deterministic tests bring the suite to 97 tests before final strict verification.

## Eighteenth Review Pass: Real CMIO Activation and Cross-Role IPC

- The extension plist now includes mandatory `NSSystemExtensionUsageDescription`; activation readiness rejects missing or empty metadata.
- Provisioning parsing extracts App Groups, rejects malformed arrays, and chooses a common exact non-wildcard value across host and extension profiles.
- `CAMERAMAN_APP_GROUP` can select among several common groups but cannot introduce an unauthorized value.
- Both signed entitlement sets and plist files carry the selected group; the CMIO Mach service is generated beneath its prefix.
- Activation location is exactly `/Applications`, matching Apple's documented requirement rather than accepting unsupported lookalike directories.
- Bare binaries keep UID-scoped POSIX shm, while signed bundles resolve the shared App Group container and mmap the same three-slot atomic protocol there.
- Bundled mode ignores environment-only transport overrides that the independently launched extension cannot inherit.
- The required plist metadata, App Group parsing/selection, bundle path, and file-backed cross-process protocol add eight deterministic tests, bringing the suite to 105.

## Nineteenth Review Pass: 100-Repository and Literature Survey

- `research.md` records a dated, linkable survey of 100 repositories with 1,470,630 combined stars; all top-level READMEs and deeper material from 20 representative projects were inspected.
- Primary sources cover CMIO activation, Core Video pools/color/time, real-time latency, lock-free publication, perceptual quality, modularity, fuzzing, HCI/accessibility, notarization, SLSA, and SPDX.
- The new backlog adds exactly 100 non-duplicated items, 501-600, grouped by correctness, performance, media contracts, SOLID decomposition, observability, verification, design, product, and release security.
- Highest-priority findings are per-frame `CVPixelBufferCreate`, missing color attachments, sleep-after-work pacing drift, indefinite replay of stale frames, wall-clock latency metadata, and an unmodeled cross-process atomic protocol.
- The study does not introduce Swift or foreign application source; proposed implementation paths remain Rust-only, including `objc2` platform bindings and optional pure-Rust wgpu/SIMD experiments.

## Twentieth Review Pass: 551-600 Completion

The requested final work ran in three bounded passes rather than one opaque
rewrite:

- Pass one implemented and evidenced recommendations 551-600: formal protocol
  checks, fuzz/process coverage, product workflow, accessibility, scenes,
  release hardening and the fixed-hardware acceptance profile.
- Pass two attacked failure paths and corrected scene size/atomicity, gesture
  history, reconnect recovery, acceptance warmup/high-water RSS accounting,
  release cleanup and actual Setup/workspace fixture capture.
- Pass three reviewed the completed diff. It documented every Rust unsafe
  boundary, enabled `clippy::undocumented_unsafe_blocks = deny`, converted raw
  pixel writes to an explicit unsafe contract and fixed a zero-fps CoreMedia
  timescale edge case with a regression test.

Final evidence:

- `cargo test --locked --all-targets --all-features`: 112 library, 46 app,
  7 extension and 17 integration/process/property tests pass (182 total).
- Strict all-target/all-feature Clippy, rustdoc with warnings denied, formatting,
  shell/Python syntax, workflow YAML and `git diff --check` pass.
- Five Kani harnesses, six Miri tests and 2,000-run smokes for all three fuzz
  targets pass; child-process tests include crash/restart and slow readers.
- Four decoded-pixel UI fixtures pass at EN/RU, compact, Setup and Retina
  configurations and were also inspected for clipping and overlap.
- `cargo-deny`, RustSec and the exact future-incompat guard pass. Notices were
  regenerated; SPDX 3.0.1 validates 432 packages and 1,325 relationships.
- The release app bundle rebuilds as arm64, both plist files lint, and deep
  strict ad-hoc signature verification passes.
- The latest 1080p30/four-source M4 Max smoke delivered 90 frames, zero torn
  frames and 5.89 ms p95. Smoke intentionally does not qualify RSS.

No local test is presented as proof of the remaining external gates: the
eight-hour run, paid Apple host/extension profiles, notarization/stapling and a
real third-party virtual-camera consumer still require their actual environment.

## Twenty-First Review Pass: 601-700 Completion

The additional work also ran as three bounded review iterations:

- Pass one validated 100 new repository records, exact sequential numbering,
  percentile math and all Rust targets/features. It corrected schema 2 report
  rotation and the documented experimental feature names.
- Pass two recalculated 9,600 raw compositor samples, verified all local
  document links, exercised the Battery Power rejection against the AC
  baseline, and corrected one overstatement about summary-file links.
- Pass three passed strict Clippy/rustdoc, rebuilt the arm64 app/extension,
  linted both plists, verified the deep ad-hoc signature, confirmed the tree has
  no Swift/C/Objective-C source, and recaptured four decoded Retina UI fixtures.
- Visual review retained the compact work-tool hierarchy and removed Setup's
  meaningless collapsible title-bar state. No overlapping text or controls was
  found in default English, compact Russian, or Setup captures.
- The new benchmark results remain explicitly exploratory on Battery Power;
  an AC rerun, eight-hour soak, paid profiles, notarization, and real CMIO
  consumer remain external release gates rather than simulated completion.

## Twenty-Second Review Pass: 611-620 Execution

- Power became available during the pass, so the strict Rust runner completed
  three native schema 3 AC processes instead of leaving an external TODO.
- The named artifact set contains 9,600 raw samples, three distinct seeds,
  stable AC/thermal snapshots, immutable baseline input, bootstrap summary,
  structured argv/env, Git state, and verified SHA-256 values for every report,
  summary, baseline input and benchmark executable.
- Two compared cases improved, one had no significant change and none
  regressed. Only the three measured native M4 Max values were updated;
  unmeasured Rosetta values were preserved.
- A representative odd-size/patterned fixture, padded-row materialization,
  separate-process mmap acknowledgement and one-second sustained smoke passed.
  The latter delivered 30/30 frames with no late, torn, sequence or generation
  errors and is not presented as an eight-hour energy qualification.
- Final review rejected mixed-build statistical summaries, removed duplicate
  materialization accounting, made executed commands identical to serialized
  invocations, copied baseline input into the artifact set and rejected
  non-empty output directories.
- Verification passes 120 library, 46 app, 7 extension, 17 integration/process
  and 1 runner test (191 total), strict all-feature Clippy/rustdoc, all 32
  compositor cases, arm64 bundle/plist/signature checks, and four visually
  reviewed Retina fixtures.
- Remaining external gates are the complete eight-hour run and physical
  camera -> signed CoreMediaIO extension -> third-party consumer evidence with
  paid provisioning and notarization.

## Twenty-Third Review Pass: Performance Hot Paths

- The numbered backlog remains exactly 700 items; this pass changes evidence
  and implementation rather than inflating the list.
- A previously unmeasured transform matrix exposed 10.65-63.14 ms generic
  crop/fill/opacity paths. Cached integer opaque paths reduce the final 1080p
  p95 range to 1.02-7.93 ms across nearest and bilinear cases.
- Redundant output/cell clears, duplicate render submissions, capture's second
  BGRA allocation, repeated unchanged spool reads, unbounded preview coordinate
  caching and per-tick process liveness checks were removed or bounded.
- Extension pacing now reads after the deadline wait, avoiding one deliberate
  frame interval of age, and its CoreVideo pool has a tested six-buffer upper
  bound with last-frame repetition under consumer backpressure.
- The separate-process E2E run reaches 159.53 frame/s with 6.44 ms p95 through
  mmap acknowledgement, while explicitly exposing the remaining 16.59 MB per
  frame before CoreVideo upload.
- [`docs/performance-review.md`](docs/performance-review.md) records the full
  evidence, remaining priority order and a crate-by-crate Rust-only greenfield
  architecture. Physical camera/CMIO, full soak and paid release gates remain
  external and are not claimed as complete.

## Twenty-Fourth Review Pass: Typed Source Runtime

- Iteration one replaced the global synthetic/real mode and parallel id arrays
  with schema-v4 ordered `SourceDescriptor` values. Preferences and scenes
  migrate legacy selections and transform keys without serializing workers.
- Iteration two made selection, reorder, missing-source policy, scene commands,
  Undo/Redo and composition operate on the same heterogeneous graph. Generated
  and physical-camera sources can now share one scene.
- Iteration three moved cadence into an owned `MediaClock`, removed the second
  production pipeline route, and retained `PipelineEngine` only as a Rust SDK
  example and test harness.
- Final review fixed false synthetic health state, removed per-tick lookup-map
  allocation, preserved mixed output when one camera fails, migrated physical
  cameras to stable AVFoundation `uniqueID` locators and negotiated the target
  camera format. It also aligned scene/source parser limits, kept reconnect in
  error state until the first new frame, and made legacy/stable camera aliases
  share one Retry lease identity. The 700 numbered recommendations remain
  exactly 1 through 700.

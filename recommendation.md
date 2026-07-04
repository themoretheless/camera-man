# CameraMan Recommendations

Exactly 500 numbered items: done work, improvements, problems, mistakes, design notes, and review findings.

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
22. Сделано: CLI `pipeline-demo` пишет последовательность PPM.
23. Сделано: CLI `check` печатает нормализованный config.
24. Сделано: тесты покрывают frame validation.
25. Сделано: тесты покрывают layout calculation.
26. Сделано: тесты покрывают composition.
27. Сделано: тесты покрывают PPM writer.
28. Сделано: тесты покрывают pipeline ticks.
29. Сделано: тесты покрывают failing source degradation.
30. Сделано: тесты покрывают capture error classification.
31. Сделано: тесты покрывают frame-spool round trip.
32. Улучшение: добавить property tests для layout.
33. Улучшение: добавить fuzz target для `Frame::new_checked`.
34. Улучшение: добавить golden images для compositor.
35. Улучшение: добавить benchmark для compositor.
36. Улучшение: измерять render duration.
37. Улучшение: измерять source read duration.
38. Улучшение: измерять sink send duration.
39. Улучшение: добавить dropped-frame metrics.
40. Улучшение: добавить `PipelineMetrics`.
41. Проблема: nearest-neighbor scaling даёт грубую картинку.
42. Улучшение: добавить bilinear scaling как опцию.
43. Улучшение: оставить nearest-neighbor как быстрый режим.
44. Проблема: compositor пока без SIMD.
45. Улучшение: позже добавить SIMD path для BGRA copy.
46. Проблема: нет теста на 5 источников.
47. Проблема: нет теста на 7 источников.
48. Проблема: нет теста на 8 источников.
49. Улучшение: расширить layout tests для odd counts.
50. Улучшение: тестировать tiny output dimensions.
51. Сделано: tiny layout не должен выходить за bounds.
52. Сделано: zero-sized frame отвергается.
53. Сделано: wrong buffer length отвергается.
54. Сделано: слишком большой buffer отвергается.
55. Проблема: `MAX_FRAME_BYTES` выбран грубо.
56. Улучшение: связать max bytes с platform memory budget.
57. Проблема: `CameraManError` ещё смешивает user и developer messages.
58. Улучшение: разделить display text и diagnostic text.
59. Улучшение: добавить structured error codes.
60. Улучшение: добавить context chaining без внешнего crate.
61. Проблема: `CaptureErrorKind::classify` эвристический.
62. Улучшение: перейти на structured backend errors там, где backend позволяет.
63. Сделано: классификация capture errors покрыта тестами.
64. Ошибка исправлена: broad `already` мог перекрывать not-found cases.
65. Улучшение: добавить реальные AVFoundation/nokhwa error samples.
66. Проблема: API library пока не документирован rustdoc.
67. Улучшение: добавить rustdoc для публичных traits.
68. Улучшение: добавить пример создания custom `FrameSource`.
69. Улучшение: добавить пример создания custom `VirtualCameraSink`.
70. Проблема: `PipelineEngine` generic по sink, но sources boxed.
71. Улучшение: оценить typed source lists для hot paths.
72. Проблема: boxed sources проще, но имеют dynamic dispatch.
73. Улучшение: оставить boxed API ради plugin-like расширяемости.
74. Проблема: `PipelineEngine` не владеет render loop.
75. Улучшение: сделать отдельный `RenderLoop` worker.
76. Улучшение: добавить cancellation token для render loop.
77. Улучшение: добавить bounded channel для frame reports.
78. Проблема: backpressure model пока не формализован.
79. Улучшение: описать drop-latest/drop-oldest strategy.
80. Улучшение: добавить latency metric.
81. Улучшение: добавить per-source stale-frame metric.
82. Проблема: `Frame` всегда owns `Vec<u8>`.
83. Улучшение: рассмотреть borrowed frame view для zero-copy paths.
84. Улучшение: добавить row-stride aware frame view.
85. Проблема: current frame assumes tightly packed BGRA.
86. Улучшение: явно документировать tight packing.
87. Улучшение: добавить conversion from padded CVPixelBuffer later.
88. Проблема: `set_bgra` silently clips in release.
89. Улучшение: иметь checked writer для debug tools.
90. Улучшение: оставить clipping для renderer resilience.
91. Проблема: PPM debug output не хранит metadata.
92. Улучшение: рядом писать `.json` sidecar.
93. Улучшение: добавить timestamp в PPM sequence filename.
94. Проблема: `write_ppm` не проверяется визуально.
95. Улучшение: добавить tiny PPM golden test.
96. Сделано: PPM writer использует buffering.
97. Проблема: нет integration tests в `tests/`.
98. Улучшение: вынести CLI smoke tests в integration suite.
99. Улучшение: проверять `cargo run -- check` в CI.
100. Улучшение: проверять `cargo run -- status` в CI.
101. Проблема: CI не описан.
102. Улучшение: добавить GitHub Actions для fmt/clippy/test.
103. Улучшение: добавить macOS job для bundle smoke.
104. Проблема: Linux CI не сможет проверить CoreMediaIO.
105. Улучшение: `cfg(target_os = "macos")` держать узким.
106. Сделано: extension binary имеет non-mac fallback.
107. Проблема: docs раньше ссылались на `arhitecture.md`.
108. Сделано: создан правильный `architecture.md`.
109. Улучшение: удалить все ссылки на старое имя файла.
110. Проблема: старый mixed-language layout directories ещё лежат в repo.
111. Улучшение: проверить `App/`, `Extension/`, `Shared/` и удалить/архивировать лишнее.
112. Проблема: `.DS_Store` присутствует в рабочем дереве.
113. Улучшение: убедиться, что `.DS_Store` игнорируется.
114. Проблема: `.idea` может быть user-local.
115. Улучшение: решить, хранить ли IDE files в repo.
116. Проблема: `target/` должен быть ignored.
117. Сделано: generated build output не должен попадать в commit.
118. Улучшение: добавить `signing/` в ignore, если там только generated files.
119. Проблема: ручные signing artifacts могут случайно утечь.
120. Улучшение: документировать env vars для signing.
121. Проблема: `VirtualCameraConfig` UID продублирован в extension constants.
122. Улучшение: вынести shared constants в library.
123. Проблема: `EXTENSION_BUNDLE_ID` дублируется в коде.
124. Улучшение: иметь single source of truth for bundle ids.
125. Проблема: Info.plist создаётся string literal.
126. Улучшение: template builder снизит риск typo.
127. Улучшение: проверить plist через `plutil` в tests/scripts.
128. Проблема: signing helper молча продолжает после failure.
129. Улучшение: возвращать error для release signing failure.
130. Улучшение: ad-hoc signing failure можно оставить warning.
131. Проблема: `codesign` output не структурирован.
132. Улучшение: печатать exact command context.
133. Проблема: release/debug bundle mode раньше мог расходиться.
134. Сделано: release bundle builds release extension.
135. Улучшение: добавить test/assert на embedded binary path.
136. Проблема: CLI args hand-written.
137. Улучшение: добавить lightweight parser или оставить hand-written до роста.
138. Проблема: help text не показывает signing env vars.
139. Улучшение: добавить `bundle --help` later.
140. Проблема: no version command.
141. Улучшение: добавить `cargo run -- version`.
142. Проблема: no diagnostics command.
143. Улучшение: добавить `diagnose-extension`.
144. Улучшение: diagnose should print app entitlements.
145. Улучшение: diagnose should print extension entitlements.
146. Улучшение: diagnose should print systemextensionsctl list.
147. Улучшение: diagnose should detect missing provisioning profile.
148. Проблема: current docs are correct but not executable.
149. Улучшение: add checklist commands for release install.
150. Улучшение: add troubleshooting for `No matching profile found`.
151. Проблема: README earlier claimed remaining bridge work after bridge existed.
152. Сделано: README rewritten to current truth.
153. Проблема: old recommendations exceeded 500 numbered items.
154. Сделано: recommendation list normalized to exactly 500 items.
155. Улучшение: keep review findings non-numbered when request says 500.
156. Проблема: docs can drift quickly.
157. Улучшение: add doc checklist before release.
158. Улучшение: mention actual test count after each major change.
159. Проблема: no changelog.
160. Улучшение: add `CHANGELOG.md` after first stable milestone.
161. Проблема: no license visible.
162. Улучшение: add license decision.
163. Проблема: no contribution guide.
164. Улучшение: add short dev workflow later.
165. Проблема: no architecture diagram asset.
166. Улучшение: keep text diagrams in markdown for now.
167. Улучшение: add Mermaid diagram only when renderer supports it.

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
177. Сделано: output toggle controls frame-spool sink.
178. Сделано: install-extension button requests activation.
179. Сделано: activation status is shown in the UI.
180. Сделано: PPM export is available.
181. Сделано: FPS presets and Auto mode exist.
182. Сделано: active fps updates compositor and transport.
183. Сделано: render cadence follows active fps.
184. Сделано: virtual sink stores target fps in transport header.
185. Сделано: extension can adapt cadence from transport fps.
186. Проблема: install button is visible even when entitlement/profile is missing.
187. Улучшение: preflight entitlement before enabling install button.
188. Улучшение: show signing state in UI.
189. Улучшение: show provisioning-profile state in UI.
190. Улучшение: link status to troubleshooting docs.
191. Проблема: app settings are not persisted.
192. Улучшение: persist input mode.
193. Улучшение: persist selected layout.
194. Улучшение: persist fps mode.
195. Улучшение: persist export path.
196. Улучшение: persist virtual output toggle.
197. Проблема: left panel width is fixed.
198. Улучшение: use `egui::SidePanel` with resizable width.
199. Проблема: output path is monospace text only.
200. Улучшение: add reveal/copy action for spool path.
201. Проблема: status event slot is single.
202. Улучшение: separate sticky errors from transient confirmations.
203. Проблема: switching Synthetic/Real stops stream silently.
204. Улучшение: show explicit mode-switch event.
205. Проблема: stale preview may remain after mode switch.
206. Улучшение: clear preview or badge it as stale.
207. Проблема: measured fps counts loop ticks.
208. Улучшение: count successfully composed frames.
209. Проблема: waiting-for-camera can still show previous frame.
210. Улучшение: add waiting overlay.
211. Проблема: no spinner during camera discovery.
212. Улучшение: discover cameras in background.
213. Проблема: refresh cameras can block UI.
214. Улучшение: move refresh to worker.
215. Проблема: first launch camera permission flow is not guided.
216. Улучшение: detect permission denied and show recovery text.
217. Проблема: capture errors are shown as raw backend strings.
218. Улучшение: map common errors to user-action text.
219. Сделано: capture errors have typed `CaptureErrorKind`.
220. Улучшение: attach source name to per-source errors.
221. Проблема: partial real-camera failure uses one status slot.
222. Улучшение: show per-source health indicators.
223. Проблема: source selection order is implicit.
224. Улучшение: show composition order numbers.
225. Проблема: no drag reorder.
226. Улучшение: add reorder controls later.
227. Проблема: no primary camera mode.
228. Улучшение: add picture-in-picture layout.
229. Улучшение: add one-large-many-small layout.
230. Улучшение: add equal grid presets for 5/7/8 sources.
231. Проблема: no labels on rendered cells.
232. Улучшение: optional source labels overlay.
233. Проблема: no crop/fit mode controls.
234. Улучшение: add fit/fill/crop choice.
235. Проблема: no per-source mute/disable while running.
236. Улучшение: allow uncheck without stopping all sources.
237. Сделано: unchecked real source is released.
238. Проблема: Stop -> immediate Start can race camera release.
239. Улучшение: add source shutdown acknowledgement.
240. Улучшение: block reopen same id until old thread exits.
241. Проблема: capture timeout helper can leave detached work.
242. Улучшение: design cancellable capture open.
243. Проблема: nokhwa cancellation hooks may be limited.
244. Улучшение: document backend limitation clearly.
245. Проблема: Continuity Camera behavior may differ from built-in camera.
246. Улучшение: add device-specific diagnostics.
247. Проблема: no camera format selection.
248. Улучшение: expose resolution/fps choices later.
249. Проблема: `SOURCE_WIDTH`/`SOURCE_HEIGHT` fixed for synthetic.
250. Улучшение: allow synthetic test resolution presets.
251. Проблема: app preview uploads full 1080p texture.
252. Улучшение: measure texture upload cost.
253. Улучшение: use lower preview texture while output remains 1080p if needed.
254. Проблема: frame spool writes full 1080p per frame.
255. Улучшение: move to shared memory or IOSurface.
256. Проблема: file spool has no backpressure.
257. Улучшение: include producer heartbeat and stale timeout.
258. Сделано: transport includes sequence and timestamp.
259. Сделано: transport version is explicit.
260. Улучшение: add CRC or checksum for debug validation.
261. Проблема: malformed spool currently becomes None.
262. Улучшение: count malformed spool events.
263. Проблема: file path default depends on OS temp dir.
264. Улучшение: show resolved path in diagnostics.
265. Проблема: no cleanup of stale spool file on exit.
266. Улучшение: optionally remove spool on disconnect.
267. Проблема: removing spool could blank extension unexpectedly.
268. Улучшение: keep last frame unless explicit cleanup requested.
269. Проблема: app has no visual automated tests.
270. Улучшение: add screenshot smoke through app automation.
271. Проблема: no mobile/narrow viewport because desktop app only.
272. Улучшение: test minimum window size.
273. Проблема: fixed text may overflow in narrow panel.
274. Улучшение: audit all UI labels at min width.
275. Проблема: buttons use text where icons may help.
276. Улучшение: add icons if egui icon source is chosen.
277. Проблема: color palette is serviceable but plain.
278. Улучшение: refine contrast and semantic states.
279. Проблема: warning and error colors may be close for colorblind users.
280. Улучшение: add icons/shapes with color.
281. Проблема: no dark/light theme switch.
282. Улучшение: keep dark-only until product stabilizes.
283. Проблема: no keyboard shortcuts list.
284. Улучшение: show shortcuts via tooltips only.
285. Сделано: Space toggles start/stop when keyboard is free.
286. Проблема: Space may conflict with focused button behavior.
287. Улучшение: audit egui focus handling.
288. Проблема: no menu bar.
289. Улучшение: add menu only when commands grow.
290. Проблема: no crash reporting.
291. Улучшение: log to file in app support directory.
292. Проблема: no structured tracing.
293. Улучшение: add `tracing` later if needed.
294. Проблема: no user-facing diagnostics export.
295. Улучшение: add "Copy diagnostics" button.
296. Проблема: app cannot open docs from UI.
297. Улучшение: add Help menu after signing flow is stable.
298. Проблема: app does not distinguish launch bundle from cargo run.
299. Улучшение: show "running from bundle" status.
300. Проблема: install activation only works from installed app bundle.
301. Улучшение: detect non-bundle launch and disable activation.
302. Проблема: `/Applications` install is manual.
303. Улучшение: add `cargo run -- install-app` later.
304. Проблема: overwriting `/Applications/CameraMan.app` can be destructive.
305. Улучшение: backup existing install before replace.
306. Проблема: current UI does not show Team ID.
307. Улучшение: show signing identity in diagnostics.
308. Проблема: no provisioning profile parser.
309. Улучшение: parse embedded profile to confirm entitlement.
310. Проблема: Xcode-managed signing is outside current CLI.
311. Улучшение: document manual signing path.
312. Проблема: user can click Install repeatedly.
313. Улучшение: disable while Requesting.
314. Сделано: delegate/request are retained while installer lives.
315. Проблема: repeated activation replaces active request.
316. Улучшение: explicitly cancel or reject duplicate activation.
317. Проблема: delegate queue is main dispatch queue.
318. Улучшение: evaluate serial queue for CLI diagnose/install.
319. Проблема: activation status is not persisted.
320. Улучшение: query `systemextensionsctl list` for installed state.
321. Проблема: `systemextensionsctl` output is not parsed.
322. Улучшение: parse it for diagnostics only, not core logic.
323. Проблема: no uninstall helper.
324. Улучшение: document `systemextensionsctl uninstall`.
325. Проблема: uninstall requires Team ID.
326. Улучшение: print Team ID after signing.
327. Проблема: real install cannot be fully automated without user approval.
328. Улучшение: make that explicit in README and UI.
329. Сделано: README now states provisioning requirement.
330. Проблема: app still allows impossible activation in ad-hoc mode.
331. Улучшение: code should prevent that path.
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
347. Сделано: extension reads frame spool.
348. Сделано: extension can scale incoming frame to output size.
349. Сделано: extension uses host-time timestamps.
350. Сделано: extension has a run loop.
351. Сделано: video format description is cached.
352. Сделано: creation failure is logged instead of hidden panic.
353. Проблема: extension FFI code is unsafe-heavy.
354. Улучшение: isolate more unsafe blocks behind small wrappers.
355. Проблема: ObjC callback panics can cross runtime boundary.
356. Улучшение: wrap callback bodies in `catch_unwind`.
357. Проблема: stream handle stores raw pointer-like value.
358. Улучшение: replace with retained object managed safely across thread.
359. Проблема: client authorization is not enforced.
360. Улучшение: verify client signing identity or bundle id if API allows.
361. Сделано: client id logging exists.
362. Проблема: client id is not security identity.
363. Улучшение: document that logging is not authorization.
364. Проблема: Core Foundation Create Rule is easy to violate.
365. Улучшение: audit every `from_raw`.
366. Проблема: extension error reporting is mostly `eprintln!`.
367. Улучшение: add structured extension logging.
368. Проблема: extension has no health endpoint.
369. Улучшение: expose heartbeat through transport later.
370. Проблема: frame spool reader reads whole file per frame.
371. Улучшение: mmap or shared memory.
372. Проблема: per-frame disk I/O can be hundreds of MB/s.
373. Улучшение: use app-group shared container only as transition.
374. Улучшение: use IOSurface/shared memory for production.
375. Проблема: transport schema is custom.
376. Улучшение: keep header tiny and versioned.
377. Сделано: header has magic and version.
378. Проблема: no backward compatibility for v1/v2 transport.
379. Улучшение: parse old version for smoother dev upgrades.
380. Проблема: extension silently falls back when transport invalid.
381. Улучшение: rate-limit diagnostics for invalid transport.
382. Проблема: fallback placeholder may hide app failure.
383. Улучшение: encode "source is fallback" visually in placeholder.
384. Проблема: virtual output can appear alive while app is dead.
385. Улучшение: include stale-frame age in extension logic.
386. Проблема: output fps and transport fps may drift.
387. Сделано: transport now carries fps.
388. Улучшение: extension should smooth fps changes.
389. Проблема: clients may expect fixed stream format.
390. Улучшение: keep format fixed, only cadence changes.
391. Проблема: no integration test with real CoreMediaIO client.
392. Улучшение: test with OBS/QuickTime/FaceTime after signing.
393. Проблема: system extension is not listed without valid install.
394. Улучшение: add release checklist for System Settings approval.
395. Проблема: ad-hoc signing cannot install extension.
396. Сделано: docs now state that clearly.
397. Проблема: Apple Development cert without provisioning profile gets killed.
398. Сделано: this was verified through AMFI `No matching profile found`.
399. Сделано: bundler copies an embedded provisioning profile when provided.
400. Сделано: bundler leaves restricted entitlement out when no profile is provided.
401. Сделано: bundler no longer creates an unlaunchable entitlement-bearing app from identity alone.
402. Сделано: added `CAMERAMAN_PROVISIONING_PROFILE` / `PROVISIONING_PROFILE` env support.
403. Улучшение: verify profile contains bundle id.
404. Улучшение: verify profile contains system-extension entitlement.
405. Улучшение: verify Team ID matches signing identity.
406. Проблема: extension entitlements may need more than sandbox for production.
407. Улучшение: audit CMIO extension entitlement requirements.
408. Проблема: Mach service name may need Team ID prefix in signed builds.
409. Улучшение: generate mach service name from Team ID config.
410. Проблема: app group is not configured.
411. Улучшение: add app group only after provisioning exists.
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
438. Сделано: recommendations hold exactly 500 items.
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
475. Проблема: no panic policy documented.
476. Улучшение: document no panic across FFI boundaries.
477. Проблема: no unsafe audit comments for every block.
478. Улучшение: add focused safety comments in extension code.
479. Проблема: too many manual Objective-C method signatures.
480. Улучшение: wrap each protocol implementation in smaller module.
481. Проблема: app and extension share transport through filesystem only.
482. Улучшение: abstract transport trait for future shared memory.
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
500. Следующий шаг: validate provisioning profile contents and disable impossible install states.

## Review Summary

The active code review after this rewrite should focus on signing/provisioning, stale documentation, app UI state, and unsafe extension boundaries. Keep the numbered backlog above at exactly 500 items; add future detailed review notes here without extra numbered entries.

## Second Review Pass

Ran a 15-agent adversarial review targeted at exactly the four areas above, plus a design-critique pass on the current egui UI (via the design-critique skill) and a direct check of this machine's signing state. Every claimed finding below was independently re-verified by a second agent reading the source directly before being accepted.

Confirmed and fixed:

- `extension_main.rs`: `stop_streaming` only flipped a bool; `start_streaming` could race ahead and spawn a second `stream_samples` thread before the first noticed and exited, leaving two threads sending samples to the same `CMIOExtensionStream` concurrently. Fixed by having `start_streaming` join the previous worker thread before spawning a replacement.
- `app.rs`: a permanently broken camera in a multi-camera composite re-posted its error every tick, which reset the message's TTL every time and both blocked any other status-bar message forever and never engaged the auto-stop safety net (that only ever saw the all-cameras-failing case). Fixed with a per-source failure streak that posts once and auto-drops the camera after `MAX_CAPTURE_ERROR_STREAK`.
- `app.rs`: if every selected real camera disappeared while running, the status bar kept showing "Preview running" in green with nothing being captured. Fixed: this path now stops the preview and posts an explicit event.
- `main.rs` / `system_extension.rs`: the extension bundle id was still duplicated across the `Info.plist` template and the `.systemextension` path despite `architecture.md` already listing this as an open DRY violation from an earlier pass; unified behind `EXTENSION_BUNDLE_ID`.
- `app.rs`: the "Install extension" button stayed clickable even on an ad-hoc-signed bundle where activation is guaranteed to fail. It is now gated on `extension_capable()`, which checks both that the process runs from an installed `.app` bundle and that the bundle's own entitlements (read via `codesign`) actually grant `system-extension.install`.
- `app.rs`: the control column had no scroll area, so the Output/System Extension controls were clipped at the bottom even at the default window size. Wrapped in `egui::ScrollArea::vertical()`.
- `app.rs`: the install-extension button and status shared no visual grouping with the section above it. Gave it its own "System Extension" label and separator.
- `architecture.md`: test count claimed 20, actual is 22; bundle-id duplication claimed as an open "Still open" item after it had already been fixed elsewhere in this same pass; Module Map for `system_extension.rs` omitted the `EXTENSION_BUNDLE_ID` constant; the install-extension-button design issue described the pre-fix behavior. All corrected.
- `README.md`: the install-extension-button description and "Current UI gaps" list described the pre-fix behavior. Corrected.

Checked and confirmed NOT a bug (no action taken):

- `extension_main.rs`'s `stream_samples` divides by the frame-spool-reported fps; verified the value is always clamped to at least 1 both where it's parsed (`frame_transport.rs`) and where it's read (`FrameSpoolReader::poll`), so no division-by-zero path exists.
- The frame-spool's temp-file-then-rename write strategy was verified race-safe against the extension's concurrent reads: POSIX rename is atomic, so a reader only ever sees a complete pre- or post-rename file, never a torn one.

Still open (see architecture.md's per-iteration "Still open" lists for the full picture):

- A real camera whose `open()` call itself hangs is indistinguishable from one merely warming up, so it never counts toward any failure streak. Needs a bounded, generous open-timeout in the capture worker.
- `ExtensionActivationStatus::Requesting` and `Idle` render identically (both dim gray), so an in-flight activation request gives no visual feedback that anything is happening.
- Fixed fps mode gives no feedback when the chosen rate exceeds what the camera can actually deliver.
- Client authorization in the CMIO extension is logged but not actually enforced (any local process can still connect).
- Real system-extension install still needs a paid Apple Developer Program membership with the System Extension capability; the free personal-team certificate now present on this machine (`Apple Development: d.o.mezhov@gmail.com`, team `VBA8KCMNX7`) can sign a plain app but cannot carry that capability.

# CameraMan Rust-Only Recommendations

Ниже 500 пунктов для нового Rust-only направления. Старый mixed-language подход больше не является архитектурой проекта.

## Итерация 1: чистое Rust-ядро

1. Сделано: non-Rust слой удален из основной структуры.
2. Сделано: `Cargo.toml` теперь описывает библиотеку и бинарник.
3. Сделано: `src/lib.rs` экспортирует публичные Rust-модули.
4. Сделано: `src/main.rs` перестал быть `Hello, world!`.
5. Сделано: добавлен `VideoFormat`.
6. Сделано: добавлен `VirtualCameraConfig`.
7. Сделано: добавлен единый `CameraManError`.
8. Сделано: добавлен `PixelFormat`.
9. Сделано: добавлен валидируемый `Frame`.
10. Сделано: добавлен `CompositionLayout`.
11. Сделано: добавлен `GridLayoutCalculator`.
12. Сделано: добавлен чистый Rust `Compositor`.
13. Сделано: добавлен trait `FrameSource`.
14. Сделано: добавлен trait `VirtualCameraSink`.
15. Сделано: добавлен `PipelineEngine`.
16. Сделано: добавлен synthetic source для демо и тестов.
17. Сделано: добавлен memory sink для тестируемой границы.
18. Сделано: добавлен unsupported sink как честная заглушка backend.
19. Сделано: добавлен CLI status output.
20. Сделано: добавлен `--demo`.
21. Сделано: demo пишет PPM в `target/`.
22. Сделано: тесты покрывают frame validation.
23. Сделано: тесты покрывают layout calculation.
24. Сделано: тесты покрывают composition.
25. Сделано: `cargo test` проходит.
26. Улучшение: добавить тесты на 0 камер.
27. Улучшение: добавить тесты на 1 камеру.
28. Улучшение: добавить тесты на 2 камеры.
29. Улучшение: добавить тесты на 3 камеры.
30. Улучшение: добавить тесты на 4 камеры.
31. Улучшение: добавить тесты на 5 камер.
32. Улучшение: добавить тесты на 8 камер.
33. Улучшение: добавить тесты на row layout.
34. Улучшение: добавить тесты на column layout.
35. Улучшение: добавить тесты на grid layout.
36. Улучшение: добавить тесты на remainder pixels.
37. Улучшение: добавить тесты на invalid dimensions.
38. Улучшение: добавить тесты на invalid buffer length.
39. Улучшение: добавить тесты на unsupported pixel format.
40. Улучшение: добавить тесты на empty input.
41. Улучшение: добавить тест на aspect fit для tall frame.
42. Улучшение: добавить тест на aspect fit для wide frame.
43. Улучшение: добавить тест на centered paste.
44. Улучшение: добавить тест на empty cell color.
45. Улучшение: добавить тест на background color.
46. Улучшение: добавить property tests для layout.
47. Улучшение: добавить fuzz target для `Frame::new_checked`.
48. Улучшение: добавить golden PPM для demo.
49. Улучшение: добавить snapshot-style тесты без внешних crate.
50. Улучшение: добавить benchmark для compositor.
51. Проблема: compositor сейчас nearest-neighbor.
52. Улучшение: добавить bilinear scaling.
53. Проблема: compositor копирует пиксели без SIMD.
54. Улучшение: позже добавить SIMD path.
55. Сделано: source frames теперь имеют timestamp в `FrameMetadata`.
56. Сделано: добавлен `FrameMetadata`.
57. Сделано: `Frame` отделен от captured metadata.
58. Сделано: добавлен `CapturedFrame`.
59. Сделано: synthetic sources ведут sequence number.
60. Улучшение: добавить monotonic frame index для реальных camera sources.
61. Проблема: нет dropped-frame метрик.
62. Улучшение: добавить `PipelineMetrics`.
63. Проблема: нет render duration.
64. Улучшение: измерять time per compose.
65. Проблема: нет source duration.
66. Улучшение: измерять time per source read.
67. Проблема: нет sink duration.
68. Улучшение: измерять time per sink send.
69. Проблема: нет backpressure model.
70. Улучшение: добавить frame dropping strategy.
71. Проблема: `PipelineEngine` сейчас делает один tick.
72. Улучшение: добавить render loop.
73. Проблема: render loop не должен блокировать UI.
74. Улучшение: вынести loop в отдельный worker.
75. Проблема: нет lifecycle state.
76. Улучшение: добавить `PipelineState`.
77. Проблема: нет typed start report.
78. Улучшение: добавить `StartPipelineReport`.
79. Проблема: нет typed stop report.
80. Улучшение: добавить `StopPipelineReport`.
81. Проблема: нет source status.
82. Улучшение: добавить per-source status.
83. Проблема: нет sink status.
84. Улучшение: добавить virtual camera status.
85. Проблема: нет config validation.
86. Улучшение: добавить `VideoFormat::validate`.
87. Проблема: fps может быть 0.
88. Улучшение: запретить zero fps.
89. Проблема: dimensions могут быть слишком большими.
90. Улучшение: добавить max dimensions.
91. Проблема: byte length conversion использует `as usize`.
92. Улучшение: сделать checked conversion везде.
93. Проблема: ошибки не имеют кодов.
94. Улучшение: добавить стабильные error codes.
95. Проблема: ошибки не имеют recovery action.
96. Улучшение: добавить `recovery_hint`.
97. Проблема: CLI пока только status/demo.
98. Улучшение: добавить `--format`.
99. Улучшение: добавить `--layout`.
100. Улучшение: добавить `--output`.

## Итерация 2: ввод, вывод и platform backend

101. Сделано: реальная камера подключена через Rust crate `nokhwa`.
102. Сделано: выбран Rust-native strategy для camera capture.
103. Сделано: macOS capture adapter идёт через AVFoundation backend nokhwa.
104. Сделано: реализован `FrameSource` для real camera.
105. Сделано: device discovery добавлен.
106. Сделано: реализован `CameraDiscovery` для nokhwa.
107. Проблема: permission handling всё ещё системный и зависит от macOS.
108. Сделано: добавлен Rust app bundle с `NSCameraUsageDescription`.
109. Проблема: camera permissions platform-specific.
110. Улучшение: держать permissions вне renderer.
111. Проблема: нет автоматического горячего подключения камер.
112. Сделано: добавлен ручной device refresh в app.
113. Проблема: нет camera IDs в pipeline.
114. Улучшение: добавить source IDs.
115. Проблема: нет camera names на frame metadata.
116. Улучшение: добавить display name.
117. Проблема: нет ordered selection.
118. Улучшение: добавить `CameraSelection`.
119. Проблема: нет reorder.
120. Улучшение: добавить ordered vector with validation.
121. Проблема: нет persistent config.
122. Улучшение: добавить TOML config позже.
123. Проблема: нет user profile.
124. Улучшение: добавить saved layouts.
125. Проблема: нет dynamic layout change.
126. Улучшение: разрешить менять layout во время stream.
127. Проблема: нет dynamic source add.
128. Улучшение: добавить `PipelineEngine::update_sources`.
129. Проблема: нет dynamic source remove.
130. Улучшение: добавить graceful source removal.
131. Проблема: нет reconnect policy.
132. Улучшение: добавить retry with backoff.
133. Проблема: нет stale frame detection.
134. Улучшение: помечать старые кадры.
135. Проблема: nil frame сейчас просто empty cell.
136. Улучшение: добавить source placeholder.
137. Проблема: placeholder без текста.
138. Улучшение: добавить text rendering позже.
139. Проблема: text rendering потребует font strategy.
140. Улучшение: выбрать Rust text renderer.
141. Проблема: нет mirroring.
142. Улучшение: добавить mirror transform.
143. Проблема: нет rotation.
144. Улучшение: добавить rotate transform.
145. Проблема: нет crop.
146. Улучшение: добавить fit/fill/crop modes.
147. Проблема: нет gutters.
148. Улучшение: добавить configurable gutters.
149. Проблема: нет borders.
150. Улучшение: добавить border rendering.
151. Проблема: нет labels.
152. Улучшение: добавить source labels.
153. Проблема: нет primary camera mode.
154. Улучшение: добавить picture-in-picture.
155. Проблема: нет smart layouts.
156. Улучшение: добавить layouts for 5, 7, 8 sources.
157. Проблема: нет output format switching.
158. Улучшение: добавить 720p, 1080p, 4K presets.
159. Проблема: нет fps switching.
160. Улучшение: добавить 15, 30, 60 fps presets.
161. Проблема: macOS virtual camera backend не реализован.
162. Улучшение: реализовать backend за `VirtualCameraSink`.
163. Проблема: CoreMediaIO backend будет unsafe-heavy.
164. Улучшение: изолировать unsafe в одном модуле.
165. Проблема: FFI может протекать в core.
166. Улучшение: не экспортировать platform types из core.
167. Проблема: sample buffer ownership сложен.
168. Улучшение: написать wrapper с Drop.
169. Проблема: queue ownership сложен.
170. Улучшение: написать sink queue wrapper.
171. Проблема: timestamps должны быть host-time compatible.
172. Улучшение: добавить `Timestamp` abstraction.
173. Проблема: backend errors будут OSStatus.
174. Улучшение: конвертировать OSStatus в `CameraManError`.
175. Проблема: system extension packaging сложен.
176. Улучшение: сначала сделать backend proof-of-concept.
177. Проблема: pure Rust system extension может требовать Objective-C runtime bindings.
178. Улучшение: исследовать `objc2` ecosystem.
179. Проблема: release signing не должен быть в core.
180. Улучшение: вынести packaging в scripts.
181. Проблема: `UnsupportedVirtualCameraSink` может быть забыт.
182. Улучшение: пометить его как temporary.
183. Сделано: есть `pipeline-demo` через `PipelineEngine`.
184. Сделано: добавлен `PpmSequenceSink`.
185. Проблема: `MemorySink` не потокобезопасен.
186. Улучшение: добавить shared test sink при необходимости.
187. Сделано: добавлены tests для pipeline ticks.
188. Улучшение: добавить отдельные integration tests в `tests/`.
189. Проблема: нет stress test.
190. Улучшение: прогнать 1000 compose ticks.
191. Проблема: нет memory profile.
192. Улучшение: измерить allocations per frame.
193. Проблема: каждый output allocates.
194. Улучшение: добавить frame buffer reuse.
195. Проблема: нет buffer pool.
196. Улучшение: добавить Rust `FramePool`.
197. Проблема: нет zero-copy path.
198. Улучшение: изучить zero-copy only after correctness.
199. Проблема: нет bounded queue.
200. Улучшение: добавить channel strategy.

## Итерация 3: Rust UI, дизайн и developer experience

201. Сделано: GUI больше не отсутствует, добавлен Rust desktop app.
202. Сделано: выбран Rust GUI stack `egui`/`eframe`.
203. Сделано: `egui`/`eframe` подключен как зависимость.
204. Решение: `iced` пока не нужен, чтобы не плодить UI-стеки.
205. Решение: `tao` + custom renderer пока не нужен.
206. Сделано: GUI не лезет в pixel internals напрямую.
207. Сделано: GUI работает через core frame/layout/render APIs.
208. Сделано: preview загружается в egui texture.
209. Сделано: добавлен preview frame conversion boundary.
210. Проблема: preview может перегружать CPU при росте разрешения.
211. Сделано: preview repaint ограничен timer logic.
212. Сделано: Start/Stop кнопка имеет стабильный размер.
213. Сделано: основные кнопки имеют стабильные размеры.
214. Проблема: настройки всё ещё могут перегрузить первый экран.
215. Сделано: первый экран минимален: sources, layout, preview, status.
216. Сделано: появилась понятная визуальная иерархия.
217. Сделано: primary action только Start/Stop.
218. Решение: Render выступает secondary action.
219. Решение: Settings пока не добавлен, чтобы не раздувать UI.
220. Улучшение: dangerous actions hidden or confirmed when they appear.
221. Проблема: empty states пока базовые.
222. Улучшение: добавить no cameras state для реального capture backend.
223. Улучшение: добавить no permission state для реального capture backend.
224. Улучшение: добавить backend missing state.
225. Улучшение: добавить streaming state.
226. Улучшение: добавить stopped state.
227. Проблема: нет status bar.
228. Улучшение: добавить fps.
229. Улучшение: добавить dropped frames.
230. Улучшение: добавить backend status.
231. Улучшение: добавить selected source count.
232. Проблема: нет diagnostics panel.
233. Улучшение: добавить collapsible diagnostics.
234. Проблема: нет copy diagnostics.
235. Улучшение: добавить copy diagnostics action.
236. Проблема: нет logs.
237. Улучшение: добавить `tracing`.
238. Проблема: нет log levels.
239. Улучшение: добавить env-based log filter.
240. Проблема: нет file logs.
241. Улучшение: добавить optional file logging.
242. Проблема: нет crash reports.
243. Улучшение: добавить panic hook.
244. Проблема: нет CLI help.
245. Улучшение: добавить clap later.
246. Проблема: нет stable commands.
247. Улучшение: добавить `demo`, `check`, `list-devices`.
248. Проблема: нет config file.
249. Улучшение: добавить config after UI decisions.
250. Проблема: нет docs for module boundaries.
251. Улучшение: добавить rustdoc module docs.
252. Проблема: публичные exports могут разрастись.
253. Улучшение: экспортировать только stable API.
254. Проблема: нет crate-level docs.
255. Улучшение: добавить docs in `lib.rs`.
256. Проблема: нет examples folder.
257. Улучшение: добавить `examples/render_demo.rs`.
258. Проблема: demo logic в main.
259. Улучшение: вынести demo helper later.
260. Проблема: нет lint policy.
261. Улучшение: добавить clippy.
262. Улучшение: запускать `cargo clippy -- -D warnings`.
263. Проблема: нет formatting check.
264. Улучшение: добавить `cargo fmt --check`.
265. Проблема: нет CI.
266. Улучшение: добавить GitHub Actions later.
267. Проблема: нет MSRV.
268. Улучшение: определить minimum Rust version.
269. Проблема: edition 2024 может требовать свежий toolchain.
270. Улучшение: указать toolchain в README.
271. Проблема: нет rust-toolchain.toml.
272. Улучшение: добавить pin only if needed.
273. Проблема: нет license.
274. Улучшение: добавить LICENSE.
275. Проблема: нет changelog.
276. Улучшение: добавить CHANGELOG before first release.
277. Проблема: нет roadmap.
278. Улучшение: держать roadmap in docs.
279. Проблема: нет ADR.
280. Улучшение: добавить ADR for Rust-only decision.
281. Проблема: нет explicit non-goals.
282. Улучшение: указать, что non-Rust source code не используется.
283. Проблема: нет safety policy for unsafe.
284. Улучшение: все unsafe только в platform modules.
285. Проблема: нет review checklist.
286. Улучшение: добавить checklist для unsafe PR.
287. Проблема: нет performance budget.
288. Улучшение: target 30 fps at 1080p.
289. Проблема: нет latency budget.
290. Улучшение: target below one frame of latency where possible.
291. Проблема: нет memory budget.
292. Улучшение: cap allocations per render tick.
293. Проблема: нет error taxonomy.
294. Улучшение: разделить config, capture, render, sink, platform errors.
295. Проблема: нет recovery taxonomy.
296. Улучшение: добавить user action per recoverable error.
297. Проблема: нет test data.
298. Улучшение: добавить synthetic fixtures.
299. Проблема: нет visual fixtures.
300. Улучшение: добавить generated PPM fixtures.

## Итерация 4: качество, производительность и релиз

301. Проблема: CPU compositor может быть медленным.
302. Улучшение: сначала измерить, потом оптимизировать.
303. Проблема: SIMD premature может усложнить код.
304. Улучшение: держать scalar reference implementation.
305. Проблема: GPU path может понадобиться.
306. Улучшение: добавить GPU only after Rust core stable.
307. Проблема: нет color management.
308. Улучшение: явно документировать BGRA/sRGB assumptions.
309. Проблема: alpha пока всегда 255.
310. Улучшение: определить alpha policy.
311. Проблема: no premultiplied alpha policy.
312. Улучшение: зафиксировать non-premultiplied or premultiplied.
313. Проблема: нет stride abstraction.
314. Улучшение: добавить stride when backend requires it.
315. Проблема: current frame assumes packed pixels.
316. Улучшение: keep packed core, convert at boundaries.
317. Проблема: нет planar pixel formats.
318. Улучшение: не добавлять planar formats до необходимости.
319. Проблема: YUV может понадобиться для backend.
320. Улучшение: добавить converter later.
321. Проблема: no timestamp monotonicity test.
322. Улучшение: добавить timestamp tests with fake clock.
323. Проблема: no fake clock.
324. Улучшение: добавить `Clock` trait if needed.
325. Проблема: no render scheduler.
326. Улучшение: добавить scheduler independent from UI.
327. Проблема: no cancellation token.
328. Улучшение: добавить shutdown signal.
329. Проблема: no worker thread ownership model.
330. Улучшение: document thread ownership.
331. Проблема: no Send/Sync audit.
332. Улучшение: проверить trait bounds for threaded pipeline.
333. Проблема: trait objects may hide Send requirement.
334. Улучшение: require `FrameSource: Send` once threaded.
335. Проблема: sink may need Send.
336. Улучшение: require `VirtualCameraSink: Send` once threaded.
337. Проблема: Frame clone can be expensive.
338. Улучшение: introduce Arc-backed buffers if needed.
339. Проблема: MemorySink clones whole frames.
340. Улучшение: acceptable only for tests.
341. Проблема: no allocation stats.
342. Улучшение: add benchmark with allocation counters later.
343. Проблема: no binary size awareness.
344. Улучшение: measure after GUI/backend dependencies.
345. Проблема: no dependency policy.
346. Улучшение: prefer small, maintained crates.
347. Проблема: no security audit.
348. Улучшение: run cargo audit later.
349. Проблема: no supply-chain policy.
350. Улучшение: pin release dependencies.
351. Проблема: no macOS packaging.
352. Улучшение: decide app bundle strategy for Rust GUI.
353. Проблема: no signing docs.
354. Улучшение: document Developer ID flow.
355. Проблема: no notarization docs.
356. Улучшение: add notarization only when app bundle exists.
357. Проблема: no uninstall docs.
358. Улучшение: document virtual camera cleanup.
359. Проблема: no compatibility matrix.
360. Улучшение: track macOS versions.
361. Проблема: no Apple Silicon/Intel policy.
362. Улучшение: decide universal binary support.
363. Проблема: no CI macOS runner.
364. Улучшение: add macOS CI for pure core.
365. Проблема: virtual camera CI hard.
366. Улучшение: keep backend tests separated.
367. Проблема: no manual QA checklist.
368. Улучшение: add checklist for real device testing.
369. Проблема: no OBS test plan.
370. Улучшение: include OBS in manual QA.
371. Проблема: no browser test plan.
372. Улучшение: include browser camera picker in QA.
373. Проблема: no Zoom test plan.
374. Улучшение: include Zoom only if available.
375. Проблема: no permission reset guide.
376. Улучшение: document macOS privacy reset when needed.
377. Проблема: no backend crash isolation.
378. Улучшение: isolate platform backend errors.
379. Проблема: no UI/backend separation doc.
380. Улучшение: keep architecture doc updated.
381. Проблема: recommendation file can become stale.
382. Улучшение: refresh after each major iteration.
383. Проблема: README can promise too much.
384. Улучшение: keep current status honest.
385. Проблема: demo output is PPM.
386. Улучшение: later support PNG via dependency.
387. Проблема: no image dependency yet.
388. Улучшение: add one only when needed.
389. Проблема: no audio support.
390. Улучшение: explicitly keep project video-only for now.
391. Проблема: no multi-output support.
392. Улучшение: one virtual camera first.
393. Проблема: no multiple profile support.
394. Улучшение: postpone profiles.
395. Проблема: no plugin system.
396. Улучшение: do not add plugins yet.
397. Проблема: no remote streaming.
398. Улучшение: keep local-only.
399. Проблема: no privacy statement.
400. Улучшение: document that frames stay local.

## Итерация 5: порядок маленьких PR

401. PR 1: keep Rust-only cleanup.
402. PR 2: add more compositor tests.
403. PR 3: add pipeline integration test.
404. PR 4: add `FrameMetadata`.
405. PR 5: add `CapturedFrame`.
406. PR 6: add metrics.
407. PR 7: add render loop.
408. PR 8: add frame pool.
409. PR 9: add CLI subcommands.
410. PR 10: add clippy config.
411. PR 11: add rustdoc.
412. PR 12: add examples.
413. PR 13: add camera discovery trait refinement.
414. PR 14: done, research macOS capture backend.
415. PR 15: done, prototype camera capture.
416. PR 16: done, add real camera list command.
417. PR 17: done, add real camera frame capture with timeout.
418. PR 18: done, add preview file output from real camera path.
419. PR 19: research virtual camera backend.
420. PR 20: isolate unsafe backend wrappers.
421. PR 21: prototype CoreMediaIO device discovery in Rust.
422. PR 22: prototype sink connection in Rust.
423. PR 23: send one synthetic frame to virtual sink.
424. PR 24: send render loop frames to virtual sink.
425. PR 25: add backend diagnostics.
426. PR 26: add backend error mapping.
427. PR 27: add manual QA docs.
428. PR 28: done, choose `egui`/`eframe`.
429. PR 29: done, create basic GUI shell.
430. PR 30: done, show synthetic source list.
431. PR 31: done, show preview.
432. PR 32: done, add layout controls.
433. PR 33: done, add start/stop controls.
434. PR 34: done, add status bar.
435. PR 35: add settings after real backend exists.
436. PR 36: add persistence.
437. PR 37: add packaging.
438. PR 38: add signing docs.
439. PR 39: add release checklist.
440. PR 40: prepare alpha.
441. Правило: один PR должен менять одну архитектурную идею.
442. Правило: renderer PR должен иметь visual or pixel tests.
443. Правило: backend PR должен иметь safety notes.
444. Правило: UI PR должен иметь screenshot or demo note.
445. Правило: docs PR должен remove stale promises.
446. Правило: no platform code inside renderer.
447. Правило: no UI code inside backend.
448. Правило: no unsafe outside backend modules.
449. Правило: no global mutable state.
450. Правило: no hidden blocking on UI thread.
451. Правило: prefer explicit config.
452. Правило: prefer small structs.
453. Правило: prefer typed errors.
454. Правило: prefer testable pure functions.
455. Правило: prefer traits at platform boundaries.
456. Правило: avoid abstraction before second implementation.
457. Правило: benchmark before optimization.
458. Правило: document every unsafe block.
459. Правило: keep CLI useful for debugging.
460. Правило: keep GUI thin now that core works.
461. Правило: keep README honest.
462. Правило: keep architecture doc short enough to read.
463. Правило: keep recommendation list as backlog, not law.
464. Правило: remove dead code quickly.
465. Правило: do not reintroduce non-Rust source files.
466. Правило: generated assets are okay if documented.
467. Правило: platform metadata files are okay only for packaging.
468. Правило: Rust code remains source of behavior.
469. Правило: docs should say what is implemented now.
470. Правило: docs should separate plan from reality.
471. Учиться: start with `src/frame.rs`.
472. Учиться: then read `src/layout.rs`.
473. Учиться: then read `src/render.rs`.
474. Учиться: then read `src/camera.rs`.
475. Учиться: then read `src/virtual_camera.rs`.
476. Учиться: then read `src/pipeline.rs`.
477. Учиться: then read `src/main.rs`.
478. Учиться: run `cargo test`.
479. Учиться: run `cargo run`.
480. Учиться: run `cargo run -- demo`.
481. Учиться: inspect `target/camera-man-demo.ppm`.
482. Учиться: change synthetic colors.
483. Учиться: change layout.
484. Учиться: add a fourth synthetic source.
485. Учиться: add a compositor test.
486. Учиться: add a pipeline test.
487. Учиться: add a new layout.
488. Учиться: add a simple border.
489. Учиться: add metrics.
490. Учиться: add CLI arg parsing.
491. Следующий шаг: add external integration test in `tests/`.
492. Следующий шаг: add app-level integration checks.
493. Следующий шаг: add real render loop backend.
494. Следующий шаг: test bundled app camera permission flow.
495. Следующий шаг: research Rust CoreMediaIO backend.
496. Следующий шаг: replace synthetic app sources with Rust capture.
497. Следующий шаг: keep tests green.
498. Следующий шаг: keep docs synced.
499. Следующий шаг: make small commits.
500. Следующий шаг: build the real Rust backend behind the existing traits.

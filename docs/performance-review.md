# Performance Review: 2026-07-19

Этот документ отделяет измеренные узкие места от гипотез и описывает вариант
Rust-only архитектуры, если начинать CameraMan заново. Он не заменяет
`docs/acceptance.md`: локальный быстрый прогон не является доказательством
восьмичасовой стабильности или работы подписанного CoreMediaIO extension с
реальной камерой.

## Контекст измерений

- Машина: Apple M4 Max, `aarch64-apple-darwin`, питание от сети.
- Compositor: release build, 1920x1080, четыре источника, Grid, 10 warm-up и
  100 измеряемых итераций для финального прогона.
- E2E: representative patterned fixture, bilinear, compose -> mmap publish ->
  отдельный reader process acknowledgement, 10 warm-up и 100 итераций.
- В E2E не входят физическая камера, CoreVideo upload, CMIO delivery и
  отображение/кодирование сторонним приложением.
- Первый диагностический transform-прогон до исправления имел 20 итераций,
  поэтому коэффициенты ускорения ниже являются направленным сравнением, а не
  новым release baseline. Сохранённый AC baseline меняется только полным
  process-level runner.

Воспроизведение:

```bash
CAMERAMAN_BENCH_MODE=transforms \
CAMERAMAN_BENCH_ITERATIONS=100 \
CAMERAMAN_BENCH_WARMUP_ITERATIONS=10 \
  cargo bench --locked --bench compositor --no-default-features

cargo run --release --locked --bin cameraman-e2e-benchmark \
  --no-default-features -- --iterations 100 --warmups 10
```

## Что было медленным

Любая неидентичная трансформация раньше уходила в универсальный per-pixel
float path. Он заново вычислял координаты и цветовой/alpha pipeline для каждого
пикселя. В результате обычный bilinear Fill занимал больше трёх кадрового
бюджета 60 FPS.

| Сценарий 1080p/4/Grid | До, avg ms | После, avg ms | После, p95 ms | Ускорение |
|---|---:|---:|---:|---:|
| Nearest identity | 1.042 | 0.973 | 1.024 | 1.07x |
| Nearest Fill | 19.870 | 1.741 | 1.826 | 11.41x |
| Nearest crop+rotate+mirror | 10.650 | 1.273 | 1.375 | 8.37x |
| Nearest opacity 85% | 14.955 | 2.740 | 2.935 | 5.46x |
| Bilinear identity | 5.841 | 6.039 | 6.207 | шум измерения |
| Bilinear Fill | 63.141 | 7.759 | 7.929 | 8.14x |
| Bilinear crop+rotate+mirror | 30.360 | 5.806 | 6.115 | 5.23x |
| Bilinear opacity 85% | 47.625 | 6.871 | 7.373 | 6.93x |

Финальный худший p95 в этой матрице равен 7.929 ms и укладывается в бюджет
16.67 ms для 60 FPS. Это бюджет только compositor, не всей системы.

## Сквозной профиль

Финальный отдельный producer/consumer прогон дал:

| Метрика | Результат |
|---|---:|
| Throughput без искусственного pacing | 159.53 frame/s |
| Compose p95 | 6.254 ms |
| mmap publish p95 | 0.229 ms |
| Cross-process acknowledgement wait p95 | 0.005 ms |
| Fixture-ready -> extension-input acknowledgement p95 | 6.443 ms |
| Рост RSS producer за короткий measured interval | 48 KiB |

Ledger показывает две полнокадровые записи до входа extension:

- composition output: 8,294,400 bytes на кадр;
- shared-memory publish: 8,294,400 bytes на кадр.

Это около 474.6 MiB/s памяти при 30 FPS и 949.2 MiB/s при 60 FPS. Extension
добавляет ещё один BGRA upload в `CVPixelBuffer`, поэтому полный текущий путь
имеет не менее трёх 1080p проходов: около 711.9 MiB/s при 30 FPS или 1.39 GiB/s
при 60 FPS, ещё до capture decode и preview upload.

## Исправлено в этом проходе

- Для opaque/full-range BT.709 добавлен специализированный transform path с
  integer nearest/bilinear/opacity и кэшированными O(width+height) coordinate
  maps. Универсальный path остался oracle/fallback для сложных контрактов.
- Полная очистка output выполняется только для реально непокрытой области.
  Opaque Fill и совпадающий aspect больше не получают лишний cell prefill.
- Одинаковый captured frame с неизменной сценой не компонуется повторно. При
  source 30 FPS и output 60 FPS CMIO повторяет уже готовый buffer.
- Nokhwa decode теперь получает RGBA и меняет R/B in-place вместо выделения
  второго BGRA `Vec`.
- Extension ждёт deadline до transport poll/CV upload, поэтому свежий кадр не
  лежит готовым ещё один полный frame interval перед отправкой.
- `CVPixelBufferPool` ограничен шестью outstanding buffers. Медленный consumer
  получает повтор последнего кадра и drop telemetry вместо неограниченного
  роста примерно по 8.29 MB на allocation.
- Shared reader ограничивает дорогие reopen и PID/start-token проверки, но
  продолжает читать атомарный latest-frame header каждый tick.
- File fallback проверяет inode/size/mtime и не читает неизменившийся
  многомегабайтный spool заново.
- Preview хранит только одну актуальную coordinate map вместо неограниченного
  кэша ширин, накопленных при resize окна.
- Однотонные synthetic источники уменьшены с 320x240 до логических 4x3 с тем же
  aspect и тем же визуальным результатом.

## Оставшиеся проблемы

| Приоритет | Проблема | Почему важно | Следующее доказательство |
|---|---|---|---|
| P0 | Нет физического camera -> signed CMIO -> third-party consumer профиля | Текущий E2E заканчивается перед самым platform-specific участком | Instruments/signposts на FaceTime/OBS/Zoom с paid profile |
| P0 | Capture выбирает `AbsoluteHighestFrameRate` | Nokhwa затем выбирает самое большое разрешение среди max-FPS форматов; 4K60 может декодироваться ради 1080p30 | Перечислить реальные форматы и выбрать минимальный формат, покрывающий output |
| P0 | `nokhwa::Camera::frame()` не отменяется | Зависший driver удерживает thread/device и мешает быстрому restart | Direct AVFoundation adapter с контролируемой session queue и stop contract |
| P0 | mmap publish и CoreVideo upload остаются полными копиями | Это основной гарантированный bandwidth floor | IOSurface-backed prototype без CPU readback, затем реальный signed test |
| P1 | UI repaint остаётся producer clock | Свёрнутое или перегруженное UI может нерегулярно создавать render jobs | Capture/config-driven runtime thread; UI только наблюдатель |
| P1 | Preview делает BGRA -> `ColorImage` и texture upload | До 2.07 MB CPU conversion плюс GPU upload на preview frame | Отдельный preview benchmark и GPU texture bridge |
| P1 | Diagnostics span всегда делает tracing, signpost и mutex histogram write | Стоимость мала относительно resize, но пока не изолирована | Microbenchmark enabled/disabled и sampling policy при необходимости |
| P1 | Sampling cache защищён `Arc<Mutex<_>>` на весь paste | Один worker не конкурирует, но платит lock и усложняет ownership | Worker-owned mutable compositor/cache |
| P2 | Cache очищает все 64 maps при переполнении | При частом переборе сцен возможны burst rebuilds | Малый LRU или generation-local cache после профильного сценария |
| P2 | File transport всё равно sync/rename полного кадра | Это диагностический degraded mode, не production data plane | Оставить только явным fallback и показывать warning |
| P2 | `block 0.1.6` future-incompatible | Это риск обновления toolchain, не текущий hot path | Удалить через обновление/замену upstream dependency |
| Gate | Не выполнен полный 8-hour soak | Короткий RSS delta не доказывает отсутствие медленного роста | AC soak с RSS/footprint/wakeups/energy и сохранённым отчётом |

## Если начинать с нуля

Я бы оставил приложение полностью на Rust, но поменял data plane. Главная цель:
один IOSurface-backed output должен одновременно служить compositor, preview и
CMIO, а между процессами должны передаваться ownership/handle metadata, не
8.29 MB пикселей на каждый кадр.

```text
cameraman-types
  Dimensions, Fps, ColorContract, FrameId, SurfaceId, timestamps

cameraman-capture-avfoundation
  device/format selection, CVPixelBuffer ingress, cancellation, health

cameraman-runtime
  latest-only source slots, scene reducer, clock domains, backpressure

cameraman-compositor-cpu
  deterministic scalar oracle and recovery fallback

cameraman-compositor-metal
  CVMetalTextureCache input, scene plan, IOSurface-backed output

cameraman-surface-pool
  bounded triple/quad buffer ownership and lifetime state machine

cameraman-ipc
  authorized Mach/XPC surface handoff plus versioned metadata protocol

cameraman-cmio
  deadline pacing, last-frame repeat, CMSampleBuffer delivery

cameraman-app-core
  commands, reducer, scenes, persistence, diagnostics snapshots

cameraman-ui-egui
  views and user intent only; no capture/render clock ownership

xtask
  bundle, sign, notarize, SBOM, benchmark and release evidence
```

Целевой поток:

```text
AVFoundation CVPixelBuffer
  -> bounded latest-only source slot
  -> CVMetalTextureCache
  -> Metal scene compositor
  -> bounded IOSurface-backed output pool
  -> preview texture
  -> authorized cross-process surface handoff
  -> CMIO CMSampleBuffer
```

CPU compositor нужен с первого дня как детерминированный oracle, fallback при
device loss и источник golden tests. GPU path принимается только без обязательного
readback: уже измеренный Metal+readback был медленнее CPU примерно в 3.79 раза.

## Маленькие этапы реализации

| Этап | Изолированный результат | Критерий готовности |
|---:|---|---|
| 1 | `cameraman-types` без macOS API | property tests для размеров, времени, цвета и wire schema |
| 2 | Latest-only runtime slots | Loom/model tests: нет backlog, torn state и unbounded allocation |
| 3 | CPU oracle | golden/property/fuzz parity на crop/rotate/mirror/alpha |
| 4 | Direct AVFoundation capture | формат выбирается под output, stop ограничен по времени |
| 5 | Bounded IOSurface pool | ownership state machine и allocation ceiling доказаны тестом |
| 6 | Metal compositor | pixel parity/quality gate и p95 лучше CPU на целевом hardware |
| 7 | Cross-process surface handoff | crash/restart, stale handle и unauthorized client tests |
| 8 | CMIO adapter | 15/30/60 FPS pacing, repeat/drop/discontinuity contracts |
| 9 | UI reducer | UI можно полностью тестировать без camera/Metal/CMIO |
| 10 | Product qualification | physical-camera latency, 8-hour soak, VoiceOver, notarization |

Каждый этап должен собираться и тестироваться отдельно. Следующий crate не
получает доступ к внутренним типам предыдущего, только к узкому trait/contract.
Так SOLID становится границей компиляции, а DRY означает один владелец каждого
медиа-контракта, а не общий модуль со всем подряд.

## Чего не делать заново

- Не выбирать самый высокий camera FPS/размер без связи с output contract.
- Не использовать UI timer как источник истины для media cadence.
- Не добавлять GPU path с обязательным CPU readback.
- Не передавать full-frame bytes через файл или очередь без backpressure.
- Не создавать unbounded channels, caches или CoreVideo pools.
- Не смешивать control plane и pixel data plane в одном app state.
- Не добавлять Swift-слой: нужные platform boundaries остаются узкими Rust
  `objc2` adapters с отдельным unsafe invariant register.

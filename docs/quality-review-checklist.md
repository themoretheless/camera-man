# Image quality review

Automated exact-golden checks remain the correctness oracle. Perceptual scores
and human review are additional evidence; neither may hide a pixel-contract
regression.

## Automated report

- Use `representative_quality_fixture(127, 73)` so the corpus includes text,
  one-pixel edges, diagonals, gradients, skin-like tones, chroma detail and odd
  crop markers.
- Compare at least 30 frames per candidate with RGB MSE, PSNR and SSIM.
- Persist raw per-frame scores, seed and at least 1,000 deterministic bootstrap
  iterations. Report the mean and 95% interval from `summarize_quality`.
- Run offline VMAF on representative motion clips when a scaler, color path or
  compositor backend changes. Keep command, model version and raw JSON beside
  the release evidence.
- Reject a candidate when the exact golden differs unexpectedly even if its
  aggregate perceptual score passes.

## Subjective pass

Inspect the same output at 100% and 200% on a color-managed display:

- small UI text and hairlines remain readable without ringing;
- diagonal edges do not shimmer across consecutive frames;
- gradients do not introduce visible banding or a color cast;
- skin-like patches preserve hue and highlight detail;
- saturated red/green/blue edges do not bleed into adjacent pixels;
- odd crop offsets do not shift, soften or duplicate the edge;
- paused and live output match apart from intentional animation;
- CPU fallback matches any experimental backend at the contract boundary.

Record reviewer, display, macOS version, source clips and pass/fail notes. A new
GPU or linear-light path stays experimental until this checklist and the
end-to-end latency gate both pass.

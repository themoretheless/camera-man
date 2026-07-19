# Accessibility release checklist

This checklist is a release-candidate gate, not a claim that automated tests
replace assistive-technology testing. Record the macOS version, hardware,
CameraMan commit, VoiceOver version and tester beside the release artifact.

## Automated preflight

- Run `cargo test --locked --bin camera-man app:: -- --test-threads=1`.
- Run `scripts/capture-ui-fixtures.sh target/ui-fixtures-release` and retain all
  1x/2x workspace, Setup, empty, disconnected, install-error and running PNGs.
- Inspect the pseudo-long minimum/default/large fixtures for clipping,
  overlap, ambiguous truncation and lost primary actions.
- Confirm pointer-target and minimum-layout contract tests pass.

## VoiceOver workflow

1. Enable VoiceOver before launching the signed release candidate.
2. Traverse Scene, Sources, Preview, Output and Status in that order. Confirm
   every control has one stable role/name and every status includes text.
3. Select two sources using only the keyboard. Move the selected source earlier
   and later, then edit fit, rotation, mirror, opacity, position and crop.
4. Save the scene, undo and redo once, and confirm focus remains on the same
   logical source after reorder and scene refresh.
5. Start preview, hear the running state, then stop it. The camera LED must turn
   off and focus must remain in Output.
6. Open Setup, start the virtual-camera self-test, hear its phase, cancel it,
   start it again and wait for pass/fail. Finish with Stop.
7. Trigger camera discovery and frame/diagnostics export, then cancel each.
   Confirm cancellation is announced and the previous destination remains.
8. Disconnect a selected camera. Confirm the source identity, disconnected
   state and recovery action are announced without relying on color.
9. Trigger an extension-install failure. Confirm operation, cause, recovery and
   Retry activation are reachable before the technical signing checklist.

## System preferences

- Repeat the workflow with Reduce Motion enabled; no essential state may depend
  on a spinner or continuous animation.
- Repeat key status checks with Increase Contrast and Differentiate Without
  Color enabled.
- Verify English and Russian at 920x560, 1120x720 and 1440x900.

The release gate remains open until the signed build completes this manual
workflow on the minimum supported macOS version.

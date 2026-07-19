#!/usr/bin/env python3
import argparse
import json
from pathlib import Path


def stage(report, name):
    for latency in report["diagnostics"]["latency"]:
        if latency["stage"] == name:
            return latency
    raise ValueError(f"missing latency stage: {name}")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("profile", type=Path)
    parser.add_argument("report", type=Path)
    parser.add_argument("--smoke", action="store_true")
    args = parser.parse_args()

    profile = json.loads(args.profile.read_text(encoding="utf-8"))
    report = json.loads(args.report.read_text(encoding="utf-8"))
    failures = []

    def require(condition, message):
        if not condition:
            failures.append(message)

    expected_hardware = profile["hardware"]
    actual_hardware = report["hardware"]
    require(actual_hardware["os"] == expected_hardware["os"], "OS mismatch")
    require(
        actual_hardware["architecture"] == expected_hardware["architecture"],
        "architecture mismatch",
    )
    require(
        expected_hardware["cpu_brand_contains"] in actual_hardware["cpu_brand"],
        "CPU profile mismatch",
    )

    workload = profile["workload"]
    actual_format = report["format"]
    require(actual_format["width"] == workload["width"], "width mismatch")
    require(actual_format["height"] == workload["height"], "height mismatch")
    require(actual_format["fps"] == workload["fps"], "fps mismatch")
    require(report["source_count"] == workload["source_count"], "source count mismatch")
    require(
        report["warmup_frames"] == workload["warmup_frames"],
        "warmup contract mismatch",
    )
    require(
        report["queue_contract"]["transport_slots"] == workload["transport_slots"],
        "transport queue is not the reviewed bounded size",
    )
    require(
        report["queue_contract"]["max_pending_render_jobs"]
        == workload["max_pending_render_jobs"],
        "render queue is not the reviewed bounded size",
    )
    if not args.smoke:
        require(
            report["elapsed_seconds"] >= workload["minimum_duration_seconds"],
            "run is shorter than the release qualification duration",
        )

    validation = report["validation"]
    require(validation["reads"] == report["output_frames"], "not every frame was read")
    for field in ("read_misses", "sequence_errors", "generation_errors", "torn_frames"):
        require(validation[field] == 0, f"{field} must be zero")

    drops = report["diagnostics"]["drops"]
    require(drops["queue_replacement"] == 0, "render queue replacements occurred")
    require(drops["all_slots_busy"] == 0, "all mmap slots became busy")
    require(drops["discontinuity"] == 0, "transport discontinuities occurred")

    budgets = profile["budgets"]
    require(
        report["late_frame_ratio"] <= budgets["maximum_late_frame_ratio"],
        "late-frame ratio exceeded budget",
    )
    # A three-second smoke run is dominated by allocator and framework warmup.
    # Memory qualification belongs to the fixed eight-hour release profile.
    if not args.smoke:
        require(
            report["max_resident_growth_bytes"]
            <= budgets["maximum_resident_growth_bytes"],
            "maximum RSS growth exceeded budget",
        )
        require(
            report["final_resident_growth_bytes"]
            <= budgets["maximum_final_resident_growth_bytes"],
            "final RSS growth exceeded budget",
        )

    compose = stage(report, "compose")
    publish = stage(report, "publish")
    frame = report["frame_latency"]
    for measured, prefix in ((compose, "compose"), (publish, "publish"), (frame, "frame")):
        require(measured["samples"] > 0, f"{prefix} has no latency samples")
        require(
            measured["p95_nanos"] <= budgets[f"{prefix}_p95_nanos"],
            f"{prefix} p95 exceeded budget",
        )
        require(
            measured["p99_nanos"] <= budgets[f"{prefix}_p99_nanos"],
            f"{prefix} p99 exceeded budget",
        )

    if failures:
        for failure in failures:
            print(f"FAIL: {failure}")
        raise SystemExit(1)
    mode = "smoke" if args.smoke else "release"
    memory_note = (
        ", RSS not qualified by smoke"
        if args.smoke
        else f", RSS growth {report['max_resident_growth_bytes'] / 1_048_576:.2f} MiB"
    )
    print(
        f"PASS {profile['profile_id']} ({mode}): {report['output_frames']} frames, "
        f"0 torn, p95 {frame['p95_nanos'] / 1_000_000:.2f} ms"
        f"{memory_note}"
    )


if __name__ == "__main__":
    main()

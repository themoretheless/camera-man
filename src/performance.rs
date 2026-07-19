use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use serde::{Deserialize, Serialize};

const COPY_STAGE_COUNT: usize = 7;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum CopyStage {
    CaptureDecode = 0,
    CompositionOutput = 1,
    SharedMemoryPublish = 2,
    SharedMemoryMaterialize = 3,
    FileSpool = 4,
    CoreVideoUpload = 5,
    PreviewConversion = 6,
}

impl CopyStage {
    pub const ALL: [Self; COPY_STAGE_COUNT] = [
        Self::CaptureDecode,
        Self::CompositionOutput,
        Self::SharedMemoryPublish,
        Self::SharedMemoryMaterialize,
        Self::FileSpool,
        Self::CoreVideoUpload,
        Self::PreviewConversion,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::CaptureDecode => "capture_decode",
            Self::CompositionOutput => "composition_output",
            Self::SharedMemoryPublish => "shared_memory_publish",
            Self::SharedMemoryMaterialize => "shared_memory_materialize",
            Self::FileSpool => "file_spool",
            Self::CoreVideoUpload => "core_video_upload",
            Self::PreviewConversion => "preview_conversion",
        }
    }

    pub const fn reason(self) -> &'static str {
        match self {
            Self::CaptureDecode => "camera backend converts its native frame into canonical BGRA",
            Self::CompositionOutput => "the compositor writes the canonical output surface",
            Self::SharedMemoryPublish => "the app crosses the process boundary through mmap",
            Self::SharedMemoryMaterialize => {
                "owned-reader compatibility path; CMIO uses a borrowed slot"
            }
            Self::FileSpool => "explicit disk fallback when shared memory is disabled",
            Self::CoreVideoUpload => "CMIO copies the borrowed slot into a pooled CVPixelBuffer",
            Self::PreviewConversion => {
                "the UI needs downscaled RGBA pixels rather than full-size BGRA"
            }
        }
    }

    const fn index(self) -> usize {
        self as usize
    }
}

#[derive(Default)]
struct StageCounters {
    copy_operations: AtomicU64,
    copied_bytes: AtomicU64,
    allocations: AtomicU64,
    allocated_bytes: AtomicU64,
}

pub struct CopyLedger {
    stages: [StageCounters; COPY_STAGE_COUNT],
}

impl Default for CopyLedger {
    fn default() -> Self {
        Self {
            stages: std::array::from_fn(|_| StageCounters::default()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CopyStageSnapshot {
    pub stage: CopyStage,
    pub copy_operations: u64,
    pub copied_bytes: u64,
    pub allocations: u64,
    pub allocated_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CopyLedgerSnapshot {
    pub schema_version: u32,
    pub stages: Vec<CopyStageSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CopyStagePerOutputSnapshot {
    pub stage: CopyStage,
    pub copies_per_output: f64,
    pub copied_bytes_per_output: f64,
    pub allocations_per_output: f64,
    pub allocated_bytes_per_output: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CopyLedgerPerOutputSnapshot {
    pub schema_version: u32,
    pub output_frames: u64,
    pub stages: Vec<CopyStagePerOutputSnapshot>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessUsageSnapshot {
    pub resident_size_bytes: u64,
    pub physical_footprint_bytes: u64,
    pub lifetime_max_physical_footprint_bytes: u64,
    pub rss_high_water_bytes: u64,
    pub allocator_blocks_in_use: u64,
    pub allocator_size_in_use_bytes: u64,
    pub allocator_peak_size_in_use_bytes: u64,
    pub allocator_size_allocated_bytes: u64,
    pub allocator_fragmentation_bytes: u64,
    pub package_idle_wakeups: u64,
    pub interrupt_wakeups: u64,
    pub pageins: u64,
    pub billed_energy_raw: u64,
    pub serviced_energy_raw: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct ProcessUsageRates {
    pub elapsed_seconds: f64,
    pub package_idle_wakeups_per_second: f64,
    pub interrupt_wakeups_per_second: f64,
    pub pageins_per_second: f64,
    pub billed_energy_raw_per_second: f64,
    pub serviced_energy_raw_per_second: f64,
}

impl ProcessUsageSnapshot {
    pub fn difference_since(self, start: Self) -> Self {
        Self {
            resident_size_bytes: self
                .resident_size_bytes
                .saturating_sub(start.resident_size_bytes),
            physical_footprint_bytes: self
                .physical_footprint_bytes
                .saturating_sub(start.physical_footprint_bytes),
            lifetime_max_physical_footprint_bytes: self
                .lifetime_max_physical_footprint_bytes
                .saturating_sub(start.lifetime_max_physical_footprint_bytes),
            rss_high_water_bytes: self
                .rss_high_water_bytes
                .saturating_sub(start.rss_high_water_bytes),
            allocator_blocks_in_use: self
                .allocator_blocks_in_use
                .saturating_sub(start.allocator_blocks_in_use),
            allocator_size_in_use_bytes: self
                .allocator_size_in_use_bytes
                .saturating_sub(start.allocator_size_in_use_bytes),
            allocator_peak_size_in_use_bytes: self
                .allocator_peak_size_in_use_bytes
                .saturating_sub(start.allocator_peak_size_in_use_bytes),
            allocator_size_allocated_bytes: self
                .allocator_size_allocated_bytes
                .saturating_sub(start.allocator_size_allocated_bytes),
            allocator_fragmentation_bytes: self
                .allocator_fragmentation_bytes
                .saturating_sub(start.allocator_fragmentation_bytes),
            package_idle_wakeups: self
                .package_idle_wakeups
                .saturating_sub(start.package_idle_wakeups),
            interrupt_wakeups: self
                .interrupt_wakeups
                .saturating_sub(start.interrupt_wakeups),
            pageins: self.pageins.saturating_sub(start.pageins),
            billed_energy_raw: self
                .billed_energy_raw
                .saturating_sub(start.billed_energy_raw),
            serviced_energy_raw: self
                .serviced_energy_raw
                .saturating_sub(start.serviced_energy_raw),
        }
    }

    pub fn rates_over(self, elapsed: Duration) -> ProcessUsageRates {
        let seconds = elapsed.as_secs_f64().max(f64::EPSILON);
        ProcessUsageRates {
            elapsed_seconds: elapsed.as_secs_f64(),
            package_idle_wakeups_per_second: self.package_idle_wakeups as f64 / seconds,
            interrupt_wakeups_per_second: self.interrupt_wakeups as f64 / seconds,
            pageins_per_second: self.pageins as f64 / seconds,
            billed_energy_raw_per_second: self.billed_energy_raw as f64 / seconds,
            serviced_energy_raw_per_second: self.serviced_energy_raw as f64 / seconds,
        }
    }
}

impl CopyLedgerSnapshot {
    pub fn difference_since(&self, earlier: &Self) -> Self {
        Self {
            schema_version: self.schema_version,
            stages: CopyStage::ALL
                .into_iter()
                .map(|stage| {
                    let current = self.stages.iter().find(|snapshot| snapshot.stage == stage);
                    let previous = earlier
                        .stages
                        .iter()
                        .find(|snapshot| snapshot.stage == stage);
                    CopyStageSnapshot {
                        stage,
                        copy_operations: current
                            .map_or(0, |snapshot| snapshot.copy_operations)
                            .saturating_sub(
                                previous.map_or(0, |snapshot| snapshot.copy_operations),
                            ),
                        copied_bytes: current
                            .map_or(0, |snapshot| snapshot.copied_bytes)
                            .saturating_sub(previous.map_or(0, |snapshot| snapshot.copied_bytes)),
                        allocations: current
                            .map_or(0, |snapshot| snapshot.allocations)
                            .saturating_sub(previous.map_or(0, |snapshot| snapshot.allocations)),
                        allocated_bytes: current
                            .map_or(0, |snapshot| snapshot.allocated_bytes)
                            .saturating_sub(
                                previous.map_or(0, |snapshot| snapshot.allocated_bytes),
                            ),
                    }
                })
                .collect(),
        }
    }

    pub fn per_output_frame(&self, output_frames: u64) -> CopyLedgerPerOutputSnapshot {
        let denominator = output_frames.max(1) as f64;
        CopyLedgerPerOutputSnapshot {
            schema_version: 1,
            output_frames,
            stages: self
                .stages
                .iter()
                .map(|stage| CopyStagePerOutputSnapshot {
                    stage: stage.stage,
                    copies_per_output: stage.copy_operations as f64 / denominator,
                    copied_bytes_per_output: stage.copied_bytes as f64 / denominator,
                    allocations_per_output: stage.allocations as f64 / denominator,
                    allocated_bytes_per_output: stage.allocated_bytes as f64 / denominator,
                })
                .collect(),
        }
    }
}

impl CopyLedger {
    pub fn record_copy(&self, stage: CopyStage, bytes: usize) {
        let counters = &self.stages[stage.index()];
        counters.copy_operations.fetch_add(1, Ordering::Relaxed);
        counters
            .copied_bytes
            .fetch_add(bytes as u64, Ordering::Relaxed);
    }

    pub fn record_allocation(&self, stage: CopyStage, bytes: usize) {
        let counters = &self.stages[stage.index()];
        counters.allocations.fetch_add(1, Ordering::Relaxed);
        counters
            .allocated_bytes
            .fetch_add(bytes as u64, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> CopyLedgerSnapshot {
        CopyLedgerSnapshot {
            schema_version: 1,
            stages: CopyStage::ALL
                .into_iter()
                .map(|stage| {
                    let counters = &self.stages[stage.index()];
                    CopyStageSnapshot {
                        stage,
                        copy_operations: counters.copy_operations.load(Ordering::Relaxed),
                        copied_bytes: counters.copied_bytes.load(Ordering::Relaxed),
                        allocations: counters.allocations.load(Ordering::Relaxed),
                        allocated_bytes: counters.allocated_bytes.load(Ordering::Relaxed),
                    }
                })
                .collect(),
        }
    }
}

pub fn copy_ledger() -> &'static CopyLedger {
    static LEDGER: OnceLock<CopyLedger> = OnceLock::new();
    LEDGER.get_or_init(CopyLedger::default)
}

#[cfg(target_os = "macos")]
pub fn process_usage_snapshot() -> ProcessUsageSnapshot {
    let mut info = std::mem::MaybeUninit::<libc::rusage_info_v4>::zeroed();
    // SAFETY: info provides writable storage for the requested V4 structure;
    // the current pid and flavor are passed by value.
    let status = unsafe {
        libc::proc_pid_rusage(
            libc::getpid(),
            libc::RUSAGE_INFO_V4,
            info.as_mut_ptr().cast::<libc::rusage_info_t>(),
        )
    };
    let info = if status == 0 {
        // SAFETY: a successful proc_pid_rusage call initialized the complete
        // V4 output structure.
        unsafe { info.assume_init() }
    } else {
        // SAFETY: rusage_info_v4 is a C aggregate of integer counters for
        // which the all-zero representation is valid and means unavailable.
        unsafe { std::mem::zeroed() }
    };
    let allocator = allocator_usage_snapshot();
    ProcessUsageSnapshot {
        resident_size_bytes: info.ri_resident_size,
        physical_footprint_bytes: info.ri_phys_footprint,
        lifetime_max_physical_footprint_bytes: info.ri_lifetime_max_phys_footprint,
        rss_high_water_bytes: rss_high_water_bytes(),
        allocator_blocks_in_use: allocator.blocks_in_use,
        allocator_size_in_use_bytes: allocator.size_in_use_bytes,
        allocator_peak_size_in_use_bytes: allocator.peak_size_in_use_bytes,
        allocator_size_allocated_bytes: allocator.size_allocated_bytes,
        allocator_fragmentation_bytes: allocator
            .size_allocated_bytes
            .saturating_sub(allocator.size_in_use_bytes),
        package_idle_wakeups: info.ri_pkg_idle_wkups,
        interrupt_wakeups: info.ri_interrupt_wkups,
        pageins: info.ri_pageins,
        billed_energy_raw: info.ri_billed_energy,
        serviced_energy_raw: info.ri_serviced_energy,
    }
}

#[cfg(target_os = "macos")]
#[derive(Debug, Clone, Copy, Default)]
struct AllocatorUsageSnapshot {
    blocks_in_use: u64,
    size_in_use_bytes: u64,
    peak_size_in_use_bytes: u64,
    size_allocated_bytes: u64,
}

#[cfg(target_os = "macos")]
#[repr(C)]
struct DarwinMallocStatistics {
    blocks_in_use: libc::c_uint,
    size_in_use: usize,
    max_size_in_use: usize,
    size_allocated: usize,
}

#[cfg(target_os = "macos")]
fn allocator_usage_snapshot() -> AllocatorUsageSnapshot {
    let mut statistics = DarwinMallocStatistics {
        blocks_in_use: 0,
        size_in_use: 0,
        max_size_in_use: 0,
        size_allocated: 0,
    };
    // SAFETY: a null zone requests aggregate statistics for all malloc zones;
    // `statistics` is writable for the exact C structure and is not retained.
    unsafe {
        malloc_zone_statistics(std::ptr::null_mut(), &mut statistics);
    }
    AllocatorUsageSnapshot {
        blocks_in_use: u64::from(statistics.blocks_in_use),
        size_in_use_bytes: statistics.size_in_use as u64,
        peak_size_in_use_bytes: statistics.max_size_in_use as u64,
        size_allocated_bytes: statistics.size_allocated as u64,
    }
}

#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn malloc_zone_statistics(zone: *mut libc::c_void, stats: *mut DarwinMallocStatistics);
}

#[cfg(target_os = "macos")]
fn rss_high_water_bytes() -> u64 {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::zeroed();
    // SAFETY: usage points to writable storage for one rusage and remains
    // alive for the call.
    if unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) } == 0 {
        // SAFETY: a zero return guarantees the output structure is initialized.
        unsafe { usage.assume_init().ru_maxrss.max(0) as u64 }
    } else {
        0
    }
}

#[cfg(not(target_os = "macos"))]
pub fn process_usage_snapshot() -> ProcessUsageSnapshot {
    ProcessUsageSnapshot::default()
}

/// Returns the lower empirical percentile index used by runtime and benchmark reports.
pub fn percentile_index(sample_count: usize, percentile: usize) -> Option<usize> {
    (sample_count > 0).then(|| (sample_count - 1).saturating_mul(percentile.min(100)) / 100)
}

pub fn copy_ledger_markdown() -> String {
    let mut markdown = String::from("| Stage | Why it exists |\n|---|---|\n");
    for stage in CopyStage::ALL {
        markdown.push_str("| `");
        markdown.push_str(stage.name());
        markdown.push_str("` | ");
        markdown.push_str(stage.reason());
        markdown.push_str(" |\n");
    }
    markdown
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_copy_and_allocation_bytes_by_stage() {
        let ledger = CopyLedger::default();
        ledger.record_copy(CopyStage::CoreVideoUpload, 16);
        ledger.record_allocation(CopyStage::CoreVideoUpload, 32);

        let snapshot = ledger.snapshot();
        let stage = snapshot
            .stages
            .iter()
            .find(|stage| stage.stage == CopyStage::CoreVideoUpload)
            .unwrap();
        assert_eq!(stage.copy_operations, 1);
        assert_eq!(stage.copied_bytes, 16);
        assert_eq!(stage.allocations, 1);
        assert_eq!(stage.allocated_bytes, 32);
    }

    #[test]
    fn markdown_lists_every_copy_boundary() {
        let markdown = copy_ledger_markdown();
        for stage in CopyStage::ALL {
            assert!(markdown.contains(stage.name()));
        }
    }

    #[test]
    fn normalizes_copy_cost_by_completed_output_frames() {
        let ledger = CopyLedger::default();
        ledger.record_copy(CopyStage::SharedMemoryPublish, 200);
        ledger.record_copy(CopyStage::SharedMemoryPublish, 200);
        ledger.record_allocation(CopyStage::SharedMemoryPublish, 100);

        let normalized = ledger.snapshot().per_output_frame(2);
        let publish = normalized
            .stages
            .iter()
            .find(|stage| stage.stage == CopyStage::SharedMemoryPublish)
            .unwrap();
        assert_eq!(publish.copies_per_output, 1.0);
        assert_eq!(publish.copied_bytes_per_output, 200.0);
        assert_eq!(publish.allocations_per_output, 0.5);
    }

    #[test]
    fn process_usage_delta_keeps_allocator_categories_separate() {
        let start = ProcessUsageSnapshot {
            allocator_size_in_use_bytes: 100,
            allocator_size_allocated_bytes: 180,
            allocator_fragmentation_bytes: 80,
            ..ProcessUsageSnapshot::default()
        };
        let end = ProcessUsageSnapshot {
            allocator_size_in_use_bytes: 140,
            allocator_size_allocated_bytes: 260,
            allocator_fragmentation_bytes: 120,
            ..ProcessUsageSnapshot::default()
        };
        let delta = end.difference_since(start);
        assert_eq!(delta.allocator_size_in_use_bytes, 40);
        assert_eq!(delta.allocator_size_allocated_bytes, 80);
        assert_eq!(delta.allocator_fragmentation_bytes, 40);
    }

    #[test]
    fn computes_saturating_copy_ledger_delta() {
        let ledger = CopyLedger::default();
        ledger.record_copy(CopyStage::CompositionOutput, 100);
        let before = ledger.snapshot();
        ledger.record_copy(CopyStage::CompositionOutput, 200);
        ledger.record_allocation(CopyStage::CompositionOutput, 300);
        let delta = ledger.snapshot().difference_since(&before);
        let composition = delta
            .stages
            .iter()
            .find(|stage| stage.stage == CopyStage::CompositionOutput)
            .unwrap();
        assert_eq!(composition.copy_operations, 1);
        assert_eq!(composition.copied_bytes, 200);
        assert_eq!(composition.allocations, 1);
        assert_eq!(composition.allocated_bytes, 300);
    }

    #[test]
    fn computes_process_usage_delta_and_rates() {
        let start = ProcessUsageSnapshot {
            package_idle_wakeups: 10,
            billed_energy_raw: 20,
            ..ProcessUsageSnapshot::default()
        };
        let finish = ProcessUsageSnapshot {
            package_idle_wakeups: 16,
            billed_energy_raw: 30,
            ..ProcessUsageSnapshot::default()
        };
        let delta = finish.difference_since(start);
        let rates = delta.rates_over(Duration::from_secs(2));

        assert_eq!(delta.package_idle_wakeups, 6);
        assert_eq!(delta.billed_energy_raw, 10);
        assert_eq!(rates.package_idle_wakeups_per_second, 3.0);
        assert_eq!(rates.billed_energy_raw_per_second, 5.0);
    }

    #[test]
    fn percentile_index_handles_empty_small_and_full_samples() {
        assert_eq!(percentile_index(0, 95), None);
        assert_eq!(percentile_index(1, 95), Some(0));
        assert_eq!(percentile_index(100, 95), Some(94));
        assert_eq!(percentile_index(101, 95), Some(95));
        assert_eq!(percentile_index(100, 101), Some(99));
    }
}

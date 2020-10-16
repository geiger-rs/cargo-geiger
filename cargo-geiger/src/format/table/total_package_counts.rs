use crate::format::CrateDetectionStatus;

use geiger::CounterBlock;

pub struct TotalPackageCounts {
    pub none_detected_forbids_unsafe: i32,
    pub none_detected_allows_unsafe: i32,
    pub unsafe_detected: i32,
    pub total_counter_block: CounterBlock,
    pub total_unused_counter_block: CounterBlock,
}

impl TotalPackageCounts {
    pub fn new() -> TotalPackageCounts {
        TotalPackageCounts {
            none_detected_forbids_unsafe: 0,
            none_detected_allows_unsafe: 0,
            unsafe_detected: 0,
            total_counter_block: CounterBlock::default(),
            total_unused_counter_block: CounterBlock::default(),
        }
    }

    pub fn get_total_detection_status(&self) -> CrateDetectionStatus {
        match (
            self.none_detected_forbids_unsafe > 0,
            self.none_detected_allows_unsafe > 0,
            self.unsafe_detected > 0,
        ) {
            (_, _, true) => CrateDetectionStatus::UnsafeDetected,
            (true, false, false) => {
                CrateDetectionStatus::NoneDetectedForbidsUnsafe
            }
            _ => CrateDetectionStatus::NoneDetectedAllowsUnsafe,
        }
    }
}

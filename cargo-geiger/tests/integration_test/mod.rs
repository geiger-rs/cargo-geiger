use crate::context::Context;
use crate::report::to_quick_report;
use crate::run::run_geiger_with;

use cargo_geiger_serde::{QuickSafetyReport, ReportEntry, SafetyReport};
use std::process::Output;

pub trait IntegrationTest {
    const NAME: &'static str;

    fn expected_report(&self, cx: &Context) -> SafetyReport;
    fn expected_report_entry(&self, cx: &Context) -> ReportEntry;

    fn expected_quick_report(&self, cx: &Context) -> QuickSafetyReport {
        to_quick_report(self.expected_report(cx))
    }

    fn run(&self) {
        let (output, cx) = run_geiger_json(Self::NAME);
        assert!(output.status.success());
        let actual =
            serde_json::from_slice::<SafetyReport>(&output.stdout).unwrap();
        assert_eq!(actual, self.expected_report(&cx));
    }

    fn run_quick(&self) {
        let (output, cx) = run_geiger_json_quick(Self::NAME);
        assert!(output.status.success());
        let actual =
            serde_json::from_slice::<QuickSafetyReport>(&output.stdout)
                .unwrap();
        assert_eq!(actual, self.expected_quick_report(&cx));
    }
}

fn run_geiger_json(test_name: &str) -> (Output, Context) {
    run_geiger_with(test_name, &["--json"])
}

fn run_geiger_json_quick(test_name: &str) -> (Output, Context) {
    run_geiger_with(test_name, &["--forbid-only", "--json"])
}

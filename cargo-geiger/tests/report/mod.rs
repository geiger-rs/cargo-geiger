use cargo_geiger_serde::{
    PackageId, QuickReportEntry, QuickSafetyReport, ReportEntry, SafetyReport,
};
use std::collections::{HashMap, HashSet};
use std::hash::Hash;

fn report_entry_list_to_map<I>(entries: I) -> HashMap<PackageId, ReportEntry>
where
    I: IntoIterator<Item = ReportEntry>,
{
    entries
        .into_iter()
        .map(|e| (e.package.id.clone(), e))
        .collect()
}

pub fn to_set<I>(items: I) -> HashSet<I::Item>
where
    I: IntoIterator,
    I::Item: Hash + Eq,
{
    items.into_iter().collect()
}

// This function does not handle all merges but works well enough to avoid repetition in these
// tests.
pub fn merge_test_reports(report: &mut SafetyReport, other: SafetyReport) {
    report.packages.extend(other.packages);
    report
        .packages_without_metrics
        .extend(other.packages_without_metrics);
    report
        .used_but_not_scanned_files
        .extend(other.used_but_not_scanned_files);
}

pub fn to_quick_report(report: SafetyReport) -> QuickSafetyReport {
    let entries = report
        .packages
        .into_iter()
        .map(|(id, entry)| {
            let quick_entry = QuickReportEntry {
                package: entry.package,
                forbids_unsafe: entry.unsafety.forbids_unsafe,
            };
            (id, quick_entry)
        })
        .collect();
    QuickSafetyReport {
        packages: entries,
        packages_without_metrics: report.packages_without_metrics,
    }
}

pub fn single_entry_safety_report(entry: ReportEntry) -> SafetyReport {
    SafetyReport {
        packages: report_entry_list_to_map(vec![entry]),
        ..Default::default()
    }
}

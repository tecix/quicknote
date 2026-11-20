//! QuickAdd "Capture" re-implementation: format syntax, placement rules and
//! vault file handling. No GUI dependencies.

pub mod edit;
pub mod format;
pub mod vault;

pub use edit::{apply_capture, InsertAfter};
pub use format::{format_text, moment_format};
pub use vault::{
    detect_vaults, list_capture_choices, resolve_target, run_capture, CaptureSet, Choice, Vault,
};

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use std::collections::HashMap;

    fn now() -> chrono::DateTime<chrono::Local> {
        chrono::Local.with_ymd_and_hms(2026, 11, 1, 9, 5, 0).unwrap()
    }

    #[test]
    fn formats() {
        let g = HashMap::new();
        assert_eq!(
            format_text("ts/{{date:yyyy}}/{{date:yyyy-[W]WW}}", "", &now(), &g),
            "ts/2026/2026-W44"
        );
        assert_eq!(
            format_text("- {{DATE:ddd HH:mm}} \u{2014} {{VALUE}}\\n", "hi $& x", &now(), &g),
            "- Sun 09:05 \u{2014} hi $& x\n"
        );
        assert_eq!(format_text("{{date+1}} {{DATE:DD/MM+2}}", "", &now(), &g), "2026-11-02 03/11");
        let d = chrono::Local.with_ymd_and_hms(2024, 12, 30, 0, 0, 0).unwrap();
        assert_eq!(moment_format(&d, "GGGG-WW gggg-ww"), "2025-01 2025-01");
        let d = chrono::Local.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        assert_eq!(moment_format(&d, "GGGG-WW Do dddd"), "2026-01 1st Thursday");
    }

    #[test]
    fn placement() {
        let task = InsertAfter {
            enabled: true,
            after: "## To-do".into(),
            insert_at_end: true,
            create_if_not_found: true,
            create_at_top: true,
            ..Default::default()
        };
        let c = "---\na: 1\n---\n# Day\n## To-do\n- [ ] one\n\n## Notes\nx\n";
        assert_eq!(
            apply_capture(c, "- [ ] two", false, &task).unwrap(),
            "---\na: 1\n---\n# Day\n## To-do\n- [ ] one\n- [ ] two\n\n## Notes\nx\n"
        );
        assert_eq!(
            apply_capture("---\na: 1\n---\nbody\n", "- [ ] t", false, &task).unwrap(),
            "---\na: 1\n---\n## To-do\n- [ ] t\nbody\n"
        );
        let none = InsertAfter::default();
        assert_eq!(apply_capture(c, "J", true, &none).unwrap(), format!("---\na: 1\n---\nJ\n{}", &c[13..]));
        assert_eq!(apply_capture("x", "J", false, &none).unwrap(), "x\nJ\n");
        let bottom = InsertAfter { enabled: true, after: "# Logs".into(), create_if_not_found: true, ..Default::default() };
        assert_eq!(apply_capture("a\n", "L", false, &bottom).unwrap(), "a\n\n# Logs\nL\n");
    }
}

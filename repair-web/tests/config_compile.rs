#[path = "../../examples/repair/config.rs"]
pub mod config;
#[path = "../../examples/repair/config_build.rs"]
mod config_build;

mod scene {
    pub use crate::config;
}

include!(concat!(env!("OUT_DIR"), "/repair_config.rs"));

const DEFAULT: &str = "schema_version = 1\npolicy_set = \"repair_cafe_v1\"\ntrace_capacity = 128\nreport_to_intake_ticks = 1\nreport_to_technician_ticks = 2\ntechnician_to_intake_ticks = 1\ntechnician_to_collection_ticks = 2\ncable_receipt_at_tick = 4\n";

fn replace(field: &str, value: &str) -> String {
    DEFAULT
        .lines()
        .map(|line| {
            if line.starts_with(&format!("{field} =")) {
                format!("{field} = {value}\n")
            } else {
                format!("{line}\n")
            }
        })
        .collect()
}

#[test]
fn default_compiles_deterministically_without_source_metadata() {
    let generated = config_build::compile(DEFAULT, "/private/path/repair.toml").unwrap();
    assert_eq!(
        generated,
        config_build::compile(DEFAULT, "another.toml").unwrap()
    );
    let reordered = DEFAULT
        .lines()
        .filter(|line| !line.starts_with('#'))
        .rev()
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(
        generated,
        config_build::compile(&reordered, "reordered.toml").unwrap()
    );
    assert!(
        generated.starts_with("pub fn generated_config() -> crate::scene::config::ValidatedConfig")
    );
    assert!(generated.contains("policy_set: \"repair_cafe_v1\""));
    assert!(!generated.contains("private/path"));
    assert!(!generated.contains("toml::"));
    let direct = config::validate(config::DEFAULT_INPUT).unwrap();
    assert_eq!(
        generated_config(),
        config_build::validate_source(include_str!("../repair.toml"), "repair.toml").unwrap()
    );
    assert_eq!(direct.trace_capacity(), 128);
    assert_eq!(direct.report_to_intake_ticks(), 1);
    assert_eq!(direct.report_to_technician_ticks(), 2);
    assert_eq!(direct.technician_to_intake_ticks(), 1);
    assert_eq!(direct.technician_to_collection_ticks(), 2);
    assert_eq!(direct.cable_receipt_at_tick(), 4);
}

#[test]
fn syntax_unknown_missing_and_duplicate_keys_fail_closed() {
    for (source, code) in [
        (replace("policy_set", "\"unterminated"), "syntax"),
        (format!("{DEFAULT}\nextra = 1\n"), "unknown_field"),
        (
            DEFAULT
                .lines()
                .filter(|line| !line.starts_with("trace_capacity ="))
                .collect::<Vec<_>>()
                .join("\n"),
            "missing_field",
        ),
        (format!("{DEFAULT}\ntrace_capacity = 2\n"), "syntax"),
    ] {
        let diagnostics = config_build::compile(&source, "invalid.toml").unwrap_err();
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, code, "{diagnostics:?}");
        assert_eq!(diagnostics[0].filename, "invalid.toml");
        assert!(diagnostics[0].span.end <= source.len());
        assert!(diagnostics[0].line >= 1);
        assert!(diagnostics[0].column >= 1);
    }
}

#[test]
fn all_fields_are_required_and_reject_wrong_types() {
    for field in [
        "schema_version",
        "policy_set",
        "trace_capacity",
        "report_to_intake_ticks",
        "report_to_technician_ticks",
        "technician_to_intake_ticks",
        "technician_to_collection_ticks",
        "cable_receipt_at_tick",
    ] {
        let missing = DEFAULT
            .lines()
            .filter(|line| !line.starts_with(&format!("{field} =")))
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(
            config_build::compile(&missing, "missing.toml").unwrap_err()[0].code,
            "missing_field",
            "{field}"
        );
        let wrong = replace(
            field,
            if field == "policy_set" {
                "123"
            } else {
                "\"1\""
            },
        );
        assert_eq!(
            config_build::compile(&wrong, "type.toml").unwrap_err()[0].code,
            "invalid_type",
            "{field}"
        );
        if field != "policy_set" {
            assert_eq!(
                config_build::compile(&replace(field, "-1"), "negative.toml").unwrap_err()[0].code,
                "invalid_type",
                "{field}"
            );
        }
    }
}

#[test]
fn semantic_diagnostics_have_exact_value_spans_and_stable_order() {
    let source =
        replace("schema_version", "2").replace("trace_capacity = 128", "trace_capacity = 1025");
    let diagnostics = config_build::compile(&source, "bounded.toml").unwrap_err();
    assert_eq!(
        diagnostics,
        config_build::compile(&source, "bounded.toml").unwrap_err()
    );
    assert_eq!(diagnostics.len(), 2);
    for (diagnostic, field, value, code) in [
        (
            &diagnostics[0],
            config::Field::SchemaVersion,
            "2",
            "unsupported_version",
        ),
        (
            &diagnostics[1],
            config::Field::TraceCapacity,
            "1025",
            "out_of_range",
        ),
    ] {
        assert_eq!(diagnostic.field, Some(field));
        assert_eq!(diagnostic.code, code);
        assert_eq!(&source[diagnostic.span.clone()], value);
        let expected_start = source
            .find(&format!("{} = {value}", field.as_str()))
            .unwrap()
            + field.as_str().len()
            + 3;
        assert_eq!(
            diagnostic.span,
            expected_start..expected_start + value.len()
        );
        assert_eq!(diagnostic.column, field.as_str().len() + 4);
        assert!(diagnostic.to_string().contains("bounded.toml:"));
    }
}

#[test]
fn policy_strings_never_become_executable_rust() {
    for value in [
        "\"unknown\"",
        "'\"); panic!(\"injected\"); //'",
        "\"repair_cafe_v1\\ncode\"",
    ] {
        let source = replace("policy_set", value);
        let diagnostic = &config_build::compile(&source, "policy.toml").unwrap_err()[0];
        assert_eq!(diagnostic.code, "unsupported_policy");
        assert_eq!(diagnostic.field, Some(config::Field::PolicySet));
        assert_eq!(&source[diagnostic.span.clone()], value);
    }
}

#[test]
fn bounded_values_include_zero_and_maximum_without_timing_semantics() {
    for (capacity, tick) in [(0, 0), (1024, 1000)] {
        let input = config::Input {
            trace_capacity: capacity,
            report_to_intake_ticks: tick,
            report_to_technician_ticks: tick,
            technician_to_intake_ticks: tick,
            technician_to_collection_ticks: tick,
            cable_receipt_at_tick: tick,
            ..config::DEFAULT_INPUT
        };
        assert!(config::validate(input).is_ok());
        let source = DEFAULT
            .lines()
            .map(|line| {
                if line.starts_with("trace_capacity =") {
                    format!("trace_capacity = {capacity}")
                } else if line.contains("_ticks =") || line.starts_with("cable_receipt_at_tick =") {
                    format!("{} = {tick}", line.split(" =").next().unwrap())
                } else {
                    line.to_owned()
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(config_build::compile(&source, "bounds.toml").is_ok());
    }
}

#[test]
fn every_timing_field_rejects_out_of_range_and_delay_overflow() {
    use config::{Field, ValidationCode};
    for (field, set) in [
        (
            Field::ReportToIntakeTicks,
            (|input: &mut config::Input<'_>, value| input.report_to_intake_ticks = value)
                as fn(&mut config::Input<'_>, u64),
        ),
        (
            Field::ReportToTechnicianTicks,
            (|input: &mut config::Input<'_>, value| input.report_to_technician_ticks = value)
                as fn(&mut config::Input<'_>, u64),
        ),
        (
            Field::TechnicianToIntakeTicks,
            (|input: &mut config::Input<'_>, value| input.technician_to_intake_ticks = value)
                as fn(&mut config::Input<'_>, u64),
        ),
        (
            Field::TechnicianToCollectionTicks,
            (|input: &mut config::Input<'_>, value| input.technician_to_collection_ticks = value)
                as fn(&mut config::Input<'_>, u64),
        ),
        (
            Field::CableReceiptAtTick,
            (|input: &mut config::Input<'_>, value| input.cable_receipt_at_tick = value)
                as fn(&mut config::Input<'_>, u64),
        ),
    ] {
        for value in [1001, u64::MAX] {
            let mut input = config::DEFAULT_INPUT;
            set(&mut input, value);
            let errors = config::validate(input).unwrap_err();
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].field, field);
            assert_eq!(
                errors[0].code,
                if value == u64::MAX && field != Field::CableReceiptAtTick {
                    ValidationCode::Overflow
                } else {
                    ValidationCode::OutOfRange
                }
            );
        }
        let diagnostics =
            config_build::compile(&replace(field.as_str(), "1001"), "range.toml").unwrap_err();
        assert_eq!(diagnostics[0].field, Some(field));
        assert_eq!(diagnostics[0].code, "out_of_range");
    }
}

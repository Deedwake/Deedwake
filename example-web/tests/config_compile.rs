#[path = "../../examples/convoy/config.rs"]
pub mod config;
#[path = "../../examples/convoy/config_build.rs"]
mod config_build;

mod scene {
    pub use crate::config;
}

include!(concat!(env!("OUT_DIR"), "/convoy_config.rs"));

const DEFAULT: &str = include_str!("../convoy.toml");
const FIELDS: [&str; 10] = [
    "schema_version",
    "profile_id",
    "observer",
    "retention",
    "reaction",
    "sela_confidence",
    "trace_capacity",
    "initial_delivery_ticks",
    "routine_delivery_ticks",
    "intermediary_delivery_ticks",
];

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
fn generated_constructor_matches_validated_source_without_source_metadata() {
    let generated = config_build::compile(DEFAULT, "/private/path/convoy.toml").unwrap();
    let reordered = DEFAULT.lines().rev().collect::<Vec<_>>().join("\n");
    assert_eq!(
        generated,
        config_build::compile(&reordered, "other.toml").unwrap()
    );
    assert!(!generated.contains("private/path"));
    assert!(!generated.contains("toml::"));
    assert!(generated.contains("profile_id: \"configured-convoy-v1\""));
    assert_eq!(
        generated_config(),
        config_build::validate_source(DEFAULT, "convoy.toml").unwrap()
    );
    assert_eq!(
        generated_config(),
        config::validate(config::DEFAULT_INPUT).unwrap()
    );
}

#[test]
fn all_fields_are_required_and_reject_wrong_types() {
    for field in FIELDS {
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
        let string = matches!(
            field,
            "profile_id" | "observer" | "retention" | "reaction" | "sela_confidence"
        );
        let wrong = replace(field, if string { "123" } else { "\"1\"" });
        assert_eq!(
            config_build::compile(&wrong, "type.toml").unwrap_err()[0].code,
            "invalid_type",
            "{field}"
        );
        if !string {
            assert_eq!(
                config_build::compile(&replace(field, "-1"), "negative.toml").unwrap_err()[0].code,
                "invalid_type"
            );
        }
    }
}

#[test]
fn syntax_unknown_and_duplicate_keys_fail_closed() {
    for (source, code) in [
        (replace("observer", "\"unterminated"), "syntax"),
        (format!("{DEFAULT}\nextra = 1\n"), "unknown_field"),
        (format!("{DEFAULT}\ntrace_capacity = 2\n"), "syntax"),
        (
            format!("{DEFAULT}\nsela_confidence = \"trusted\"\n"),
            "syntax",
        ),
    ] {
        let diagnostics = config_build::compile(&source, "invalid.toml").unwrap_err();
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, code, "{diagnostics:?}");
        assert!(diagnostics[0].span.end <= source.len());
        assert!(diagnostics[0].line >= 1);
        assert!(diagnostics[0].column >= 1);
    }
}

#[test]
fn semantic_diagnostics_have_value_spans_and_stable_field_order() {
    let source = replace("schema_version", "3")
        .replace("trace_capacity = 128", "trace_capacity = 1025")
        .replace("observer = \"mara\"", "observer = \"other\"")
        .replace(
            "sela_confidence = \"trusted\"",
            "sela_confidence = \"other\"",
        );
    let diagnostics = config_build::compile(&source, "bounds.toml").unwrap_err();
    assert_eq!(
        diagnostics,
        config_build::compile(&source, "bounds.toml").unwrap_err()
    );
    for (diagnostic, field, value, code) in [
        (
            &diagnostics[0],
            config::Field::SchemaVersion,
            "3",
            "unsupported_version",
        ),
        (
            &diagnostics[1],
            config::Field::Observer,
            "\"other\"",
            "unsupported_observer",
        ),
        (
            &diagnostics[2],
            config::Field::SelaConfidence,
            "\"other\"",
            "unsupported_confidence",
        ),
        (
            &diagnostics[3],
            config::Field::TraceCapacity,
            "1025",
            "out_of_range",
        ),
    ] {
        assert_eq!(diagnostic.field, Some(field));
        assert_eq!(diagnostic.code, code);
        assert_eq!(&source[diagnostic.span.clone()], value);
        assert_eq!(diagnostic.column, field.as_str().len() + 4);
        assert!(diagnostic.to_string().contains("bounds.toml:"));
    }
}

#[test]
fn strings_are_validated_before_rust_generation() {
    for value in [
        "\"\"",
        "\"UPPER\"",
        "\"non ascii é\"",
        "'\"); panic!(\"injected\"); //'",
    ] {
        let diagnostic =
            &config_build::compile(&replace("profile_id", value), "profile.toml").unwrap_err()[0];
        assert_eq!(diagnostic.code, "invalid_profile_id");
    }
    for (field, value, code) in [
        ("observer", "\"Mara\"", "unsupported_observer"),
        ("retention", "\"exact\"", "unsupported_retention"),
        ("reaction", "\"restrict\"", "unsupported_reaction"),
        ("observer", "\"mara\\ncode\"", "unsupported_observer"),
        ("sela_confidence", "\"Trusted\"", "unsupported_confidence"),
        ("sela_confidence", "\"other\"", "unsupported_confidence"),
        (
            "sela_confidence",
            "\"trusted\\ncode\"",
            "unsupported_confidence",
        ),
    ] {
        assert_eq!(
            config_build::compile(&replace(field, value), "enum.toml").unwrap_err()[0].code,
            code
        );
    }
    let escaped = replace("profile_id", "\"configured\\u002dconvoy\\u002dv1\"");
    assert_eq!(
        config_build::compile(&escaped, "escaped.toml").unwrap(),
        config_build::compile(DEFAULT, "plain.toml").unwrap()
    );
}

#[test]
fn profile_and_numeric_bounds_are_inclusive() {
    for (capacity, delay, profile) in [(0, 0, "x".to_string()), (1024, 1000, "a".repeat(64))] {
        let input = config::Input {
            profile_id: &profile,
            trace_capacity: capacity,
            initial_delivery_ticks: delay,
            routine_delivery_ticks: delay,
            intermediary_delivery_ticks: delay,
            ..config::DEFAULT_INPUT
        };
        assert!(config::validate(input).is_ok());
    }
    let overlong = "a".repeat(65);
    assert!(
        config::validate(config::Input {
            profile_id: &overlong,
            ..config::DEFAULT_INPUT
        })
        .is_err()
    );
    for (field, set) in [
        (
            config::Field::InitialDeliveryTicks,
            (|input: &mut config::Input<'_>, value| input.initial_delivery_ticks = value)
                as fn(&mut config::Input<'_>, u64),
        ),
        (
            config::Field::RoutineDeliveryTicks,
            (|input: &mut config::Input<'_>, value| input.routine_delivery_ticks = value)
                as fn(&mut config::Input<'_>, u64),
        ),
        (
            config::Field::IntermediaryDeliveryTicks,
            (|input: &mut config::Input<'_>, value| input.intermediary_delivery_ticks = value)
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
                if value == u64::MAX {
                    config::ValidationCode::Overflow
                } else {
                    config::ValidationCode::OutOfRange
                }
            );
        }
        assert_eq!(
            config_build::compile(&replace(field.as_str(), "1001"), "range.toml").unwrap_err()[0]
                .code,
            "out_of_range"
        );
    }
}

#[test]
fn every_supported_enum_combination_validates() {
    for observer in ["mara", "sela"] {
        for retention in ["precise", "incomplete"] {
            for reaction in ["restrict_until_correction", "warn_and_continue"] {
                for sela_confidence in ["trusted", "unverified"] {
                    assert!(
                        config::validate(config::Input {
                            observer,
                            retention,
                            reaction,
                            sela_confidence,
                            ..config::DEFAULT_INPUT
                        })
                        .is_ok()
                    );
                }
            }
        }
    }
}

#[test]
fn schema_1_keeps_its_original_grammar_and_trusted_behavior() {
    let legacy = DEFAULT
        .lines()
        .filter(|line| !line.starts_with("sela_confidence ="))
        .collect::<Vec<_>>()
        .join("\n")
        .replace("schema_version = 2", "schema_version = 1");
    let expected = config::validate(config::DEFAULT_INPUT).unwrap();
    assert_eq!(
        config_build::validate_source(&legacy, "legacy.toml").unwrap(),
        expected
    );
    assert_eq!(
        config::validate(config::Input {
            schema_version: 1,
            ..config::DEFAULT_INPUT
        })
        .unwrap(),
        expected
    );
    let generated = config_build::compile(&legacy, "legacy.toml").unwrap();
    assert!(generated.contains("schema_version: 1,"));
    assert!(generated.contains("sela_confidence: \"trusted\","));
    for value in ["trusted", "unverified"] {
        let with_new_key = format!("{legacy}\nsela_confidence = \"{value}\"\n");
        let diagnostics = config_build::compile(&with_new_key, "legacy.toml").unwrap_err();
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "unknown_field");
        assert_eq!(diagnostics[0].field, Some(config::Field::SelaConfidence));
        assert_eq!(
            &with_new_key[diagnostics[0].span.clone()],
            format!("\"{value}\"")
        );
    }
    let diagnostics = config::validate(config::Input {
        schema_version: 1,
        sela_confidence: "unverified",
        ..config::DEFAULT_INPUT
    })
    .unwrap_err();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].field, config::Field::SelaConfidence);
    assert_eq!(
        diagnostics[0].code,
        config::ValidationCode::UnsupportedConfidence
    );
}

#[test]
fn schema_2_requires_confidence_and_rejects_unsupported_versions() {
    for (value, confidence) in [
        ("trusted", config::Confidence::Trusted),
        ("unverified", config::Confidence::Unverified),
    ] {
        let source = replace("sela_confidence", &format!("\"{value}\""));
        let validated = config_build::validate_source(&source, "confidence.toml").unwrap();
        assert_eq!(validated.confidence(), confidence);
        assert_eq!(confidence.as_str(), value);
        assert_eq!(confidence.id(), if value == "trusted" { 1 } else { 2 });
        assert!(
            config_build::compile(&source, "confidence.toml")
                .unwrap()
                .contains(&format!("sela_confidence: \"{value}\","))
        );
    }
    let without_key = DEFAULT
        .lines()
        .filter(|line| !line.starts_with("sela_confidence ="))
        .collect::<Vec<_>>()
        .join("\n");
    let diagnostics = config_build::compile(&without_key, "missing.toml").unwrap_err();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "missing_field");
    assert_eq!(diagnostics[0].field, Some(config::Field::SelaConfidence));
    for version in [0, 3, u64::MAX] {
        for source in [DEFAULT.to_string(), without_key.clone()] {
            let source =
                source.replace("schema_version = 2", &format!("schema_version = {version}"));
            let diagnostics = config_build::compile(&source, "version.toml").unwrap_err();
            assert_eq!(diagnostics.len(), 1);
            assert_eq!(diagnostics[0].code, "unsupported_version");
            assert_eq!(diagnostics[0].field, Some(config::Field::SchemaVersion));
        }
    }
    let escaped = replace("sela_confidence", "\"tr\\u0075sted\"");
    assert_eq!(
        config_build::compile(&escaped, "escaped.toml").unwrap(),
        config_build::compile(DEFAULT, "plain.toml").unwrap()
    );
}

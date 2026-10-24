//! Build-time TOML boundary; the simulation itself needs no parser dependency.

use std::{fmt, ops::Range};

use serde::Deserialize;
use toml::Spanned;

use crate::config::{self, Field, Input};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    schema_version: Spanned<u64>,
    policy_set: Spanned<String>,
    trace_capacity: Spanned<u64>,
    report_to_intake_ticks: Spanned<u64>,
    report_to_technician_ticks: Spanned<u64>,
    technician_to_intake_ticks: Spanned<u64>,
    technician_to_collection_ticks: Spanned<u64>,
    cable_receipt_at_tick: Spanned<u64>,
}

impl Raw {
    fn input(&self) -> Input<'_> {
        Input {
            schema_version: *self.schema_version.get_ref(),
            policy_set: self.policy_set.get_ref(),
            trace_capacity: *self.trace_capacity.get_ref(),
            report_to_intake_ticks: *self.report_to_intake_ticks.get_ref(),
            report_to_technician_ticks: *self.report_to_technician_ticks.get_ref(),
            technician_to_intake_ticks: *self.technician_to_intake_ticks.get_ref(),
            technician_to_collection_ticks: *self.technician_to_collection_ticks.get_ref(),
            cable_receipt_at_tick: *self.cable_receipt_at_tick.get_ref(),
        }
    }

    fn span(&self, field: Field) -> Range<usize> {
        match field {
            Field::SchemaVersion => self.schema_version.span(),
            Field::PolicySet => self.policy_set.span(),
            Field::TraceCapacity => self.trace_capacity.span(),
            Field::ReportToIntakeTicks => self.report_to_intake_ticks.span(),
            Field::ReportToTechnicianTicks => self.report_to_technician_ticks.span(),
            Field::TechnicianToIntakeTicks => self.technician_to_intake_ticks.span(),
            Field::TechnicianToCollectionTicks => self.technician_to_collection_ticks.span(),
            Field::CableReceiptAtTick => self.cable_receipt_at_tick.span(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    pub code: &'static str,
    pub field: Option<Field>,
    pub filename: String,
    pub span: Range<usize>,
    /// One-based source line and character column.
    pub line: usize,
    pub column: usize,
    pub message: String,
}

impl Diagnostic {
    fn new(
        source: &str,
        filename: &str,
        code: &'static str,
        field: Option<Field>,
        span: Range<usize>,
        message: String,
    ) -> Self {
        let prefix = &source[..span.start];
        Self {
            code,
            field,
            filename: filename.into(),
            line: prefix.bytes().filter(|byte| *byte == b'\n').count() + 1,
            column: prefix.rsplit('\n').next().unwrap_or("").chars().count() + 1,
            span,
            message,
        }
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}:{} [{}]",
            self.filename, self.line, self.column, self.code
        )?;
        if let Some(field) = self.field {
            write!(f, " {field}:")?;
        }
        write!(
            f,
            " {} (bytes {}..{})",
            self.message, self.span.start, self.span.end
        )
    }
}

fn parsed(source: &str, filename: &str) -> Result<(Raw, config::ValidatedConfig), Vec<Diagnostic>> {
    let raw: Raw = toml::from_str(source).map_err(|error: toml::de::Error| {
        let message = error.message().to_owned();
        let code = if message.starts_with("unknown field") {
            "unknown_field"
        } else if message.starts_with("missing field") {
            "missing_field"
        } else if message.starts_with("invalid type") || message.starts_with("invalid value") {
            "invalid_type"
        } else {
            "syntax"
        };
        vec![Diagnostic::new(
            source,
            filename,
            code,
            None,
            error.span().unwrap_or(0..0),
            message,
        )]
    })?;
    let input = raw.input();
    let validated = config::validate(input).map_err(|errors| {
        errors
            .into_iter()
            .map(|error| {
                Diagnostic::new(
                    source,
                    filename,
                    error.code.as_str(),
                    Some(error.field),
                    raw.span(error.field),
                    error.message,
                )
            })
            .collect::<Vec<_>>()
    })?;

    Ok((raw, validated))
}

/// Build/tooling-only entry point for checking the actual source configuration.
#[cfg(test)]
pub fn validate_source(
    source: &str,
    filename: &str,
) -> Result<config::ValidatedConfig, Vec<Diagnostic>> {
    parsed(source, filename).map(|(_, config)| config)
}

#[cfg_attr(test, allow(dead_code))]
pub fn compile(source: &str, filename: &str) -> Result<String, Vec<Diagnostic>> {
    let (raw, _) = parsed(source, filename)?;
    let input = raw.input();
    // Only validated integers and the fixed allowlisted policy reach Rust output.
    Ok(format!(
        "pub fn generated_config() -> crate::scene::config::ValidatedConfig {{\n    crate::scene::config::build_validated(crate::scene::config::Input {{\n        schema_version: {},\n        policy_set: {:?},\n        trace_capacity: {},\n        report_to_intake_ticks: {},\n        report_to_technician_ticks: {},\n        technician_to_intake_ticks: {},\n        technician_to_collection_ticks: {},\n        cable_receipt_at_tick: {},\n    }})\n}}\n",
        input.schema_version,
        input.policy_set,
        input.trace_capacity,
        input.report_to_intake_ticks,
        input.report_to_technician_ticks,
        input.technician_to_intake_ticks,
        input.technician_to_collection_ticks,
        input.cable_receipt_at_tick,
    ))
}

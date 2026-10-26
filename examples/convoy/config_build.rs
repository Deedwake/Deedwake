//! Build-time TOML boundary; the simulation itself needs no parser dependency.

use std::{fmt, ops::Range};

use serde::Deserialize;
use toml::Spanned;

use crate::config::{self, Field, Input};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    schema_version: Spanned<u64>,
    profile_id: Spanned<String>,
    observer: Spanned<String>,
    retention: Spanned<String>,
    reaction: Spanned<String>,
    sela_confidence: Option<Spanned<String>>,
    trace_capacity: Spanned<u64>,
    initial_delivery_ticks: Spanned<u64>,
    routine_delivery_ticks: Spanned<u64>,
    intermediary_delivery_ticks: Spanned<u64>,
}

impl Raw {
    fn input(&self) -> Input<'_> {
        Input {
            schema_version: *self.schema_version.get_ref(),
            profile_id: self.profile_id.get_ref(),
            observer: self.observer.get_ref(),
            retention: self.retention.get_ref(),
            reaction: self.reaction.get_ref(),
            sela_confidence: self
                .sela_confidence
                .as_ref()
                .map_or("trusted", |value| value.get_ref()),
            trace_capacity: *self.trace_capacity.get_ref(),
            initial_delivery_ticks: *self.initial_delivery_ticks.get_ref(),
            routine_delivery_ticks: *self.routine_delivery_ticks.get_ref(),
            intermediary_delivery_ticks: *self.intermediary_delivery_ticks.get_ref(),
        }
    }

    fn span(&self, field: Field) -> Range<usize> {
        match field {
            Field::SchemaVersion => self.schema_version.span(),
            Field::ProfileId => self.profile_id.span(),
            Field::Observer => self.observer.span(),
            Field::Retention => self.retention.span(),
            Field::Reaction => self.reaction.span(),
            Field::SelaConfidence => self
                .sela_confidence
                .as_ref()
                .map_or_else(|| self.schema_version.span(), Spanned::span),
            Field::TraceCapacity => self.trace_capacity.span(),
            Field::InitialDeliveryTicks => self.initial_delivery_ticks.span(),
            Field::RoutineDeliveryTicks => self.routine_delivery_ticks.span(),
            Field::IntermediaryDeliveryTicks => self.intermediary_delivery_ticks.span(),
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
    // The optional raw field preserves schema 1's original grammar.
    let boundary_error = match (*raw.schema_version.get_ref(), &raw.sela_confidence) {
        (1, Some(value)) => Some((
            "unknown_field",
            value.span(),
            "unknown field `sela_confidence` for schema_version 1",
        )),
        (2, None) => Some((
            "missing_field",
            raw.schema_version.span(),
            "missing field `sela_confidence` for schema_version 2",
        )),
        _ => None,
    };
    if let Some((code, span, message)) = boundary_error {
        return Err(vec![Diagnostic::new(
            source,
            filename,
            code,
            Some(Field::SelaConfidence),
            span,
            message.into(),
        )]);
    }
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
#[allow(dead_code)] // Shared by the standalone tool; the build script uses compile.
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
    // Rust debug formatting quotes strings; only validated bounded inputs reach output.
    Ok(format!(
        "pub fn generated_config() -> crate::scene::config::ValidatedConfig {{\n    crate::scene::config::build_validated(crate::scene::config::Input {{\n        schema_version: {},\n        profile_id: {:?},\n        observer: {:?},\n        retention: {:?},\n        reaction: {:?},\n        sela_confidence: {:?},\n        trace_capacity: {},\n        initial_delivery_ticks: {},\n        routine_delivery_ticks: {},\n        intermediary_delivery_ticks: {},\n    }})\n}}\n",
        input.schema_version,
        input.profile_id,
        input.observer,
        input.retention,
        input.reaction,
        input.sela_confidence,
        input.trace_capacity,
        input.initial_delivery_ticks,
        input.routine_delivery_ticks,
        input.intermediary_delivery_ticks,
    ))
}

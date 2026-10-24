//! Format-independent bounds for the synthetic repair host.

use std::fmt;

pub const MAX_TIME: u32 = 1000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Input<'a> {
    pub schema_version: u64,
    pub policy_set: &'a str,
    pub trace_capacity: u64,
    pub report_to_intake_ticks: u64,
    pub report_to_technician_ticks: u64,
    pub technician_to_intake_ticks: u64,
    pub technician_to_collection_ticks: u64,
    pub cable_receipt_at_tick: u64,
}

pub const DEFAULT_INPUT: Input<'static> = Input {
    schema_version: 1,
    policy_set: "repair_cafe_v1",
    trace_capacity: 128,
    report_to_intake_ticks: 1,
    report_to_technician_ticks: 2,
    technician_to_intake_ticks: 1,
    technician_to_collection_ticks: 2,
    cable_receipt_at_tick: 4,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    SchemaVersion,
    PolicySet,
    TraceCapacity,
    ReportToIntakeTicks,
    ReportToTechnicianTicks,
    TechnicianToIntakeTicks,
    TechnicianToCollectionTicks,
    CableReceiptAtTick,
}

impl Field {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SchemaVersion => "schema_version",
            Self::PolicySet => "policy_set",
            Self::TraceCapacity => "trace_capacity",
            Self::ReportToIntakeTicks => "report_to_intake_ticks",
            Self::ReportToTechnicianTicks => "report_to_technician_ticks",
            Self::TechnicianToIntakeTicks => "technician_to_intake_ticks",
            Self::TechnicianToCollectionTicks => "technician_to_collection_ticks",
            Self::CableReceiptAtTick => "cable_receipt_at_tick",
        }
    }
}

impl fmt::Display for Field {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValidationCode {
    UnsupportedVersion,
    UnsupportedPolicy,
    OutOfRange,
    Overflow,
}

impl ValidationCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UnsupportedVersion => "unsupported_version",
            Self::UnsupportedPolicy => "unsupported_policy",
            Self::OutOfRange => "out_of_range",
            Self::Overflow => "overflow",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidationError {
    pub field: Field,
    pub code: ValidationCode,
    pub message: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ValidatedConfig {
    trace_capacity: usize,
    report_to_intake_ticks: u64,
    report_to_technician_ticks: u64,
    technician_to_intake_ticks: u64,
    technician_to_collection_ticks: u64,
    cable_receipt_at_tick: u64,
}

impl ValidatedConfig {
    pub const fn trace_capacity(self) -> usize {
        self.trace_capacity
    }

    pub const fn report_to_intake_ticks(self) -> u64 {
        self.report_to_intake_ticks
    }

    pub const fn report_to_technician_ticks(self) -> u64 {
        self.report_to_technician_ticks
    }

    pub const fn technician_to_intake_ticks(self) -> u64 {
        self.technician_to_intake_ticks
    }

    pub const fn technician_to_collection_ticks(self) -> u64 {
        self.technician_to_collection_ticks
    }

    pub const fn cable_receipt_at_tick(self) -> u64 {
        self.cable_receipt_at_tick
    }
}

pub fn validate(input: Input<'_>) -> Result<ValidatedConfig, Vec<ValidationError>> {
    let mut errors = Vec::new();
    let mut reject = |field, code, message: &str| {
        errors.push(ValidationError {
            field,
            code,
            message: message.into(),
        });
    };
    if input.schema_version != 1 {
        reject(
            Field::SchemaVersion,
            ValidationCode::UnsupportedVersion,
            "schema_version must be 1",
        );
    }
    if input.policy_set != "repair_cafe_v1" {
        reject(
            Field::PolicySet,
            ValidationCode::UnsupportedPolicy,
            "policy_set must be repair_cafe_v1",
        );
    }
    // A fixed 1024-entry budget bounds trace memory even for external inputs.
    if input.trace_capacity > 1024 {
        reject(
            Field::TraceCapacity,
            ValidationCode::OutOfRange,
            "trace_capacity must be between 0 and 1024",
        );
    }
    for (field, delay) in [
        (Field::ReportToIntakeTicks, input.report_to_intake_ticks),
        (
            Field::ReportToTechnicianTicks,
            input.report_to_technician_ticks,
        ),
        (
            Field::TechnicianToIntakeTicks,
            input.technician_to_intake_ticks,
        ),
        (
            Field::TechnicianToCollectionTicks,
            input.technician_to_collection_ticks,
        ),
    ] {
        if u64::from(MAX_TIME).checked_add(delay).is_none() {
            reject(
                field,
                ValidationCode::Overflow,
                "delay overflows the simulation clock when added to MAX_TIME",
            );
        } else if delay > u64::from(MAX_TIME) {
            reject(
                field,
                ValidationCode::OutOfRange,
                "delay must be between 0 and 1000 ticks",
            );
        }
    }
    if input.cable_receipt_at_tick > u64::from(MAX_TIME) {
        reject(
            Field::CableReceiptAtTick,
            ValidationCode::OutOfRange,
            "cable_receipt_at_tick must be between 0 and 1000",
        );
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    Ok(ValidatedConfig {
        trace_capacity: input.trace_capacity as usize,
        report_to_intake_ticks: input.report_to_intake_ticks,
        report_to_technician_ticks: input.report_to_technician_ticks,
        technician_to_intake_ticks: input.technician_to_intake_ticks,
        technician_to_collection_ticks: input.technician_to_collection_ticks,
        cable_receipt_at_tick: input.cable_receipt_at_tick,
    })
}

/// Generated code delegates to the same validator. Keep the panic location in
/// tracked source rather than embedding OUT_DIR paths in the runtime artifact.
pub fn build_validated(input: Input<'_>) -> ValidatedConfig {
    validate(input).expect("build-validated repair configuration")
}

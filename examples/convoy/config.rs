//! Format-independent bounds for the synthetic convoy host.

use std::fmt;

pub const MAX_TIME: u32 = 1000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Observer {
    Mara,
    Sela,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Retention {
    Precise,
    Incomplete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reaction {
    RestrictUntilCorrection,
    WarnAndContinue,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Confidence {
    Trusted,
    Unverified,
}

impl Confidence {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Trusted => "trusted",
            Self::Unverified => "unverified",
        }
    }
    pub const fn id(self) -> u32 {
        match self {
            Self::Trusted => 1,
            Self::Unverified => 2,
        }
    }
}

impl Observer {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Mara => "mara",
            Self::Sela => "sela",
        }
    }
    pub const fn id(self) -> u32 {
        match self {
            Self::Mara => 1,
            Self::Sela => 2,
        }
    }
}

impl Retention {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Precise => "precise",
            Self::Incomplete => "incomplete",
        }
    }
    pub const fn id(self) -> u32 {
        match self {
            Self::Precise => 1,
            Self::Incomplete => 2,
        }
    }
}

impl Reaction {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RestrictUntilCorrection => "restrict_until_correction",
            Self::WarnAndContinue => "warn_and_continue",
        }
    }
    pub const fn id(self) -> u32 {
        match self {
            Self::RestrictUntilCorrection => 1,
            Self::WarnAndContinue => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Input<'a> {
    pub schema_version: u64,
    pub profile_id: &'a str,
    pub observer: &'a str,
    pub retention: &'a str,
    pub reaction: &'a str,
    pub sela_confidence: &'a str,
    pub trace_capacity: u64,
    pub initial_delivery_ticks: u64,
    pub routine_delivery_ticks: u64,
    pub intermediary_delivery_ticks: u64,
}

pub const DEFAULT_INPUT: Input<'static> = Input {
    schema_version: 2,
    profile_id: "configured-convoy-v1",
    observer: "mara",
    retention: "incomplete",
    reaction: "restrict_until_correction",
    sela_confidence: "trusted",
    trace_capacity: 128,
    initial_delivery_ticks: 3,
    routine_delivery_ticks: 3,
    intermediary_delivery_ticks: 1,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    SchemaVersion,
    ProfileId,
    Observer,
    Retention,
    Reaction,
    SelaConfidence,
    TraceCapacity,
    InitialDeliveryTicks,
    RoutineDeliveryTicks,
    IntermediaryDeliveryTicks,
}

impl Field {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SchemaVersion => "schema_version",
            Self::ProfileId => "profile_id",
            Self::Observer => "observer",
            Self::Retention => "retention",
            Self::Reaction => "reaction",
            Self::SelaConfidence => "sela_confidence",
            Self::TraceCapacity => "trace_capacity",
            Self::InitialDeliveryTicks => "initial_delivery_ticks",
            Self::RoutineDeliveryTicks => "routine_delivery_ticks",
            Self::IntermediaryDeliveryTicks => "intermediary_delivery_ticks",
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
    InvalidProfileId,
    UnsupportedObserver,
    UnsupportedRetention,
    UnsupportedReaction,
    UnsupportedConfidence,
    OutOfRange,
    Overflow,
}

impl ValidationCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UnsupportedVersion => "unsupported_version",
            Self::InvalidProfileId => "invalid_profile_id",
            Self::UnsupportedObserver => "unsupported_observer",
            Self::UnsupportedRetention => "unsupported_retention",
            Self::UnsupportedReaction => "unsupported_reaction",
            Self::UnsupportedConfidence => "unsupported_confidence",
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidatedConfig {
    profile_id: String,
    observer: Observer,
    retention: Retention,
    reaction: Reaction,
    confidence: Confidence,
    trace_capacity: usize,
    initial_delivery_ticks: u32,
    routine_delivery_ticks: u32,
    intermediary_delivery_ticks: u32,
}

impl ValidatedConfig {
    pub fn profile_id(&self) -> &str {
        &self.profile_id
    }
    pub const fn observer(&self) -> Observer {
        self.observer
    }
    pub const fn retention(&self) -> Retention {
        self.retention
    }
    pub const fn reaction(&self) -> Reaction {
        self.reaction
    }
    pub const fn confidence(&self) -> Confidence {
        self.confidence
    }
    pub const fn trace_capacity(&self) -> usize {
        self.trace_capacity
    }
    pub const fn initial_delivery_ticks(&self) -> u32 {
        self.initial_delivery_ticks
    }
    pub const fn routine_delivery_ticks(&self) -> u32 {
        self.routine_delivery_ticks
    }
    pub const fn intermediary_delivery_ticks(&self) -> u32 {
        self.intermediary_delivery_ticks
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
    if !matches!(input.schema_version, 1 | 2) {
        reject(
            Field::SchemaVersion,
            ValidationCode::UnsupportedVersion,
            "schema_version must be 1 or 2",
        );
    }
    if input.profile_id.is_empty()
        || input.profile_id.len() > 64
        || !input.profile_id.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
        })
    {
        reject(
            Field::ProfileId,
            ValidationCode::InvalidProfileId,
            "profile_id must contain 1..64 ASCII lowercase letters, digits, underscores or hyphens",
        );
    }
    let observer = match input.observer {
        "mara" => Some(Observer::Mara),
        "sela" => Some(Observer::Sela),
        _ => {
            reject(
                Field::Observer,
                ValidationCode::UnsupportedObserver,
                "observer must be mara or sela",
            );
            None
        }
    };
    let retention = match input.retention {
        "precise" => Some(Retention::Precise),
        "incomplete" => Some(Retention::Incomplete),
        _ => {
            reject(
                Field::Retention,
                ValidationCode::UnsupportedRetention,
                "retention must be precise or incomplete",
            );
            None
        }
    };
    let reaction = match input.reaction {
        "restrict_until_correction" => Some(Reaction::RestrictUntilCorrection),
        "warn_and_continue" => Some(Reaction::WarnAndContinue),
        _ => {
            reject(
                Field::Reaction,
                ValidationCode::UnsupportedReaction,
                "reaction must be restrict_until_correction or warn_and_continue",
            );
            None
        }
    };
    let confidence = match input.sela_confidence {
        "trusted" => Some(Confidence::Trusted),
        "unverified" if input.schema_version != 1 => Some(Confidence::Unverified),
        _ => {
            reject(
                Field::SelaConfidence,
                ValidationCode::UnsupportedConfidence,
                if input.schema_version == 1 {
                    "schema_version 1 requires trusted sela_confidence"
                } else {
                    "sela_confidence must be trusted or unverified"
                },
            );
            None
        }
    };
    if input.trace_capacity > 1024 {
        reject(
            Field::TraceCapacity,
            ValidationCode::OutOfRange,
            "trace_capacity must be between 0 and 1024",
        );
    }
    for (field, delay) in [
        (Field::InitialDeliveryTicks, input.initial_delivery_ticks),
        (Field::RoutineDeliveryTicks, input.routine_delivery_ticks),
        (
            Field::IntermediaryDeliveryTicks,
            input.intermediary_delivery_ticks,
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
    if !errors.is_empty() {
        return Err(errors);
    }
    Ok(ValidatedConfig {
        profile_id: input.profile_id.into(),
        observer: observer.expect("validated observer"),
        retention: retention.expect("validated retention"),
        reaction: reaction.expect("validated reaction"),
        confidence: confidence.expect("validated confidence"),
        trace_capacity: input.trace_capacity as usize,
        initial_delivery_ticks: input.initial_delivery_ticks as u32,
        routine_delivery_ticks: input.routine_delivery_ticks as u32,
        intermediary_delivery_ticks: input.intermediary_delivery_ticks as u32,
    })
}

/// Keep build validation and runtime construction on the same bounded path.
pub fn build_validated(input: Input<'_>) -> ValidatedConfig {
    validate(input).expect("build-validated convoy configuration")
}

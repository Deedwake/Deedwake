//! Matched authoring comparison for the closed, all-available Convoy scenario.
#[allow(dead_code)]
#[path = "../../examples/convoy/flags.rs"]
mod flags;
use flags::{FlagsConvoy, config};
#[allow(dead_code)]
#[path = "../../examples/convoy/config_build.rs"]
mod config_build;
use serde_json::{Value, json};
use std::{
    ffi::OsString,
    fs::File,
    io::{self, Read, Write},
    path::PathBuf,
};

const HELP: &str = "convoy-baseline --config PATH [--actions 0,1,...,6] [--json] [--trace]\nIndependent flags baseline, all entities loaded and available. No save/load or lifecycle controls.\nActions: 0 reset, 1 hold, 2 ask, 3 release, 4 handoff, 5 visit, 6 wait; at most 2128.\n--trace is baseline flag/route inspection, not a Core trace.\nExit: 0 success, 1 unavailable action, 2 configuration/output failure, 64 usage.";
struct Args {
    config: PathBuf,
    actions: Vec<u32>,
    trace: bool,
}
fn parse(raw: &[OsString]) -> Result<Args, &'static str> {
    if raw.len() > 6 || raw.iter().any(|value| value.len() > 8192) {
        return Err("arguments exceed count or 8192-byte argument limit");
    }
    let mut path = None;
    let mut actions = None;
    let mut trace = false;
    let mut json = false;
    let mut args = raw.iter();
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--config") if path.is_none() => {
                let value = args.next().ok_or("--config requires a path")?;
                if value.is_empty() || value.to_string_lossy().starts_with('-') {
                    return Err("expected configuration path");
                }
                path = Some(PathBuf::from(value));
            }
            Some("--actions") if actions.is_none() => {
                let value = args
                    .next()
                    .and_then(|value| value.to_str())
                    .ok_or("--actions requires comma-separated action IDs 0..6")?;
                let mut commands = Vec::new();
                for command in value.split(',') {
                    if commands.len() == 2128 {
                        return Err("--actions supports at most 2128 commands");
                    }
                    commands.push(match command {
                        "0" => 0,
                        "1" => 1,
                        "2" => 2,
                        "3" => 3,
                        "4" => 4,
                        "5" => 5,
                        "6" => 6,
                        _ => return Err("--actions requires comma-separated action IDs 0..6"),
                    });
                }
                actions = Some(commands);
            }
            Some("--json") if !json => json = true,
            Some("--trace") if !trace => trace = true,
            _ => return Err("unrecognized or repeated option"),
        }
    }
    Ok(Args {
        config: path.ok_or("--config PATH is required")?,
        actions: actions.unwrap_or_default(),
        trace,
    })
}
fn configuration(path: &PathBuf) -> Result<config::ValidatedConfig, Value> {
    let file = File::open(path).map_err(|e| json!([{"code":"config_read","message":format!("cannot open configuration: {}", e.kind())}]))?;
    let mut bytes = vec![];
    file.take(65537).read_to_end(&mut bytes).map_err(|e| json!([{"code":"config_read","message":format!("cannot read configuration: {}", e.kind())}]))?;
    if bytes.len() > 65536 {
        return Err(json!([{"code":"config_size","message":"configuration exceeds 65536 bytes"}]));
    }
    let source = std::str::from_utf8(&bytes)
        .map_err(|_| json!([{"code":"invalid_utf8","message":"configuration is not UTF-8"}]))?;
    config_build::validate_source(source, &path.to_string_lossy()).map_err(|errors| Value::Array(errors.into_iter().map(|e| json!({"code":e.code,"field":e.field.map(config::Field::as_str),"filename":e.filename,"line":e.line,"column":e.column,"span":[e.span.start,e.span.end],"message":e.message})).collect()))
}
fn run(raw: &[OsString]) -> (Value, i32) {
    let mut report = json!({"schema_version":1,"implementation":"flags_baseline","ok":false,"diagnostics":[],"steps":[]});
    let args = match parse(raw) {
        Ok(args) => args,
        Err(message) => {
            report["diagnostics"] = json!([{"code":"usage","message":message}]);
            return (report, 64);
        }
    };
    let config = match configuration(&args.config) {
        Ok(config) => config,
        Err(diagnostics) => {
            report["diagnostics"] = diagnostics;
            return (report, 2);
        }
    };
    let mut convoy = FlagsConvoy::with_config(config);
    let mut unavailable = false;
    let mut steps = vec![];
    for action in args.actions {
        let result = convoy.act(action);
        unavailable |= result != 0;
        steps.push(json!({"action":action,"result":result,"time":convoy.time(),"dialogue":convoy.output(0).unwrap()}));
    }
    report["ok"] = json!(!unavailable);
    report["steps"] = json!(steps);
    report["final"] = json!({"time":convoy.time(),"dialogue":convoy.output(0).unwrap(),"journal":convoy.output(1).unwrap().lines().collect::<Vec<_>>(),"enabled":(0..=6).map(|id|convoy.enabled(id)).collect::<Vec<_>>()});
    if args.trace {
        report["trace"] = json!(convoy.debug_trace());
    }
    (report, if unavailable { 1 } else { 0 })
}
fn main() {
    let raw: Vec<_> = std::env::args_os().skip(1).collect();
    if raw.len() == 1 && raw[0] == "--help" {
        println!("{HELP}");
        return;
    }
    let json_requested = raw.iter().any(|value| value == "--json");
    let (report, exit) = run(&raw);
    let mut output = io::stdout().lock();
    let written = if json_requested {
        serde_json::to_writer_pretty(&mut output, &report).map_err(io::Error::other)
    } else {
        writeln!(
            output,
            "Convoy independent flags baseline\n{}",
            serde_json::to_string_pretty(&report).unwrap()
        )
    };
    if written.and_then(|_| writeln!(output)).is_err() {
        std::process::exit(2);
    }
    std::process::exit(exit);
}

#[cfg(test)]
mod tests {
    use super::*;
    fn args(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }
    #[test]
    fn cli_accepts_only_bounded_explicit_actions_and_one_config() {
        let parse_actions = |value: &str| {
            parse(&args(&[
                "--config",
                "input.toml",
                "--actions",
                value,
                "--json",
                "--trace",
            ]))
        };
        assert_eq!(
            parse_actions("0,1,2,3,4,5,6").unwrap().actions,
            vec![0, 1, 2, 3, 4, 5, 6]
        );
        for value in ["", "01", "7", "1,", "1, 2", "-1"] {
            assert!(parse_actions(value).is_err());
        }
        assert!(parse_actions(&vec!["6"; 2128].join(",")).is_ok());
        assert!(parse_actions(&vec!["6"; 2129].join(",")).is_err());
        for values in [
            vec![],
            vec!["--config"],
            vec!["--config", "a", "--config", "b"],
            vec!["--config", "a", "--json", "--json"],
        ] {
            assert!(parse(&args(&values)).is_err());
        }
    }
    #[test]
    fn invalid_configuration_and_usage_have_structured_failure_reports() {
        let (report, exit) = run(&args(&["--actions", "7"]));
        assert_eq!(exit, 64);
        assert_eq!(report["implementation"], "flags_baseline");
        assert_eq!(report["diagnostics"][0]["code"], "usage");
        let (report, exit) = run(&args(&["--config", "/missing-convoy-baseline-config"]));
        assert_eq!(exit, 2);
        assert_eq!(report["diagnostics"][0]["code"], "config_read");
    }
}

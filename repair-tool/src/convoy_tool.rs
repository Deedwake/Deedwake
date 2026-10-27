//! Native host runner for configured Convoy sessions; no external effects.
#[allow(dead_code)]
#[path = "../../examples/convoy/scene.rs"]
mod scene;
use scene::{Convoy, ProviderDelivery, config};
#[allow(dead_code)]
#[path = "../../examples/convoy/config_build.rs"]
mod config_build;
mod convoy_session;
mod session_io;
use deedwake_core::{Availability, DeliveryRejection, DeliveryState, EntityState, Residency};
use serde_json::{Value, json};
use std::{
    ffi::OsString,
    fs::File,
    io::{self, Read, Write},
    path::PathBuf,
};

const HELP: &str = "convoy-tool (--config PATH | --load SNAPSHOT) [--action NAME | --provider STATE]... [--save SNAPSHOT] [--json] [--trace]\nconvoy-tool --help\nActions: reset, hold, ask, release, handoff, visit, wait.\nProvider states: loaded, unloaded, unavailable, incapable, retired.\nProvider operations are privileged host controls, not player choices. They never advance time or execute work.\nOperations run in argument order, up to 2128. Loading alone is inert.\nConfigured sessions only; no migrations, external effects or concurrent saves.\nExit: 0 success, 1 operation unavailable, 2 configuration/load/save failure, 64 usage.\n";
#[derive(Clone)]
enum Operation {
    Action(&'static str, u32),
    Provider(&'static str, EntityState),
}
struct Args {
    source: PathBuf,
    load: bool,
    save: Option<PathBuf>,
    operations: Vec<Operation>,
    json: bool,
    trace: bool,
}
fn parse(raw: &[OsString]) -> Result<Args, String> {
    if raw.len() > 4270 || raw.iter().any(|arg| arg.len() > 4096) {
        return Err("arguments exceed count or 4096-byte argument limit".into());
    }
    let mut args = Args {
        source: PathBuf::new(),
        load: false,
        save: None,
        operations: vec![],
        json: false,
        trace: false,
    };
    let mut source = false;
    let mut index = 0;
    while index < raw.len() {
        let flag = raw[index].to_str().ok_or("invalid option encoding")?;
        match flag {
            "--json" if !args.json => args.json = true,
            "--trace" if !args.trace => args.trace = true,
            "--config" | "--load" if !source => {
                index += 1;
                let path = raw.get(index).ok_or("source requires a path")?;
                if path.is_empty() || path.to_string_lossy().starts_with('-') {
                    return Err("expected source path".into());
                }
                args.source = path.into();
                args.load = flag == "--load";
                source = true;
            }
            "--save" if args.save.is_none() => {
                index += 1;
                let path = raw.get(index).ok_or("--save requires a path")?;
                if path.is_empty() || path.to_string_lossy().starts_with('-') {
                    return Err("expected destination path".into());
                }
                args.save = Some(path.into());
            }
            "--action" | "--provider" => {
                index += 1;
                let value = raw
                    .get(index)
                    .and_then(|x| x.to_str())
                    .ok_or("operation requires a name")?;
                if args.operations.len() == 2128 {
                    return Err("session exceeds 2128 operations".into());
                }
                let operation = if flag == "--action" {
                    match value {
                        "reset" => Operation::Action("reset", 0),
                        "hold" => Operation::Action("hold", 1),
                        "ask" => Operation::Action("ask", 2),
                        "release" => Operation::Action("release", 3),
                        "handoff" => Operation::Action("handoff", 4),
                        "visit" => Operation::Action("visit", 5),
                        "wait" => Operation::Action("wait", 6),
                        _ => return Err("unsupported action".into()),
                    }
                } else {
                    let (name, state) = match value {
                        "loaded" => (
                            "loaded",
                            EntityState::Present {
                                residency: Residency::Loaded,
                                availability: Availability::Available,
                            },
                        ),
                        "unloaded" => (
                            "unloaded",
                            EntityState::Present {
                                residency: Residency::Unloaded,
                                availability: Availability::Available,
                            },
                        ),
                        "unavailable" => (
                            "unavailable",
                            EntityState::Present {
                                residency: Residency::Loaded,
                                availability: Availability::Unavailable,
                            },
                        ),
                        "incapable" => (
                            "incapable",
                            EntityState::Present {
                                residency: Residency::Loaded,
                                availability: Availability::Incapable,
                            },
                        ),
                        "retired" => ("retired", EntityState::Retired),
                        _ => return Err("unsupported provider state".into()),
                    };
                    Operation::Provider(name, state)
                };
                args.operations.push(operation);
            }
            _ => return Err("unrecognized, misplaced or repeated option".into()),
        }
        index += 1;
    }
    if !source {
        return Err("exactly one --config PATH or --load SNAPSHOT is required".into());
    }
    Ok(args)
}
fn configuration(path: &PathBuf) -> Result<config::ValidatedConfig, Value> {
    let file=File::open(path).map_err(|e|json!([{"code":"config_read","message":format!("cannot open configuration: {}",e.kind())}]))?;
    let mut bytes = vec![];
    file.take(65537).read_to_end(&mut bytes).map_err(|e|json!([{"code":"config_read","message":format!("cannot read configuration: {}",e.kind())}]))?;
    if bytes.len() > 65536 {
        return Err(json!([{"code":"config_size","message":"configuration exceeds 65536 bytes"}]));
    }
    let text = std::str::from_utf8(&bytes)
        .map_err(|_| json!([{"code":"invalid_utf8","message":"configuration is not UTF-8"}]))?;
    config_build::validate_source(text,&path.to_string_lossy()).map_err(|errors| Value::Array(errors.into_iter().map(|e|json!({"code":e.code,"field":e.field.map(config::Field::as_str),"filename":e.filename,"line":e.line,"column":e.column,"span":[e.span.start,e.span.end],"message":e.message})).collect()))
}
fn base() -> Value {
    json!({"schema_version":1,"report_type":"convoy_session","privileged_host_report":true,"ok":true,"diagnostics":[],"steps":[]})
}
fn failure(mut report: Value, code: &str, message: String, exit: i32) -> (Value, i32) {
    report["ok"] = json!(false);
    report["diagnostics"] = json!([{"code":code,"message":message}]);
    (report, exit)
}
fn run(raw: &[OsString]) -> (Value, i32) {
    let report = base();
    let args = match parse(raw) {
        Ok(args) => args,
        Err(e) => return failure(report, "usage", e, 64),
    };
    let mut convoy = if args.load {
        match convoy_session::read(&args.source) {
            Ok(c) => c,
            Err(e) => return failure(report, "snapshot_load", e, 2),
        }
    } else {
        match configuration(&args.source) {
            Ok(c) => Convoy::with_config(c),
            Err(errors) => {
                let mut r = report;
                r["ok"] = json!(false);
                r["diagnostics"] = errors;
                return (r, 2);
            }
        }
    };
    let mut steps = vec![];
    let mut unavailable = false;
    for (index, operation) in args.operations.into_iter().enumerate() {
        let (kind, name, result, error) = match operation {
            Operation::Action(name, id) => ("action", name, convoy.act(id), None),
            Operation::Provider(name, state) => match convoy.set_provider_state(state) {
                Ok(()) => ("provider", name, 0, None),
                Err(e) => ("provider", name, 1, Some(format!("{e:?}"))),
            },
        };
        unavailable |= result != 0;
        steps.push(json!({"index":index+1,"kind":kind,"name":name,"result":result,"time":convoy.time(),"error":error}));
    }
    let mut report = report;
    report["steps"] = json!(steps);
    report["ok"] = json!(!unavailable);
    report["time"] = json!(convoy.time());
    report["dialogue"] = json!(convoy.output(0).unwrap());
    report["journal"] = json!(convoy.output(1).unwrap().lines().collect::<Vec<_>>());
    report["choices"] = json!(
        (1..=6)
            .map(|id| json!({"id":id,"enabled":convoy.enabled(id)}))
            .collect::<Vec<_>>()
    );
    report["provider"] = match convoy.provider_state().unwrap() {
        EntityState::Retired => json!({"state":"retired"}),
        EntityState::Present {
            residency,
            availability,
        } => {
            json!({"state":"present","residency":match residency{Residency::Loaded=>"loaded",Residency::Unloaded=>"unloaded"},"availability":match availability{Availability::Available=>"available",Availability::Unavailable=>"unavailable",Availability::Incapable=>"incapable"}})
        }
    };
    report["receipts"] = json!(
        [
            (ProviderDelivery::Report, "report"),
            (ProviderDelivery::Routine, "routine"),
            (ProviderDelivery::Intermediary, "intermediary")
        ]
        .into_iter()
        .map(|(kind, name)| {
            let Some(receipt) = convoy.provider_delivery_receipt(kind) else {
                return json!({"route":name,"receipt":null});
            };
            let (state, at, reason) = match receipt.state {
                DeliveryState::Pending => ("pending", None, None),
                DeliveryState::Delivered { at } => ("delivered", Some(at), None),
                DeliveryState::Rejected { at, reason } => (
                    "rejected",
                    Some(at),
                    Some(match reason {
                        DeliveryRejection::Retired => "retired",
                        DeliveryRejection::Unavailable => "unavailable",
                        DeliveryRejection::Incapable => "incapable",
                        DeliveryRejection::Unloaded => "unloaded",
                    }),
                ),
            };
            json!({"route":name,"state":state,"due":receipt.delivery.due,"at":at,"reason":reason})
        })
        .collect::<Vec<_>>()
    );
    if args.trace {
        report["trace"] = json!(convoy.output(2).unwrap());
    }
    if let Some(path) = args.save
        && let Err(error) = convoy_session::save(&convoy, &path)
    {
        let code = if matches!(error, session_io::SaveError::DurabilityUncertain(_)) {
            "snapshot_save_uncertain"
        } else {
            "snapshot_save"
        };
        return failure(report, code, error.to_string(), 2);
    }
    (report, if unavailable { 1 } else { 0 })
}
fn main() {
    let raw: Vec<_> = std::env::args_os().skip(1).collect();
    if raw.len() == 1 && raw[0] == "--help" {
        println!("{HELP}");
        return;
    }
    let json_requested = raw.iter().any(|x| x == "--json");
    let (report, exit) = run(&raw);
    let mut out = io::stdout().lock();
    let written = if json_requested {
        serde_json::to_writer_pretty(&mut out, &report).map_err(io::Error::other)
    } else {
        writeln!(
            out,
            "Convoy native session · privileged host report\n{}",
            serde_json::to_string_pretty(&report).unwrap()
        )
    };
    if written.and_then(|_| writeln!(out)).is_err() {
        std::process::exit(2)
    }
    std::process::exit(exit)
}

#[path = "convoy/scene.rs"]
pub mod scene;
use scene::{Condition, Convoy, Study};

struct Options {
    condition: Condition,
    intermediary: bool,
    trace: bool,
    study: Option<Study>,
    actions: Option<Vec<u32>>,
}
fn options(args: &[String]) -> Result<Options, &'static str> {
    let mut options = Options {
        condition: Condition::Current,
        intermediary: false,
        trace: false,
        study: None,
        actions: None,
    };
    let mut condition_selected = false;
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--condition" => {
                condition_selected = true;
                options.condition = match args.next().map(String::as_str) {
                    Some("current") => Condition::Current,
                    Some("control") => Condition::Control,
                    _ => return Err("--condition requires current or control"),
                };
            }
            "--route" => {
                options.intermediary = match args.next().map(String::as_str) {
                    Some("routine") => false,
                    Some("intermediary") => true,
                    _ => return Err("--route requires routine or intermediary"),
                };
            }
            "--study" => {
                options.study = Some(match args.next().map(String::as_str) {
                    Some("precise-immediate") => Study::PreciseImmediate,
                    Some("incomplete-immediate") => Study::IncompleteImmediate,
                    Some("precise-scheduled") => Study::PreciseScheduled,
                    Some("incomplete-scheduled") => Study::IncompleteScheduled,
                    _ => {
                        return Err(
                            "--study requires precise-immediate, incomplete-immediate, precise-scheduled or incomplete-scheduled",
                        );
                    }
                });
            }
            "--actions" => {
                let value = args
                    .next()
                    .ok_or("--actions requires comma-separated action IDs 0..6")?;
                let commands: Vec<_> = value.split(',').collect();
                if commands.len() > 2128 {
                    return Err("--actions supports at most 2128 commands");
                }
                options.actions = Some(
                    commands
                        .into_iter()
                        .map(|command| match command {
                            "0" => Ok(0),
                            "1" => Ok(1),
                            "2" => Ok(2),
                            "3" => Ok(3),
                            "4" => Ok(4),
                            "5" => Ok(5),
                            "6" => Ok(6),
                            _ => Err("--actions requires comma-separated action IDs 0..6"),
                        })
                        .collect::<Result<_, _>>()?,
                );
            }
            "--trace" => options.trace = true,
            _ => {
                return Err(
                    "usage: convoy [--condition current|control | --study precise-immediate|incomplete-immediate|precise-scheduled|incomplete-scheduled] [--route routine|intermediary] [--actions 0,1,...,6] [--trace]",
                );
            }
        }
    }
    if condition_selected && options.study.is_some() {
        return Err("--condition and --study cannot be combined");
    }
    Ok(options)
}
fn run(options: &Options, encounters: bool) {
    let mut scene = options.study.map_or_else(
        || Convoy::with_condition(options.condition),
        Convoy::with_study,
    );
    if let Some(actions) = &options.actions {
        for &action in actions {
            let result = scene.act(action);
            println!("Action {action}: result {result}");
            println!("Hour {}: {}", scene.time(), scene.output(0).unwrap());
        }
        println!("Journal:\n{}", scene.output(1).unwrap());
        println!(
            "Enabled: {:?}",
            std::array::from_fn::<_, 7, _>(|action| u32::from(scene.enabled(action as u32)))
        );
        if options.trace {
            println!("Trace:\n{}", scene.output(2).unwrap());
        }
        return;
    }
    let mut act = |action| {
        assert_eq!(scene.act(action), 0);
        if encounters && action != 6 {
            println!("Hour {}: {}", scene.time(), scene.output(0).unwrap());
        }
    };
    for action in [1, 2, 6, 6, 6, 5, 3] {
        act(action);
    }
    if options.intermediary {
        act(4);
    }
    let delay = if options.intermediary {
        options.study.map_or(
            options.condition.intermediary_delay(),
            Study::intermediary_delay,
        )
    } else {
        options
            .study
            .map_or(options.condition.routine_delay(), Study::routine_delay)
    };
    for _ in 0..delay {
        act(6);
    }
    act(5);
    if encounters {
        println!("Journal:\n{}", scene.output(1).unwrap());
    } else {
        println!("Hour {}: {}", scene.time(), scene.output(0).unwrap());
    }
    if options.trace {
        println!("Trace:\n{}", scene.output(2).unwrap());
    }
}
fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.is_empty() {
        for intermediary in [false, true] {
            run(
                &Options {
                    condition: Condition::Current,
                    intermediary,
                    trace: false,
                    study: None,
                    actions: None,
                },
                false,
            );
        }
    } else {
        match options(&args) {
            Ok(options) => run(&options, true),
            Err(message) => {
                eprintln!("{message}");
                std::process::exit(2);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_studies_and_action_lists_accept_only_compiled_bounded_inputs() {
        let parse =
            |args: &[&str]| options(&args.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        for (name, study) in [
            ("precise-immediate", Study::PreciseImmediate),
            ("incomplete-immediate", Study::IncompleteImmediate),
            ("precise-scheduled", Study::PreciseScheduled),
            ("incomplete-scheduled", Study::IncompleteScheduled),
        ] {
            let selected = parse(&["--study", name, "--actions", "0,1,2,3,4,5,6"]).unwrap();
            assert_eq!(selected.study, Some(study));
            assert_eq!(selected.actions, Some(vec![0, 1, 2, 3, 4, 5, 6]));
        }
        for args in [
            vec!["--study"],
            vec!["--study", "other"],
            vec!["--study", "precise-immediate", "--condition", "current"],
            vec!["--actions"],
            vec!["--actions", ""],
            vec!["--actions", "1,"],
            vec!["--actions", "7"],
            vec!["--actions", "01"],
            vec!["--actions", "1, 2"],
            vec!["--actions", "1+2"],
        ] {
            assert!(parse(&args).is_err());
        }
        let bounded = vec!["6"; 2128].join(",");
        assert!(parse(&["--actions", &bounded]).is_ok());
        let overlong = vec!["6"; 2129].join(",");
        assert!(parse(&["--actions", &overlong]).is_err());
    }

    #[test]
    fn native_selection_accepts_only_compiled_conditions_and_routes() {
        let parse =
            |args: &[&str]| options(&args.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        for (name, condition) in [
            ("current", Condition::Current),
            ("control", Condition::Control),
        ] {
            for route in ["routine", "intermediary"] {
                let options = parse(&["--condition", name, "--route", route, "--trace"]).unwrap();
                assert_eq!(options.condition, condition);
                assert_eq!(options.intermediary, route == "intermediary");
                assert!(options.trace);
            }
        }
        for args in [
            vec!["--condition"],
            vec!["--condition", "other"],
            vec!["--route"],
            vec!["--route", "other"],
            vec!["--unknown"],
        ] {
            assert!(parse(&args).is_err());
        }
    }
}

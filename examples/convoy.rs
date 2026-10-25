#[path = "convoy/scene.rs"]
pub mod scene;
use scene::{Condition, Convoy};

struct Options {
    condition: Condition,
    intermediary: bool,
    trace: bool,
}
fn options(args: &[String]) -> Result<Options, &'static str> {
    let mut options = Options {
        condition: Condition::Current,
        intermediary: false,
        trace: false,
    };
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--condition" => {
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
            "--trace" => options.trace = true,
            _ => {
                return Err(
                    "usage: convoy [--condition current|control] [--route routine|intermediary] [--trace]",
                );
            }
        }
    }
    Ok(options)
}
fn run(options: &Options, encounters: bool) {
    let mut scene = Convoy::with_condition(options.condition);
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
        options.condition.intermediary_delay()
    } else {
        options.condition.routine_delay()
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

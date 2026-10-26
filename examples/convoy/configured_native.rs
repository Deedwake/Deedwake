//! Native entry point for the same generated configuration as the browser example.
use deedwake_example_web::{configured_convoy, scene};

#[derive(Debug, Default)]
struct Options {
    actions: Option<Vec<u32>>,
    trace: bool,
}

fn options(args: &[String]) -> Result<Options, &'static str> {
    let mut options = Options::default();
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--trace" => options.trace = true,
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
            _ => return Err("usage: convoy_configured [--actions 0,1,...,6] [--trace]"),
        }
    }
    Ok(options)
}

fn run(options: Options) {
    let mut convoy = configured_convoy();
    if let Some(actions) = options.actions {
        for action in actions {
            let result = convoy.act(action);
            println!("Action {action}: result {result}");
            println!("Hour {}: {}", convoy.time(), convoy.output(0).unwrap());
        }
        println!("Journal:\n{}", convoy.output(1).unwrap());
        println!(
            "Enabled: {:?}",
            std::array::from_fn::<_, 7, _>(|action| u32::from(convoy.enabled(action as u32)))
        );
    } else {
        let config = convoy.config().expect("configured convoy");
        println!(
            "Profile {}: observer={}, retention={}, reaction={}, delays={}/{}/{}, trace_capacity={}",
            config.profile_id(),
            config.observer().as_str(),
            config.retention().as_str(),
            config.reaction().as_str(),
            config.initial_delivery_ticks(),
            config.routine_delivery_ticks(),
            config.intermediary_delivery_ticks(),
            config.trace_capacity()
        );
        let initial_delay = config.initial_delivery_ticks();
        let routine_delay = config.routine_delivery_ticks();
        let mut act = |action| {
            let result = convoy.act(action);
            if result != 0 {
                println!(
                    "Hour {}: Choice {action} unavailable (status {result}); state unchanged.",
                    convoy.time()
                );
            } else if action != 6 {
                println!("Hour {}: {}", convoy.time(), convoy.output(0).unwrap());
            }
        };
        for action in [1, 2] {
            act(action);
        }
        for _ in 0..initial_delay {
            act(6);
        }
        for action in [5, 3] {
            act(action);
        }
        for _ in 0..routine_delay.min(scene::config::MAX_TIME.saturating_sub(initial_delay)) {
            act(6);
        }
        act(5);
        println!("Journal:\n{}", convoy.output(1).unwrap());
    }
    if options.trace {
        println!("Trace:\n{}", convoy.output(2).unwrap());
    }
}

fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    match options(&args) {
        Ok(options) => run(options),
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actions_are_bounded_and_only_accept_explicit_ids() {
        let parse =
            |args: &[&str]| options(&args.iter().map(|arg| arg.to_string()).collect::<Vec<_>>());
        assert_eq!(
            parse(&["--actions", "0,1,2,3,4,5,6", "--trace"])
                .unwrap()
                .actions,
            Some(vec![0, 1, 2, 3, 4, 5, 6])
        );
        for args in [
            vec!["--unknown"],
            vec!["--actions"],
            vec!["--actions", ""],
            vec!["--actions", "1,"],
            vec!["--actions", "7"],
            vec!["--actions", "01"],
            vec!["--actions", "1, 2"],
        ] {
            assert!(parse(&args).is_err());
        }
        assert!(parse(&["--actions", &vec!["6"; 2128].join(",")]).is_ok());
        assert!(parse(&["--actions", &vec!["6"; 2129].join(",")]).is_err());
    }
}

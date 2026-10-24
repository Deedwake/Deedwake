#[allow(dead_code)]
#[path = "../examples/repair/config.rs"]
mod config;
#[path = "../examples/repair/config_build.rs"]
mod config_build;

fn main() {
    for input in [
        "build.rs",
        "repair.toml",
        "../examples/repair/config.rs",
        "../examples/repair/config_build.rs",
    ] {
        println!("cargo::rerun-if-changed={input}");
    }
    let bytes = std::fs::read("repair.toml").unwrap_or_else(|error| {
        eprintln!("repair.toml [read_error]: {error}");
        std::process::exit(1);
    });
    let source = std::str::from_utf8(&bytes).unwrap_or_else(|error| {
        let index = error.valid_up_to();
        let prefix = std::str::from_utf8(&bytes[..index]).expect("valid UTF-8 prefix");
        let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
        let column = prefix.rsplit('\n').next().unwrap_or("").chars().count() + 1;
        eprintln!("repair.toml:{line}:{column} [invalid_utf8]: invalid UTF-8 at byte {index}");
        std::process::exit(1);
    });
    let generated = match config_build::compile(source, "repair.toml") {
        Ok(generated) => generated,
        Err(errors) => {
            for error in errors {
                eprintln!("{error}");
            }
            std::process::exit(1);
        }
    };
    let output =
        std::path::PathBuf::from(std::env::var_os("OUT_DIR").expect("Cargo supplies OUT_DIR"));
    std::fs::write(output.join("repair_config.rs"), generated)
        .expect("write generated repair configuration");
}

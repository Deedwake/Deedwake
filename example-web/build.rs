#[allow(dead_code)]
#[path = "../examples/convoy/config.rs"]
mod config;
#[path = "../examples/convoy/config_build.rs"]
mod config_build;

fn main() {
    for input in [
        "build.rs",
        "convoy.toml",
        "../examples/convoy/config.rs",
        "../examples/convoy/config_build.rs",
    ] {
        println!("cargo::rerun-if-changed={input}");
    }
    let bytes = std::fs::read("convoy.toml").unwrap_or_else(|error| {
        eprintln!("convoy.toml [read_error]: {error}");
        std::process::exit(1);
    });
    let source = std::str::from_utf8(&bytes).unwrap_or_else(|error| {
        let index = error.valid_up_to();
        let prefix = std::str::from_utf8(&bytes[..index]).expect("valid UTF-8 prefix");
        let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
        let column = prefix.rsplit('\n').next().unwrap_or("").chars().count() + 1;
        eprintln!("convoy.toml:{line}:{column} [invalid_utf8]: invalid UTF-8 at byte {index}");
        std::process::exit(1);
    });
    let generated = match config_build::compile(source, "convoy.toml") {
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
    std::fs::write(output.join("convoy_config.rs"), generated)
        .expect("write generated convoy configuration");
}

use std::{fs, path::Path, process::Command, time::SystemTime};

fn copy_sources(source: &Path, destination: &Path) {
    if source.is_dir() {
        fs::create_dir_all(destination).unwrap();
        for entry in fs::read_dir(source).unwrap() {
            let entry = entry.unwrap();
            if entry.file_name() != "target" {
                copy_sources(&entry.path(), &destination.join(entry.file_name()));
            }
        }
    } else {
        fs::copy(source, destination).unwrap();
    }
}

struct Scratch(std::path::PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn invalid_checked_in_configuration_does_not_prevent_tool_building() {
    let nonce = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let scratch = Scratch(std::env::temp_dir().join(format!(
        "deedwake-tool-independence-{}-{nonce}",
        std::process::id()
    )));
    fs::create_dir(&scratch.0).unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    for item in [
        "Cargo.toml",
        "Cargo.lock",
        "rust-toolchain.toml",
        "src",
        "examples",
        "example-web",
        "repair-web",
        "repair-tool",
    ] {
        copy_sources(&root.join(item), &scratch.0.join(item));
    }
    let invalid = scratch.0.join("repair-web/repair.toml");
    fs::write(&invalid, "schema_version =\n").unwrap();
    let build = Command::new(env!("CARGO"))
        .args([
            "build",
            "-p",
            "deedwake-repair-tool",
            "--locked",
            "--offline",
        ])
        .current_dir(&scratch.0)
        .env("CARGO_TARGET_DIR", scratch.0.join("target"))
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "tool must build despite invalid repair configuration: {}",
        String::from_utf8_lossy(&build.stderr)
    );
    let binary = scratch
        .0
        .join("target/debug")
        .join(format!("repair-tool{}", std::env::consts::EXE_SUFFIX));
    let check = Command::new(binary)
        .arg("check")
        .arg(&invalid)
        .arg("--json")
        .output()
        .unwrap();
    assert_eq!(check.status.code(), Some(2));
    let report: serde_json::Value = serde_json::from_slice(&check.stdout).unwrap();
    assert_eq!(report["schema_version"], 1);
    assert_eq!(report["ok"], false);
    assert_eq!(report["diagnostics"][0]["code"], "syntax");
}

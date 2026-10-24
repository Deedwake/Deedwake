#[path = "../../examples/repair/scene.rs"]
pub mod scene;
mod generated {
    include!(concat!(env!("OUT_DIR"), "/repair_config.rs"));
}

/// The native and WebAssembly hosts use the same build-validated configuration.
pub fn configured_repair() -> scene::Repair {
    scene::Repair::with_config(generated::generated_config())
}

struct Fixture(scene::Repair);
impl Fixture {
    fn new() -> Self {
        Self(configured_repair())
    }
    fn time(&self) -> u32 {
        self.0.time()
    }
    fn enabled(&self, action: u32) -> bool {
        self.0.enabled(action)
    }
    fn act(&mut self, action: u32) -> u32 {
        self.0.act(action)
    }
    fn output(&self, field: u32) -> Option<String> {
        self.0.output(field)
    }
}
#[path = "../../example-web/src/bridge.rs"]
mod bridge;

#[cfg(test)]
use scene::config;
#[cfg(test)]
#[path = "../../examples/repair/config_build.rs"]
mod config_build;

#[cfg(test)]
mod parity {
    use super::*;
    #[test]
    fn generated_and_direct_construction_have_identical_behavior_and_traces() {
        let parsed =
            config_build::validate_source(include_str!("../repair.toml"), "repair.toml").unwrap();
        let direct = config::validate(config::Input {
            trace_capacity: parsed.trace_capacity() as u64,
            report_to_intake_ticks: parsed.report_to_intake_ticks(),
            report_to_technician_ticks: parsed.report_to_technician_ticks(),
            technician_to_intake_ticks: parsed.technician_to_intake_ticks(),
            technician_to_collection_ticks: parsed.technician_to_collection_ticks(),
            cable_receipt_at_tick: parsed.cable_receipt_at_tick(),
            ..config::DEFAULT_INPUT
        })
        .unwrap();
        assert_eq!(generated::generated_config(), direct);
        let mut generated = configured_repair();
        let mut handwritten = scene::Repair::with_config(direct);
        for action in [5, 1, 2, 6, 6, 2, 3, 6, 6, 3, 5, 4, 6, 5, 4, 6, 4, 0] {
            assert_eq!(generated.act(action), handwritten.act(action));
            assert_eq!(generated.time(), handwritten.time());
            for field in 0..=2 {
                assert_eq!(generated.output(field), handwritten.output(field));
            }
        }
    }
}

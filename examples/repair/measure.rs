//! Small release measurement harness, not a guarantee about other hosts/workloads.
use deedwake_repair_web::{
    configured_repair,
    scene::{
        Repair,
        config::{DEFAULT_INPUT, validate},
    },
};
use std::{hint::black_box, time::Instant};

fn measure(label: &str, make: impl Fn() -> Repair) {
    let start = Instant::now();
    for _ in 0..5_000 {
        black_box(make());
    }
    let startup = start.elapsed().as_nanos() / 5_000;
    let mut scene = make();
    for action in [1, 6, 6, 2, 3, 6, 6, 3, 6, 6, 4] {
        assert!(scene.act(action) <= 2);
    }
    scene.act(5);
    let start = Instant::now();
    for _ in 0..50_000 {
        black_box(scene.act(black_box(5)));
    }
    let query = start.elapsed().as_nanos() / 50_000;
    println!("{label}: startup_mean_ns={startup} repeated_intake_mean_ns={query}");
}
fn main() {
    let selected = configured_repair().configuration();
    let input = deedwake_repair_web::scene::config::Input {
        trace_capacity: selected.trace_capacity() as u64,
        report_to_intake_ticks: selected.report_to_intake_ticks(),
        report_to_technician_ticks: selected.report_to_technician_ticks(),
        technician_to_intake_ticks: selected.technician_to_intake_ticks(),
        technician_to_collection_ticks: selected.technician_to_collection_ticks(),
        cable_receipt_at_tick: selected.cable_receipt_at_tick(),
        ..DEFAULT_INPUT
    };
    for _ in 0..5 {
        measure("direct", || Repair::with_config(validate(input).unwrap()));
        measure("generated", configured_repair);
    }
}

//! Synthetic repair café demonstration; no real appliance control, external messages, or purchases.
use deedwake_repair_web::configured_repair;

fn main() {
    let mut cafe = configured_repair();
    for action in [1, 6, 6, 2, 3, 6, 6, 3, 5, 4, 6, 5, 4, 6, 4] {
        let result = cafe.act(action);
        assert!(result <= 2, "invalid host status");
        if result != 0 {
            println!(
                "Hour {}: Choice {action} unavailable (status {result}); state unchanged.",
                cafe.time()
            );
        } else if action != 6 {
            println!("Hour {}: {}", cafe.time(), cafe.output(0).unwrap());
        }
    }
    println!("\nLearned journal:\n{}", cafe.output(1).unwrap());
    if std::env::args().skip(1).any(|arg| arg == "--trace") {
        println!("\nCausal trace:\n{}", cafe.output(2).unwrap());
    }
}

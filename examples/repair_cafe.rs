//! Synthetic repair café demonstration; no real appliance control, external messages, or purchases.
#[path = "repair/scene.rs"]
mod scene;
use scene::Repair;

fn main() {
    let mut cafe = Repair::new();
    for action in [1, 6, 6, 2, 3, 6, 6, 3, 5, 4, 6, 5, 4, 6, 4] {
        assert!(cafe.enabled(action));
        assert_eq!(cafe.act(action), 0);
        if action != 6 {
            println!("Hour {}: {}", cafe.time(), cafe.output(0).unwrap());
        }
    }
    println!("\nLearned journal:\n{}", cafe.output(1).unwrap());
}

#[path = "convoy/scene.rs"]
mod scene;
use scene::Convoy;
fn main() {
    for intervene in [false, true] {
        let mut scene = Convoy::new();
        for action in [1, 2, 6, 6, 6, 5, 3] {
            assert_eq!(scene.act(action), 0);
        }
        if intervene {
            assert_eq!(scene.act(4), 0);
        }
        for _ in 0..if intervene { 1 } else { 3 } {
            assert_eq!(scene.act(6), 0);
        }
        assert_eq!(scene.act(5), 0);
        println!("Hour {}: {}", scene.time(), scene.output(0).unwrap());
    }
}

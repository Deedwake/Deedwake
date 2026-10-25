#[path = "../../examples/convoy/scene.rs"]
mod scene;
use scene::Convoy as Fixture;
mod bridge;

#[unsafe(no_mangle)]
pub extern "C" fn dw_convoy_select(id: u32) -> u32 {
    let Some(condition) = scene::Condition::from_id(id) else {
        return 1;
    };
    bridge::replace(Fixture::with_condition(condition));
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn dw_convoy_condition(field: u32) -> u32 {
    bridge::inspect(|fixture| match field {
        0 => 1,
        1 => fixture.condition().id(),
        2 => fixture.condition().routine_delay(),
        3 => fixture.condition().intermediary_delay(),
        _ => u32::MAX,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outputs() -> Vec<String> {
        bridge::inspect(|fixture| (0..3).map(|field| fixture.output(field).unwrap()).collect())
    }

    #[test]
    fn condition_selection_reset_and_invalid_selection_are_explicit() {
        assert_eq!(bridge::dw_version(), 1);
        bridge::dw_init();
        assert_eq!(dw_convoy_condition(0), 1);
        assert_eq!(dw_convoy_condition(1), 1);
        assert_eq!(dw_convoy_condition(2), 3);
        assert_eq!(dw_convoy_condition(3), 1);
        assert_eq!(dw_convoy_condition(4), u32::MAX);
        for id in [1, 2] {
            assert_eq!(dw_convoy_select(id), 0);
            let fresh = outputs();
            assert_eq!(dw_convoy_condition(1), id);
            assert_eq!(dw_convoy_condition(3), if id == 1 { 1 } else { 3 });
            assert_eq!(bridge::dw_act(1), 0);
            assert_eq!(bridge::dw_act(6), 0);
            let before = outputs();
            for invalid in [0, 3, u32::MAX] {
                assert_eq!(dw_convoy_select(invalid), 1);
                assert_eq!(outputs(), before);
                assert_eq!(bridge::dw_time(), 1);
                assert_eq!(dw_convoy_condition(1), id);
            }
            assert_eq!(bridge::dw_act(0), 0);
            assert_eq!(dw_convoy_condition(1), id);
            assert_eq!(outputs(), fresh);
            assert_eq!(bridge::dw_time(), 0);
            assert_eq!(bridge::dw_enabled(1), 1);
            assert_eq!(bridge::dw_act(1), 0);
            assert_eq!(dw_convoy_select(id), 0);
            assert_eq!(outputs(), fresh);
        }
        bridge::dw_init();
        assert_eq!(dw_convoy_condition(1), 1);
    }
}

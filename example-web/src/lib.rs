#[path = "../../examples/convoy/scene.rs"]
pub mod scene;
use scene::Convoy as Fixture;
mod bridge;

include!(concat!(env!("OUT_DIR"), "/convoy_config.rs"));

pub fn configured_convoy() -> Fixture {
    Fixture::with_config(generated_config())
}

#[unsafe(no_mangle)]
pub extern "C" fn dw_convoy_configured_init() -> u32 {
    bridge::replace(configured_convoy());
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn dw_convoy_config(field: u32) -> u32 {
    if field == 0 {
        return 1;
    }
    bridge::inspect(|fixture| {
        fixture.config().map_or(u32::MAX, |config| match field {
            1 => config.observer().id(),
            2 => config.retention().id(),
            3 => config.reaction().id(),
            4 => config.initial_delivery_ticks(),
            5 => config.routine_delivery_ticks(),
            6 => config.intermediary_delivery_ticks(),
            7 => config.trace_capacity() as u32,
            _ => u32::MAX,
        })
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn dw_convoy_confidence() -> u32 {
    bridge::inspect(|fixture| {
        fixture
            .config()
            .map_or(u32::MAX, |config| config.confidence().id())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn dw_convoy_profile_len() -> u32 {
    bridge::inspect(|fixture| {
        fixture
            .config()
            .map_or(u32::MAX, |config| config.profile_id().len() as u32)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn dw_convoy_profile_byte(index: u32) -> u32 {
    bridge::inspect(|fixture| {
        fixture
            .config()
            .and_then(|config| config.profile_id().as_bytes().get(index as usize))
            .map_or(256, |byte| u32::from(*byte))
    })
}

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
    bridge::inspect(|fixture| {
        if fixture.study().is_some() || fixture.config().is_some() {
            u32::MAX
        } else {
            match field {
                0 => 1,
                1 => fixture.condition().id(),
                2 => fixture.condition().routine_delay(),
                3 => fixture.condition().intermediary_delay(),
                _ => u32::MAX,
            }
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn dw_convoy_study_select(id: u32) -> u32 {
    let Some(study) = scene::Study::from_id(id) else {
        return 1;
    };
    bridge::replace(Fixture::with_study(study));
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn dw_convoy_study(field: u32) -> u32 {
    bridge::inspect(|fixture| match (field, fixture.study()) {
        (0, _) => 1,
        (1, study) => study.map_or(0, scene::Study::id),
        (2, Some(study)) => study.retention(),
        (3, Some(study)) => study.initial_delay(),
        (4, Some(study)) => study.routine_delay(),
        (5, Some(study)) => study.intermediary_delay(),
        _ => u32::MAX,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configured_metadata_is_bounded_and_reset_preserves_the_selected_mode() {
        bridge::dw_init();
        assert_eq!(dw_convoy_config(0), 1);
        assert_eq!(dw_convoy_profile_len(), u32::MAX);
        assert_eq!(dw_convoy_profile_byte(0), 256);
        for field in 1..=8 {
            assert_eq!(dw_convoy_config(field), u32::MAX);
        }
        assert_eq!(dw_convoy_configured_init(), 0);
        let expected = generated_config();
        assert_eq!(
            (0..8)
                .map(|field| dw_convoy_config(field))
                .collect::<Vec<_>>(),
            vec![
                1,
                expected.observer().id(),
                expected.retention().id(),
                expected.reaction().id(),
                expected.initial_delivery_ticks(),
                expected.routine_delivery_ticks(),
                expected.intermediary_delivery_ticks(),
                expected.trace_capacity() as u32,
            ]
        );
        assert_eq!(dw_convoy_config(8), u32::MAX);
        assert_eq!(dw_convoy_config(u32::MAX), u32::MAX);
        let profile: Vec<_> = (0..dw_convoy_profile_len())
            .map(|index| dw_convoy_profile_byte(index) as u8)
            .collect();
        assert_eq!(profile, expected.profile_id().as_bytes());
        assert_eq!(dw_convoy_profile_byte(dw_convoy_profile_len()), 256);
        assert_eq!(dw_convoy_profile_byte(u32::MAX), 256);
        for field in 0..=4 {
            assert_eq!(dw_convoy_condition(field), u32::MAX);
        }
        let fresh = outputs();
        assert_eq!(bridge::dw_act(1), 0);
        assert_eq!(bridge::dw_act(6), 0);
        let before = outputs();
        assert_eq!(dw_convoy_select(99), 1);
        assert_eq!(outputs(), before);
        assert_eq!(bridge::dw_act(0), 0);
        assert_eq!(outputs(), fresh);
        assert_eq!(bridge::dw_time(), 0);
        assert_eq!(dw_convoy_profile_len(), expected.profile_id().len() as u32);
        assert_eq!(bridge::dw_act(1), 0);
        assert_eq!(dw_convoy_configured_init(), 0);
        assert_eq!(outputs(), fresh);
        assert_eq!(dw_convoy_select(2), 0);
        assert_eq!(dw_convoy_config(1), u32::MAX);
        assert_eq!(dw_convoy_profile_len(), u32::MAX);
        assert_eq!(dw_convoy_configured_init(), 0);
        assert_eq!(dw_convoy_study_select(1), 0);
        assert_eq!(dw_convoy_config(1), u32::MAX);
        assert_eq!(dw_convoy_configured_init(), 0);
        bridge::dw_init();
        assert_eq!(dw_convoy_config(1), u32::MAX);
        assert_eq!(dw_convoy_condition(1), 1);
    }

    #[test]
    fn generated_and_direct_configuration_run_the_same_scene() {
        let mut generated = configured_convoy();
        let mut direct =
            Fixture::with_config(scene::config::validate(scene::config::DEFAULT_INPUT).unwrap());
        let mut legacy = Fixture::with_config(
            scene::config::validate(scene::config::Input {
                schema_version: 1,
                ..scene::config::DEFAULT_INPUT
            })
            .unwrap(),
        );
        for action in [1, 2, 6, 6, 6, 5, 3, 4, 6, 5, 6, 6, 5, 0, 1, 2] {
            let result = direct.act(action);
            assert_eq!(generated.act(action), result);
            assert_eq!(legacy.act(action), result);
            assert_eq!(generated.time(), direct.time());
            assert_eq!(legacy.time(), direct.time());
            for field in 0..3 {
                assert_eq!(generated.output(field), direct.output(field));
                assert_eq!(legacy.output(field), direct.output(field));
            }
            for choice in 0..7 {
                assert_eq!(generated.enabled(choice), direct.enabled(choice));
                assert_eq!(legacy.enabled(choice), direct.enabled(choice));
            }
        }
    }

    #[test]
    fn confidence_export_tracks_active_config_and_reset() {
        bridge::dw_init();
        assert_eq!(dw_convoy_confidence(), u32::MAX);
        assert_eq!(dw_convoy_configured_init(), 0);
        assert_eq!(dw_convoy_confidence(), 1);
        for (schema_version, sela_confidence, expected) in
            [(1, "trusted", 1), (2, "trusted", 1), (2, "unverified", 2)]
        {
            bridge::replace(Fixture::with_config(
                scene::config::validate(scene::config::Input {
                    schema_version,
                    sela_confidence,
                    ..scene::config::DEFAULT_INPUT
                })
                .unwrap(),
            ));
            assert_eq!(dw_convoy_confidence(), expected);
            assert_eq!(bridge::dw_act(1), 0);
            assert_eq!(bridge::dw_act(0), 0);
            assert_eq!(dw_convoy_confidence(), expected);
            assert_eq!(dw_convoy_select(99), 1);
            assert_eq!(dw_convoy_study_select(99), 1);
            assert_eq!(dw_convoy_confidence(), expected);
            assert_eq!(dw_convoy_config(0), 1);
            assert_eq!(dw_convoy_config(8), u32::MAX);
        }
        for id in [1, 2] {
            assert_eq!(dw_convoy_select(id), 0);
            assert_eq!(dw_convoy_confidence(), u32::MAX);
        }
        for id in 1..=4 {
            assert_eq!(dw_convoy_study_select(id), 0);
            assert_eq!(dw_convoy_confidence(), u32::MAX);
        }
        assert_eq!(dw_convoy_configured_init(), 0);
        bridge::dw_init();
        assert_eq!(dw_convoy_confidence(), u32::MAX);
    }

    fn outputs() -> Vec<String> {
        bridge::inspect(|fixture| (0..3).map(|field| fixture.output(field).unwrap()).collect())
    }

    #[test]
    fn study_capability_metadata_and_selection_are_bounded_and_reset_explicitly() {
        bridge::dw_init();
        assert_eq!(dw_convoy_study(0), 1);
        assert_eq!(dw_convoy_study(1), 0);
        for field in 2..=6 {
            assert_eq!(dw_convoy_study(field), u32::MAX);
        }
        for id in 1..=4 {
            assert_eq!(dw_convoy_study_select(id), 0);
            let fresh = outputs();
            let study = scene::Study::from_id(id).unwrap();
            assert_eq!(
                (0..6)
                    .map(|field| dw_convoy_study(field))
                    .collect::<Vec<_>>(),
                vec![
                    1,
                    id,
                    study.retention(),
                    study.initial_delay(),
                    study.routine_delay(),
                    study.intermediary_delay()
                ]
            );
            for field in 0..=5 {
                assert_eq!(dw_convoy_condition(field), u32::MAX);
            }
            for field in [6, u32::MAX] {
                assert_eq!(dw_convoy_study(field), u32::MAX);
            }
            assert_eq!(bridge::dw_act(1), 0);
            assert_eq!(bridge::dw_act(6), 0);
            let before = outputs();
            for invalid in [0, 5, u32::MAX] {
                assert_eq!(dw_convoy_study_select(invalid), 1);
                assert_eq!(dw_convoy_study(1), id);
                assert_eq!(outputs(), before);
                assert_eq!(bridge::dw_time(), 1);
            }
            assert_eq!(dw_convoy_select(3), 1);
            assert_eq!(outputs(), before);
            assert_eq!(bridge::dw_act(0), 0);
            assert_eq!(dw_convoy_study(1), id);
            assert_eq!(outputs(), fresh);
            assert_eq!(bridge::dw_act(1), 0);
            assert_eq!(dw_convoy_study_select(id), 0);
            assert_eq!(outputs(), fresh);
            assert_eq!(dw_convoy_select(2), 0);
            assert_eq!(dw_convoy_study(1), 0);
            assert_eq!(dw_convoy_condition(1), 2);
            assert_eq!(dw_convoy_study_select(id), 0);
            bridge::dw_init();
            assert_eq!(dw_convoy_study(1), 0);
            assert_eq!(dw_convoy_condition(1), 1);
        }
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

use deedwake_core::{Availability, DeliveryState, EntityState, Residency};
use deedwake_example_web::{
    configured_convoy,
    scene::{Convoy, ProviderDelivery},
};

fn state(residency: Residency, availability: Availability) -> EntityState {
    EntityState::Present {
        residency,
        availability,
    }
}
fn outputs(scene: &Convoy) -> Vec<String> {
    (0..3).map(|field| scene.output(field).unwrap()).collect()
}
fn act(scene: &mut Convoy, actions: &[u32]) {
    for &action in actions {
        assert_eq!(scene.act(action), 0);
    }
}

#[test]
fn public_host_receives_offscreen_then_executes_only_on_explicit_loaded_step() {
    let mut scene = configured_convoy();
    act(&mut scene, &[1, 2, 6, 6]);
    let before = outputs(&scene);
    scene
        .set_provider_state(state(Residency::Unloaded, Availability::Available))
        .unwrap();
    assert_eq!(outputs(&scene), before);
    act(&mut scene, &[6, 6, 6]);
    assert_eq!(
        scene
            .provider_delivery_receipt(ProviderDelivery::Report)
            .unwrap()
            .state,
        DeliveryState::Delivered { at: 3 }
    );
    let at_five = outputs(&scene);
    assert_eq!(scene.act(5), 1);
    assert_eq!(outputs(&scene), at_five);
    scene
        .set_provider_state(state(Residency::Loaded, Availability::Available))
        .unwrap();
    assert_eq!(outputs(&scene), at_five);
    act(&mut scene, &[5]);
    assert!(scene.output(0).unwrap().contains("At hour 5, I restricted"));
    let refusal = scene.output(0).unwrap();
    act(&mut scene, &[3, 4]);
    scene
        .set_provider_state(state(Residency::Unloaded, Availability::Available))
        .unwrap();
    let journal = scene.output(1).unwrap();
    act(&mut scene, &[6, 6]);
    assert_eq!(
        scene
            .provider_delivery_receipt(ProviderDelivery::Intermediary)
            .unwrap()
            .state,
        DeliveryState::Delivered { at: 6 }
    );
    assert_eq!(scene.output(1).unwrap(), journal);
    scene
        .set_provider_state(state(Residency::Loaded, Availability::Available))
        .unwrap();
    act(&mut scene, &[5]);
    assert!(scene.output(0).unwrap().contains("At hour 7, I restored"));
    assert!(scene.output(1).unwrap().contains(&refusal));
    act(&mut scene, &[0]);
    assert_eq!(
        scene.provider_state(),
        Some(state(Residency::Loaded, Availability::Available))
    );
    assert!(
        scene
            .provider_delivery_receipt(ProviderDelivery::Report)
            .is_none()
    );
}

#[test]
fn legacy_host_is_explicitly_unmanaged() {
    let mut scene = Convoy::new();
    let before = outputs(&scene);
    assert!(scene.set_provider_state(EntityState::Retired).is_err());
    assert!(scene.provider_state().is_none());
    assert_eq!(outputs(&scene), before);
}

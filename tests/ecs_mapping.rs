use bevy_ecs::prelude::Component;
use slotmap::{new_key_type, SecondaryMap, SlotMap};

new_key_type! {
    struct CoreEntity;
}

#[derive(Component)]
struct Marker;

#[test]
fn hecs_mapping_cleanup_does_not_alias_reused_slots() {
    let mut core = SlotMap::<CoreEntity, ()>::with_key();
    let mut mapping = SecondaryMap::<CoreEntity, hecs::Entity>::new();
    let mut ecs = hecs::World::new();

    let first = core.insert(());
    let ecs_entity = ecs.spawn((Marker,));
    mapping.insert(first, ecs_entity);

    let mapped = mapping.remove(first).expect("mapping must exist");
    ecs.despawn(mapped).expect("ECS entity must exist");
    core.remove(first).expect("core entity must exist");

    let replacement = core.insert(());
    assert_ne!(first, replacement);
    assert!(mapping.get(first).is_none());
    assert!(!ecs.contains(ecs_entity));
}

#[test]
fn bevy_mapping_cleanup_does_not_alias_reused_slots() {
    let mut core = SlotMap::<CoreEntity, ()>::with_key();
    let mut mapping = SecondaryMap::<CoreEntity, bevy_ecs::entity::Entity>::new();
    let mut ecs = bevy_ecs::world::World::new();

    let first = core.insert(());
    let ecs_entity = ecs.spawn(Marker).id();
    mapping.insert(first, ecs_entity);

    let mapped = mapping.remove(first).expect("mapping must exist");
    assert!(ecs.despawn(mapped));
    core.remove(first).expect("core entity must exist");

    let replacement = core.insert(());
    assert_ne!(first, replacement);
    assert!(mapping.get(first).is_none());
    assert!(ecs.get::<Marker>(ecs_entity).is_none());
}

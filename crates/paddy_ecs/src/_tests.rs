use paddy_utils::dbg;

use crate::{component::Component, storage::StorageType, world::World};

#[derive(Debug, Default)]
struct Position {
    x: f32,
    y: f32,
}
impl Component for Position {
    const STORAGE_TYPE: StorageType = StorageType::Table;
}
#[derive(Debug, Default)]
struct Velocity {
    x: f32,
    y: f32,
}
impl Component for Velocity {
    const STORAGE_TYPE: StorageType = StorageType::Table;
}
#[derive(Debug, Default)]
struct Name(&'static str);
impl Component for Name {
    const STORAGE_TYPE: StorageType = StorageType::Table;
}

#[test]
fn test() {
    let mut world = World::new();
    world.spawn((Name("1"), Position { x: 123., y: 456. }));
    world.spawn((
        Name("2"),
        Position { x: 123., y: 456. },
        Velocity { x: 123., y: 456. },
    ));
    world.spawn((
        Name("3"),
        Velocity { x: 123., y: 456. },
    ));
    let vec = world
        .query_filtered::<(&Name,&Position), ()>()
        .iter(&world)
        .collect::<Vec<_>>();
    println!("{:?}",vec);
}

use std::{
    ptr::NonNull,
    sync::atomic::{AtomicU32, Ordering},
};

use crate::{
    bundle::Bundles,
    component::{tick::Tick, Component, ComponentId, Components},
    entity::{archetype::Archetypes, Entities, Entity, EntityBuilder},
    storage::Storages,
};

pub mod unsafe_world_cell;

#[derive(Debug)]
pub struct WorldId(u32);
static WORLD_ID: AtomicU32 = AtomicU32::new(0);
impl WorldId {
    /// 在整个软件系统中,创建一个唯一的World ID
    ///
    pub fn next() -> Option<Self> {
        WORLD_ID
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| {
                v.checked_add(1)
            })
            .map(|v| WorldId(v))
            .ok()
    }
}

/// 没啥用,换个命名而言
type EntityAdmin = World;

#[derive(Debug)]
pub struct World {
    world_id: WorldId,
    pub(crate) entities: Entities,
    pub(crate) components: Components,
    pub(crate) storages: Storages,
    pub(crate) archetypes: Archetypes,
    pub(crate) bundles: Bundles,
    pub(crate) change_tick: AtomicU32,
    pub(crate) last_change_tick: Tick,
    pub(crate) last_check_tick: Tick,
}

impl World {
    /// 创建一个World
    pub fn create_world() -> Self {
        todo!();
    }

    /// 在当前World中,创建一个 Entity \
    /// @return EntityBuilder 用于初始化构造这个Entity
    pub fn create_entity(&mut self) -> EntityBuilder {
        todo!()
    }

    /// Initializes a new [`Component`] type and returns the [`ComponentId`] created for it.
    pub fn init_component<T: Component>(&mut self) -> ComponentId {
        self.components.init_component::<T>()
    }
}

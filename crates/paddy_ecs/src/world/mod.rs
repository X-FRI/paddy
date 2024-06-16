use std::{
    ptr::NonNull,
    sync::atomic::{AtomicU32, Ordering},
};

use crate::entity::{Entities, Entity, EntityBuilder};

#[derive(Debug)]
struct WorldId(u32);
static WORLD_ID: AtomicU32 = AtomicU32::new(0);
impl WorldId {
    /// 在整个软件系统中,创建一个唯一的ID
    /// 
    pub fn next() -> Option<Self> {
        WORLD_ID
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| v.checked_add(1))
            .map(|v| WorldId(v))
            .ok()
    }
}

/// 没啥用,换个命名而言
type EntityAdmin = World;

#[derive(Debug)]
pub(crate) struct World {
    world_id: WorldId,
    entities: Entities,

    // archetype : Archetype
}

impl World {
    /// 创建一个World
    pub fn create_world() -> Self {
        todo!();
        Self {
            world_id:WorldId::next().unwrap(),
            entities:Entities::new(),
        }
    }

    /// 在当前World中,创建一个 Entity \
    /// @return EntityBuilder 用于初始化构造这个Entity
    pub fn create_entity(&mut self) -> EntityBuilder {
        todo!()
    }
}




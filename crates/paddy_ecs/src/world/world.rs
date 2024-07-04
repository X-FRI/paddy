use std::{
    ptr::NonNull,
    sync::atomic::{AtomicU32, Ordering},
};

use super::unsafe_world_cell::UnsafeWorldCell;
use crate::{
    archetype::Archetypes,
    bundle::{Bundle, BundleSpawner, Bundles},
    component::{tick::Tick, Component, ComponentId, Components},
    entity::{Entities, Entity, EntityBuilder},
    query::{fetch::QueryData, filter::QueryFilter, state::QueryState},
    storage::Storages,
};

#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash)]
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

    /// Retrieves this [`World`]'s unique ID
    #[inline]
    pub fn id(&self) -> WorldId {
        self.world_id
    }

    /// Reads the current change tick of this world.
    ///
    /// If you have exclusive (`&mut`) access to the world, consider using [`change_tick()`](Self::change_tick),
    /// which is more efficient since it does not require atomic synchronization.
    #[inline]
    pub fn read_change_tick(&self) -> Tick {
        let tick = self.change_tick.load(Ordering::Acquire);
        Tick::new(tick)
    }

    /// Reads the current change tick of this world.
    ///
    /// This does the same thing as [`read_change_tick()`](Self::read_change_tick), only this method
    /// is more efficient since it does not require atomic synchronization.
    #[inline]
    pub fn change_tick(&mut self) -> Tick {
        let tick = *self.change_tick.get_mut();
        Tick::new(tick)
    }

    /// When called from within an exclusive system (a [`System`] that takes `&mut World` as its first
    /// parameter), this method returns the [`Tick`] indicating the last time the exclusive system was run.
    ///
    /// Otherwise, this returns the `Tick` indicating the last time that [`World::clear_trackers`] was called.
    ///
    /// [`System`]: crate::system::System
    #[inline]
    pub fn last_change_tick(&self) -> Tick {
        self.last_change_tick
    }

    /// Initializes a new [`Component`] type and returns the [`ComponentId`] created for it.
    pub fn init_component<T: Component>(&mut self) -> ComponentId {
        self.components.init_component::<T>()
    }

    /// Creates a new [`UnsafeWorldCell`] view with complete read+write access.
    #[inline]
    pub fn as_unsafe_world_cell(&mut self) -> UnsafeWorldCell<'_> {
        UnsafeWorldCell::new_mutable(self)
    }

    /// Creates a new [`UnsafeWorldCell`] view with only read access to everything.
    #[inline]
    pub fn as_unsafe_world_cell_readonly(&self) -> UnsafeWorldCell<'_> {
        UnsafeWorldCell::new_readonly(self)
    }

    #[inline]
    pub fn query_filtered<D: QueryData, F: QueryFilter>(
        &mut self,
    ) -> QueryState<D, F> {
        QueryState::new(self)
    }

    pub fn spawn<B: Bundle>(&mut self, bundle: B) -> () {
        self.flush_entities();
        let change_tick = self.change_tick();
        let entity = self.entities.alloc();
        let entity_location = {
            let mut bundle_spawner = BundleSpawner::new::<B>(self, change_tick);
            // SAFETY: bundle's type matches `bundle_info`, entity is allocated but non-existent
            unsafe { bundle_spawner.spawn_non_existent(entity, bundle) }
        };

        // SAFETY: entity and location are valid, as they were just created above
        // unsafe { EntityWorldMut::new(self, entity, entity_location) }
    }

    /// Empties queued entities and adds them to the empty [`Archetype`](crate::archetype::Archetype).
    /// This should be called before doing operations that might operate on queued entities,
    /// such as inserting a [`Component`].
    pub(crate) fn flush_entities(&mut self) {
        let empty_archetype = self.archetypes.empty_mut();
        let table = &mut self.storages.tables[empty_archetype.table_id()];
        // PERF: consider pre-allocating space for flushed entities
        // SAFETY: entity is set to a valid location
        unsafe {
            self.entities.flush(|entity, location| {
                // SAFETY: no components are allocated by archetype.allocate() because the archetype
                // is empty
                *location =
                    empty_archetype.allocate(entity, table.allocate(entity));
            });
        }
    }
}

/// Creates an instance of the type this trait is implemented for
/// using data from the supplied [`World`].
///
/// This can be helpful for complex initialization or context-aware defaults.
pub trait FromWorld {
    /// Creates `Self` using data from the given [`World`].
    fn from_world(world: &mut World) -> Self;
}

impl<T: Default> FromWorld for T {
    fn from_world(_world: &mut World) -> Self {
        T::default()
    }
}

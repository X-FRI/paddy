use core::fmt;
use std::ptr;

use fixedbitset::FixedBitSet;

use super::{fetch::QueryData, filter::QueryFilter, iter::QueryIter};
use crate::{
    archetype::{Archetype, ArchetypeGeneration, ArchetypeId},
    component::{tick::Tick, ComponentId},
    storage::table::TableId,
    world::{unsafe_world_cell::UnsafeWorldCell, World, WorldId},
};

/// An ID for either a table or an archetype. Used for Query iteration.
///
/// Query iteration is exclusively dense (over tables) or archetypal (over archetypes) based on whether
/// both `D::IS_DENSE` and `F::IS_DENSE` are true or not.
///
/// This is a union instead of an enum as the usage is determined at compile time, as all [`StorageId`]s for
/// a [`QueryState`] will be all [`TableId`]s or all [`ArchetypeId`]s, and not a mixture of both. This
/// removes the need for discriminator to minimize memory usage and branching during iteration, but requires
/// a safety invariant be verified when disambiguating them.
///
/// # Safety
/// Must be initialized and accessed as a [`TableId`], if both generic parameters to the query are dense.
/// Must be initialized and accessed as an [`ArchetypeId`] otherwise.
#[derive(Clone, Copy)]
pub(super) union StorageId {
    pub(super) table_id: TableId,
    pub(super) archetype_id: ArchetypeId,
}

/// Provides scoped access to a [`World`] state according to a given [`QueryData`] and [`QueryFilter`].
///
/// This data is cached between system runs, and is used to:
/// - store metadata about which [`Table`] or [`Archetype`] are matched by the query. "Matched" means
/// that the query will iterate over the data in the matched table/archetype.
/// - cache the [`State`] needed to compute the [`Fetch`] struct used to retrieve data
/// from a specific [`Table`] or [`Archetype`]
/// - build iterators that can iterate over the query results
///
/// [`State`]: crate::query::world_query::WorldQuery::State
/// [`Fetch`]: crate::query::world_query::WorldQuery::Fetch
/// [`Table`]: crate::storage::Table
#[repr(C)]
// SAFETY NOTE:
// Do not add any new fields that use the `D` or `F` generic parameters as this may
// make `QueryState::as_transmuted_state` unsound if not done with care.
pub struct QueryState<D: QueryData, F: QueryFilter> {
    world_id: WorldId,
    pub(crate) archetype_generation: ArchetypeGeneration,
    /// Metadata about the [`Table`](crate::storage::Table)s matched by this query.
    pub(crate) matched_tables: FixedBitSet,
    /// Metadata about the [`Archetype`]s matched by this query.
    pub(crate) matched_archetypes: FixedBitSet,
    /// [`FilteredAccess`] computed by combining the `D` and `F` access. Used to check which other queries
    /// this query can run in parallel with.
    // pub(crate) component_access: FilteredAccess<ComponentId>,
    // NOTE: we maintain both a bitset and a vec because iterating the vec is faster
    pub(super) matched_storage_ids: Vec<StorageId>,
    pub(crate) fetch_state: D::State,
    pub(crate) filter_state: F::State,
}
impl<D: QueryData, F: QueryFilter> fmt::Debug for QueryState<D, F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("QueryState")
            .field("world_id", &self.world_id)
            .field("matched_table_count", &self.matched_tables.count_ones(..))
            .field(
                "matched_archetype_count",
                &self.matched_archetypes.count_ones(..),
            )
            .finish_non_exhaustive()
    }
}

impl<D: QueryData, F: QueryFilter> QueryState<D, F> {
    /// Converts this `QueryState` reference to a `QueryState` that does not access anything mutably.
    pub fn as_readonly(&self) -> &QueryState<D::ReadOnly, F> {
        // SAFETY: invariant on `WorldQuery` trait upholds that `D::ReadOnly` and `F::ReadOnly`
        // have a subset of the access, and match the exact same archetypes/tables as `D`/`F` respectively.
        unsafe { self.as_transmuted_state::<D::ReadOnly, F>() }
    }

    /// Converts this `QueryState` reference to any other `QueryState` with
    /// the same `WorldQuery::State` associated types.
    ///
    /// Consider using `as_readonly` or `as_nop` instead which are safe functions.
    ///
    /// # SAFETY
    ///
    /// `NewD` must have a subset of the access that `D` does and match the exact same archetypes/tables
    /// `NewF` must have a subset of the access that `F` does and match the exact same archetypes/tables
    pub(crate) unsafe fn as_transmuted_state<
        NewD: QueryData<State = D::State>,
        NewF: QueryFilter<State = F::State>,
    >(
        &self,
    ) -> &QueryState<NewD, NewF> {
        &*ptr::from_ref(self).cast::<QueryState<NewD, NewF>>()
    }

    /// Returns the tables matched by this query.
    pub fn matched_tables(&self) -> impl Iterator<Item = TableId> + '_ {
        self.matched_tables.ones().map(TableId::from_usize)
    }

    /// Returns the archetypes matched by this query.
    pub fn matched_archetypes(&self) -> impl Iterator<Item = ArchetypeId> + '_ {
        self.matched_archetypes.ones().map(ArchetypeId::new)
    }
}

impl<D: QueryData, F: QueryFilter> QueryState<D, F> {
    /// Creates a new [`QueryState`] from a given [`World`] and inherits the result of `world.id()`.
    pub fn new(world: &mut World) -> Self {
        let mut state = Self::new_uninitialized(world);
        state.update_archetypes(world);
        state
    }

    /// Creates a new [`QueryState`] but does not populate it with the matched results from the World yet
    ///
    /// `new_archetype` and its variants must be called on all of the World's archetypes before the
    /// state can return valid query results.
    fn new_uninitialized(world: &mut World) -> Self {
        let fetch_state = D::init_state(world);
        let filter_state = F::init_state(world);

        // let mut component_access = FilteredAccess::default();
        // D::update_component_access(&fetch_state, &mut component_access);

        // Use a temporary empty FilteredAccess for filters. This prevents them from conflicting with the
        // main Query's `fetch_state` access. Filters are allowed to conflict with the main query fetch
        // because they are evaluated *before* a specific reference is constructed.
        // let mut filter_component_access = FilteredAccess::default();
        // F::update_component_access(&filter_state, &mut filter_component_access);

        // Merge the temporary filter access with the main access. This ensures that filter access is
        // properly considered in a global "cross-query" context (both within systems and across systems).
        // component_access.extend(&filter_component_access);

        Self {
            world_id: world.id(),
            archetype_generation: ArchetypeGeneration::initial(),
            matched_storage_ids: Vec::new(),
            fetch_state,
            filter_state,
            // component_access,
            matched_tables: Default::default(),
            matched_archetypes: Default::default(),
        }
    }

    /// Process the given [`Archetype`] to update internal metadata about the [`Table`](crate::storage::Table)s
    /// and [`Archetype`]s that are matched by this query.
    ///
    /// Returns `true` if the given `archetype` matches the query. Otherwise, returns `false`.
    /// If there is no match, then there is no need to update the query's [`FilteredAccess`].
    ///
    /// # Safety
    /// `archetype` must be from the `World` this state was initialized from.
    unsafe fn new_archetype_internal(&mut self, archetype: &Archetype) -> bool {
        if D::matches_component_set(&self.fetch_state, &|id| {
            archetype.contains(id)
        }) && F::matches_component_set(&self.filter_state, &|id| {
            archetype.contains(id)
        }) && self.matches_component_set(&|id| archetype.contains(id))
        {
            let archetype_index = archetype.id().index();
            if !self.matched_archetypes.contains(archetype_index) {
                self.matched_archetypes.grow_and_insert(archetype_index);
                if !D::IS_DENSE || !F::IS_DENSE {
                    self.matched_storage_ids.push(StorageId {
                        archetype_id: archetype.id(),
                    });
                }
            }
            let table_index = archetype.table_id().as_usize();
            if !self.matched_tables.contains(table_index) {
                self.matched_tables.grow_and_insert(table_index);
                if D::IS_DENSE && F::IS_DENSE {
                    self.matched_storage_ids.push(StorageId {
                        table_id: archetype.table_id(),
                    });
                }
            }
            true
        } else {
            false
        }
    }

    /// Updates the state's internal view of the [`World`]'s archetypes. If this is not called before querying data,
    /// the results may not accurately reflect what is in the `world`.
    ///
    /// This is only required if a `manual` method (such as [`Self::get_manual`]) is being called, and it only needs to
    /// be called if the `world` has been structurally mutated (i.e. added/removed a component or resource). Users using
    /// non-`manual` methods such as [`QueryState::get`] do not need to call this as it will be automatically called for them.
    ///
    /// If you have an [`UnsafeWorldCell`] instead of `&World`, consider using [`QueryState::update_archetypes_unsafe_world_cell`].
    ///
    /// # Panics
    ///
    /// If `world` does not match the one used to call `QueryState::new` for this instance.
    #[inline]
    pub fn update_archetypes(&mut self, world: &World) {
        self.update_archetypes_unsafe_world_cell(
            world.as_unsafe_world_cell_readonly(),
        );
    }

    /// Updates the state's internal view of the `world`'s archetypes. If this is not called before querying data,
    /// the results may not accurately reflect what is in the `world`.
    ///
    /// This is only required if a `manual` method (such as [`Self::get_manual`]) is being called, and it only needs to
    /// be called if the `world` has been structurally mutated (i.e. added/removed a component or resource). Users using
    /// non-`manual` methods such as [`QueryState::get`] do not need to call this as it will be automatically called for them.
    ///
    /// # Note
    ///
    /// This method only accesses world metadata.
    ///
    /// # Panics
    ///
    /// If `world` does not match the one used to call `QueryState::new` for this instance.
    pub fn update_archetypes_unsafe_world_cell(
        &mut self,
        world: UnsafeWorldCell,
    ) {
        self.validate_world(world.id());
        let archetypes = world.archetypes();
        let old_generation = std::mem::replace(
            &mut self.archetype_generation,
            archetypes.generation(),
        );

        for archetype in &archetypes[old_generation..] {
            // SAFETY: The validate_world call ensures that the world is the same the QueryState
            // was initialized from.
            unsafe {
                self.new_archetype_internal(archetype);
            }
        }
    }

    /// # Panics
    ///
    /// If `world_id` does not match the [`World`] used to call `QueryState::new` for this instance.
    ///
    /// Many unsafe query methods require the world to match for soundness. This function is the easiest
    /// way of ensuring that it matches.
    #[inline]
    #[track_caller]
    pub fn validate_world(&self, world_id: WorldId) {
        #[inline(never)]
        #[track_caller]
        #[cold]
        fn panic_mismatched(this: WorldId, other: WorldId) -> ! {
            panic!("Encountered a mismatched World. This QueryState was created from {this:?}, but a method was called using {other:?}.");
        }

        if self.world_id != world_id {
            panic_mismatched(self.world_id, world_id);
        }
    }

    /// Returns an [`Iterator`] over the query results for the given [`World`].
    ///
    /// This can only be called for read-only queries, see [`Self::iter_mut`] for write-queries.
    #[inline]
    pub fn iter<'w, 's>(
        &'s mut self,
        world: &'w World,
    ) -> QueryIter<'w, 's, D::ReadOnly, F> {
        self.update_archetypes(world);
        // SAFETY: query is read only
        unsafe {
            self.as_readonly().iter_unchecked_manual(
                world.as_unsafe_world_cell_readonly(),
                world.last_change_tick(),
                world.read_change_tick(),
            )
        }
    }

    /// Returns an [`Iterator`] for the given [`World`], where the last change and
    /// the current change tick are given.
    ///
    /// This iterator is always guaranteed to return results from each matching entity once and only once.
    /// Iteration order is not guaranteed.
    ///
    /// # Safety
    ///
    /// This does not check for mutable query correctness. To be safe, make sure mutable queries
    /// have unique access to the components they query.
    /// This does not validate that `world.id()` matches `self.world_id`. Calling this on a `world`
    /// with a mismatched [`WorldId`] is unsound.
    #[inline]
    pub(crate) unsafe fn iter_unchecked_manual<'w, 's>(
        &'s self,
        world: UnsafeWorldCell<'w>,
        last_run: Tick,
        this_run: Tick,
    ) -> QueryIter<'w, 's, D, F> {
        QueryIter::new(world, self, last_run, this_run)
    }

    /// Returns `true` if this query matches a set of components. Otherwise, returns `false`.
    pub fn matches_component_set(
        &self,
        set_contains_id: &impl Fn(ComponentId) -> bool,
    ) -> bool {
        // self.component_access.filter_sets.iter().any(|set| {
        //     set.with.ones().all(|index| {
        //         set_contains_id(ComponentId::get_sparse_set_index(index))
        //     }) && set.without.ones().all(|index| {
        //         !set_contains_id(ComponentId::get_sparse_set_index(index))
        //     })
        // })
        true
    }
}

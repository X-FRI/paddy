use core::fmt;

use fixedbitset::FixedBitSet;

use crate::{
    entity::archetype::{ArchetypeGeneration, ArchetypeId},
    storage::table::TableId,
    world::WorldId,
};

use super::{fetch::QueryData, filter::QueryFilter};

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

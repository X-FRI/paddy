use paddy_utils::all_tuples;

use super::WorldQuery;
use crate::{entity::Entity, storage::table::TableRow};

#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a valid `Query` filter",
    label = "invalid `Query` filter",
    note = "a `QueryFilter` typically uses a combination of `With<T>` and `Without<T>` statements"
)]
pub trait QueryFilter: WorldQuery {
    /// Returns true if (and only if) this Filter relies strictly on archetypes to limit which
    /// components are accessed by the Query.
    ///
    /// This enables optimizations for [`crate::query::QueryIter`] that rely on knowing exactly how
    /// many elements are being iterated (such as `Iterator::collect()`).
    const IS_ARCHETYPAL: bool;

    /// Returns true if the provided [`Entity`] and [`TableRow`] should be included in the query results.
    /// If false, the entity will be skipped.
    ///
    /// Note that this is called after already restricting the matched [`Table`]s and [`Archetype`]s to the
    /// ones that are compatible with the Filter's access.
    ///
    /// # Safety
    ///
    /// Must always be called _after_ [`WorldQuery::set_table`] or [`WorldQuery::set_archetype`]. `entity` and
    /// `table_row` must be in the range of the current table and archetype.
    #[allow(unused_variables)]
    unsafe fn filter_fetch(
        fetch: &mut Self::Fetch<'_>,
        entity: Entity,
        table_row: TableRow,
    ) -> bool;
}


macro_rules! impl_tuple_query_filter {
    ($($name: ident),*) => {
        #[allow(unused_variables)]
        #[allow(non_snake_case)]
        #[allow(clippy::unused_unit)]

        impl<$($name: QueryFilter),*> QueryFilter for ($($name,)*) {
            const IS_ARCHETYPAL: bool = true $(&& $name::IS_ARCHETYPAL)*;

            #[inline(always)]
            unsafe fn filter_fetch(
                fetch: &mut Self::Fetch<'_>,
                _entity: Entity,
                _table_row: TableRow
            ) -> bool {
                let ($($name,)*) = fetch;
                // SAFETY: The invariants are uphold by the caller.
                true $(&& unsafe { $name::filter_fetch($name, _entity, _table_row) })*
            }
        }

    };
}

all_tuples!(impl_tuple_query_filter, 0, 15, F);
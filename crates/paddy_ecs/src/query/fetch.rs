use std::cell::UnsafeCell;

use paddy_ptr::ThinSlicePtr;
use paddy_utils::all_tuples;

use super::WorldQuery;
use crate::{component::Component, storage::sparse_set::ComponentSparseSet};

#[diagnostic::on_unimplemented(
    message = "`{Self}` is not valid to request as data in a `Query`",
    label = "invalid `Query` data"
)]
pub unsafe trait QueryData: WorldQuery {
    /// The read-only variant of this [`QueryData`], which satisfies the [`ReadOnlyQueryData`] trait.
    type ReadOnly: ReadOnlyQueryData<State = <Self as WorldQuery>::State>;
}

/// A [`QueryData`] that is read only.
///
/// # Safety
///
/// This must only be implemented for read-only [`QueryData`]'s.
pub unsafe trait ReadOnlyQueryData: QueryData<ReadOnly = Self> {}

/// The item type returned when a [`WorldQuery`] is iterated over
pub type QueryItem<'w, Q> = <Q as WorldQuery>::Item<'w>;
/// The read-only variant of the item type returned when a [`QueryData`] is iterated over immutably
pub type ROQueryItem<'w, D> = QueryItem<'w, <D as QueryData>::ReadOnly>;


macro_rules! impl_tuple_query_data {
    ($(($name: ident, $state: ident)),*) => {

        #[allow(non_snake_case)]
        #[allow(clippy::unused_unit)]
        // SAFETY: defers to soundness `$name: WorldQuery` impl
        unsafe impl<$($name: QueryData),*> QueryData for ($($name,)*) {
            type ReadOnly = ($($name::ReadOnly,)*);
        }

        /// SAFETY: each item in the tuple is read only
        unsafe impl<$($name: ReadOnlyQueryData),*> ReadOnlyQueryData for ($($name,)*) {}

    };
}
all_tuples!(impl_tuple_query_data, 0, 15, F, S);

/// SAFETY: `Self` is the same as `Self::ReadOnly`
unsafe impl<T: Component> QueryData for &T {
    type ReadOnly = Self;
}

/// SAFETY: access is read only
unsafe impl<T: Component> ReadOnlyQueryData for &T {}



#[doc(hidden)]
pub struct ReadFetch<'w, T> {
    // T::STORAGE_TYPE = StorageType::Table
    pub(crate) table_components: Option<ThinSlicePtr<'w, UnsafeCell<T>>>,
    // T::STORAGE_TYPE = StorageType::SparseSet
    pub(crate) sparse_set: Option<&'w ComponentSparseSet>,
}

impl<T> Clone for ReadFetch<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for ReadFetch<'_, T> {}

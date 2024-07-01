use std::cell::UnsafeCell;

use paddy_ptr::ThinSlicePtr;

use crate::storage::sparse_set::ComponentSparseSet;

use super::WorldQuery;

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

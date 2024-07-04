use crate::{
    component::tick::Tick, debug::DebugCheckedUnwrap, entity::{
        Entity,
    }, storage::table::{TableRow, Tables}, world::unsafe_world_cell::UnsafeWorldCell
};
use crate::archetype::{ArchetypeEntity, Archetypes};

use super::{
    fetch::QueryData,
    filter::QueryFilter,
    state::{QueryState, StorageId},
};

/// An [`Iterator`] over query results of a [`Query`](crate::system::Query).
///
/// This struct is created by the [`Query::iter`](crate::system::Query::iter) and
/// [`Query::iter_mut`](crate::system::Query::iter_mut) methods.
pub struct QueryIter<'w, 's, D: QueryData, F: QueryFilter> {
    world: UnsafeWorldCell<'w>,
    tables: &'w Tables,
    archetypes: &'w Archetypes,
    query_state: &'s QueryState<D, F>,
    cursor: QueryIterationCursor<'w, 's, D, F>,
}

impl<'w, 's, D: QueryData, F: QueryFilter> QueryIter<'w, 's, D, F> {
    /// # Safety
    /// - `world` must have permission to access any of the components registered in `query_state`.
    /// - `world` must be the same one used to initialize `query_state`.
    pub(crate) unsafe fn new(
        world: UnsafeWorldCell<'w>,
        query_state: &'s QueryState<D, F>,
        last_run: Tick,
        this_run: Tick,
    ) -> Self {
        QueryIter {
            world,
            query_state,
            // SAFETY: We only access table data that has been registered in `query_state`.
            tables: unsafe { &world.storages().tables },
            archetypes: world.archetypes(),
            // SAFETY: The invariants are uphold by the caller.
            cursor: unsafe {
                QueryIterationCursor::init(
                    world,
                    query_state,
                    last_run,
                    this_run,
                )
            },
        }
    }
}

struct QueryIterationCursor<'w, 's, D: QueryData, F: QueryFilter> {
    storage_id_iter: std::slice::Iter<'s, StorageId>,
    table_entities: &'w [Entity],
    archetype_entities: &'w [ArchetypeEntity],
    fetch: D::Fetch<'w>,
    filter: F::Fetch<'w>,
    // length of the table or length of the archetype, depending on whether both `D`'s and `F`'s fetches are dense
    current_len: usize,
    // either table row or archetype index, depending on whether both `D`'s and `F`'s fetches are dense
    current_row: usize,
}

impl<D: QueryData, F: QueryFilter> Clone
    for QueryIterationCursor<'_, '_, D, F>
{
    fn clone(&self) -> Self {
        Self {
            storage_id_iter: self.storage_id_iter.clone(),
            table_entities: self.table_entities,
            archetype_entities: self.archetype_entities,
            fetch: self.fetch.clone(),
            filter: self.filter.clone(),
            current_len: self.current_len,
            current_row: self.current_row,
        }
    }
}

impl<'w, 's, D: QueryData, F: QueryFilter> QueryIterationCursor<'w, 's, D, F> {
    const IS_DENSE: bool = D::IS_DENSE && F::IS_DENSE;
    
    unsafe fn init_empty(
        world: UnsafeWorldCell<'w>,
        query_state: &'s QueryState<D, F>,
        last_run: Tick,
        this_run: Tick,
    ) -> Self {
        QueryIterationCursor {
            storage_id_iter: [].iter(),
            ..Self::init(world, query_state, last_run, this_run)
        }
    }

    /// # Safety
    /// - `world` must have permission to access any of the components registered in `query_state`.
    /// - `world` must be the same one used to initialize `query_state`.
    unsafe fn init(
        world: UnsafeWorldCell<'w>,
        query_state: &'s QueryState<D, F>,
        last_run: Tick,
        this_run: Tick,
    ) -> Self {
        let fetch =
            D::init_fetch(world, &query_state.fetch_state, last_run, this_run);
        let filter =
            F::init_fetch(world, &query_state.filter_state, last_run, this_run);
        QueryIterationCursor {
            fetch,
            filter,
            table_entities: &[],
            archetype_entities: &[],
            storage_id_iter: query_state.matched_storage_ids.iter(),
            current_len: 0,
            current_row: 0,
        }
    }

        // NOTE: If you are changing query iteration code, remember to update the following places, where relevant:
    // QueryIter, QueryIterationCursor, QuerySortedIter, QueryManyIter, QueryCombinationIter, QueryState::par_fold_init_unchecked_manual
    /// # Safety
    /// `tables` and `archetypes` must belong to the same world that the [`QueryIterationCursor`]
    /// was initialized for.
    /// `query_state` must be the same [`QueryState`] that was passed to `init` or `init_empty`.
    #[inline(always)]
    unsafe fn next(
        &mut self,
        tables: &'w Tables,
        archetypes: &'w Archetypes,
        query_state: &'s QueryState<D, F>,
    ) -> Option<D::Item<'w>> {
        if Self::IS_DENSE {
            loop {
                // we are on the beginning of the query, or finished processing a table, so skip to the next
                if self.current_row == self.current_len {
                    let table_id = self.storage_id_iter.next()?.table_id;
                    let table = tables.get(table_id).debug_checked_unwrap();
                    // SAFETY: `table` is from the world that `fetch/filter` were created for,
                    // `fetch_state`/`filter_state` are the states that `fetch/filter` were initialized with
                    unsafe {
                        D::set_table(&mut self.fetch, &query_state.fetch_state, table);
                        F::set_table(&mut self.filter, &query_state.filter_state, table);
                    }
                    self.table_entities = table.entities();
                    self.current_len = table.entity_count();
                    self.current_row = 0;
                    continue;
                }

                // SAFETY: set_table was called prior.
                // `current_row` is a table row in range of the current table, because if it was not, then the above would have been executed.
                let entity = unsafe { self.table_entities.get_unchecked(self.current_row) };
                let row = TableRow::from_usize(self.current_row);
                if !F::filter_fetch(&mut self.filter, *entity, row) {
                    self.current_row += 1;
                    continue;
                }

                // SAFETY:
                // - set_table was called prior.
                // - `current_row` must be a table row in range of the current table,
                //   because if it was not, then the above would have been executed.
                // - fetch is only called once for each `entity`.
                let item = unsafe { D::fetch(&mut self.fetch, *entity, row) };

                self.current_row += 1;
                return Some(item);
            }
        } else {
            loop {
                if self.current_row == self.current_len {
                    let archetype_id = self.storage_id_iter.next()?.archetype_id;
                    let archetype = archetypes.get(archetype_id).debug_checked_unwrap();
                    let table = tables.get(archetype.table_id()).debug_checked_unwrap();
                    // SAFETY: `archetype` and `tables` are from the world that `fetch/filter` were created for,
                    // `fetch_state`/`filter_state` are the states that `fetch/filter` were initialized with
                    unsafe {
                        D::set_archetype(
                            &mut self.fetch,
                            &query_state.fetch_state,
                            archetype,
                            table,
                        );
                        F::set_archetype(
                            &mut self.filter,
                            &query_state.filter_state,
                            archetype,
                            table,
                        );
                    }
                    self.archetype_entities = archetype.entities();
                    self.current_len = archetype.len();
                    self.current_row = 0;
                    continue;
                }

                // SAFETY: set_archetype was called prior.
                // `current_row` is an archetype index row in range of the current archetype, because if it was not, then the if above would have been executed.
                let archetype_entity =
                    unsafe { self.archetype_entities.get_unchecked(self.current_row) };
                if !F::filter_fetch(
                    &mut self.filter,
                    archetype_entity.id(),
                    archetype_entity.table_row(),
                ) {
                    self.current_row += 1;
                    continue;
                }

                // SAFETY:
                // - set_archetype was called prior.
                // - `current_row` must be an archetype index row in range of the current archetype,
                //   because if it was not, then the if above would have been executed.
                // - fetch is only called once for each `archetype_entity`.
                let item = unsafe {
                    D::fetch(
                        &mut self.fetch,
                        archetype_entity.id(),
                        archetype_entity.table_row(),
                    )
                };
                self.current_row += 1;
                return Some(item);
            }
        }
    }
}

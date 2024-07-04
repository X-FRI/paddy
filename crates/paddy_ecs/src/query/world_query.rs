use crate::{
    component::{tick::Tick, ComponentId, Components},
    entity::{ Entity},
    storage::table::{Table, TableRow},
    world::{unsafe_world_cell::UnsafeWorldCell, World},
    archetype::Archetype
};

pub unsafe trait WorldQuery {
    /// 由 [`WorldQuery`] 返回的项 的类型
    ///
    /// 对于 `QueryData`，`Item` 是查询返回的具体数据类型
    ///
    /// 对于 `QueryFilter`，`Item` 可能是 `()` 或 `bool` 值，或者这些值的组合
    type Item<'a>;

    /// 定义如何从 `World` 中提取数据的类型
    ///
    /// `Fetch` 表示执行查询时所需的状态和操作
    ///
    /// 每个 archetype/table 状态，用于被 [`WorldQuery`] 使用 来获取 [`Self::Item`](`WorldQuery::Item`)
    type Fetch<'a>: Clone;
    /// 定义用于构建 `Fetch` 的状态
    ///
    /// `State` 存储了执行查询时的所有必要信息，并且会被缓存，以减少每次查询时的计算开销
    ///
    /// 用于构造 [`Self::Fetch`](`WorldQuery::Fetch`) 的状态。这个状态将被缓存到 [`QueryState`](crate::query::QueryState) 中，
    /// 因此最好将尽可能多的数据/计算移动到这里，以减少构造 [`Self::Fetch`](`WorldQuery::Fetch`) 的成本。
    type State: Send + Sync + Sized;

    /// 这个函数手动实现了查询项的子类型化
    fn shrink<'wlong: 'wshort, 'wshort>(
        item: Self::Item<'wlong>,
    ) -> Self::Item<'wshort>;

    /// 创建一个新的 `fetch` 实例
    ///
    /// 使用 `State` 创建一个新的 `Fetch` 实例，准备从 `World` 中提取数据
    ///
    /// # Safety
    ///
    /// - `state` 必须使用与传入此函数的相同 `world`（通过 [`WorldQuery::init_state`] 初始化）进行初始化
    unsafe fn init_fetch<'w>(
        world: UnsafeWorldCell<'w>,
        state: &Self::State,
        last_run: Tick,
        this_run: Tick,
    ) -> Self::Fetch<'w>;

    /// 如果并且仅当这个 `fetch` 匹配的每个原型的每个表都包含所有匹配的组件时，返回 `true`。
    /// 这用于为“密集”查询选择更高效的“表迭代器”。
    ///
    /// 如果返回 `true`，则在调用 [`WorldQuery::fetch`] 之前必须使用 [`WorldQuery::set_table`]。
    /// 如果返回 `false`，则在调用 [`WorldQuery::fetch`] 之前必须使用 [`WorldQuery::set_archetype`]。
    ///
    /// 如果返回 true，表示所有匹配的原型和表都包含所有需要的组件，可以使用更高效的“表迭代器”。
    /// 如果返回 false，需要使用“原型迭代器”
    const IS_DENSE: bool;

    /// Adjusts internal state to account for the next [`Archetype`]. This will always be called on
    /// archetypes that match this [`WorldQuery`].\
    /// 调整内部状态以适应下一个 [`Archetype`]。这将始终在与此 [`WorldQuery`] 匹配的archetype上调用
    ///
    /// 在匹配的archetype上调用，准备 `Fetch` 以处理下一个archetype
    ///
    /// # Safety
    ///
    /// - `archetype` 和 `tables` 必须来自与调用 [`WorldQuery::init_state`] 相同的 [`World`]
    /// - `table` 必须与 `archetype` 对应
    /// - `state` 必须是使用 `fetch` 初始化的 [`State`](Self::State)
    unsafe fn set_archetype<'w>(
        fetch: &mut Self::Fetch<'w>,
        state: &Self::State,
        archetype: &'w Archetype,
        table: &'w Table,
    );

    /// Adjusts internal state to account for the next [`Table`]. This will always be called on tables
    /// that match this [`WorldQuery`].\
    /// 调整内部状态以适应下一个 [`Table`]。这将始终在与此 [`WorldQuery`] 匹配的table上调用
    ///
    /// 在匹配的table上调用，准备 `Fetch` 以处理下一个table
    ///
    /// # Safety
    /// - `table` 必须来自与调用 [`WorldQuery::init_state`] 相同的 [`World`]
    /// - `state` 必须是使用 `fetch` 初始化的 [`State`](Self::State)
    unsafe fn set_table<'w>(
        fetch: &mut Self::Fetch<'w>,
        state: &Self::State,
        table: &'w Table,
    );

    /// Sets available accesses for implementors with dynamic access such as [`FilteredEntityRef`](crate::world::FilteredEntityRef)
    /// or [`FilteredEntityMut`](crate::world::FilteredEntityMut).
    ///
    /// Called when constructing a [`QueryLens`](crate::system::QueryLens) or calling [`QueryState::from_builder`](super::QueryState::from_builder)
    ///
    /// ---
    /// 设置具有动态访问的实现者的可用访问权限，例如 [`FilteredEntityRef`](crate::world::FilteredEntityRef)
    /// 或 [`FilteredEntityMut`](crate::world::FilteredEntityMut)。
    ///
    /// 在构建 [`QueryLens`](crate::system::QueryLens) 或调用 [`QueryState::from_builder`](super::QueryState::from_builder) 时调用
    // fn set_access(_state: &mut Self::State, _access: &FilteredAccess<ComponentId>) {}

    /// Fetch [`Self::Item`](`WorldQuery::Item`) for either the given `entity` in the current [`Table`],
    /// or for the given `entity` in the current [`Archetype`]. This must always be called after
    /// [`WorldQuery::set_table`] with a `table_row` in the range of the current [`Table`] or after
    /// [`WorldQuery::set_archetype`]  with a `entity` in the current archetype.
    ///
    /// # Safety
    ///
    /// Must always be called _after_ [`WorldQuery::set_table`] or [`WorldQuery::set_archetype`]. `entity` and
    /// `table_row` must be in the range of the current table and archetype.
    ///
    /// ---
    /// 为当前 [`Table`] 或当前 [`Archetype`] 中的给定 `entity` 获取 [`Self::Item`](`WorldQuery::Item`)。
    /// 这必须始终在使用当前 [`Table`] 范围内的 `table_row` 调用 [`WorldQuery::set_table`] 之后，
    /// 或在当前原型中的 `entity` 调用 [`WorldQuery::set_archetype`] 之后调用。
    ///
    /// 从当前的 `Table` 或 `Archetype` 中获取指定 `entity` 的查询项\
    /// 根据 `entity` 和 `table_row` 获取查询项
    ///
    /// # 安全性
    ///
    /// 必须始终在 [`WorldQuery::set_table`] 或 [`WorldQuery::set_archetype`] 之后调用。
    /// `entity` 和 `table_row` 必须在当前表和原型的范围内。
    unsafe fn fetch<'w>(
        fetch: &mut Self::Fetch<'w>,
        entity: Entity,
        table_row: TableRow,
    ) -> Self::Item<'w>;

    /// Adds any component accesses used by this [`WorldQuery`] to `access`.
    ///
    /// Used to check which queries are disjoint and can run in parallel
    // This does not have a default body of `{}` because 99% of cases need to add accesses
    // and forgetting to do so would be unsound.
    /// 将此 [`WorldQuery`] 使用的任何组件访问添加到 `access`。
    ///
    /// 用于检查哪些查询是互不相交的，可以并行运行。
    // 这没有默认的空实现 `{}`，因为在 99% 的情况下需要添加访问，
    // 忘记这样做会导致不安全的行为。
    // fn update_component_access(state: &Self::State, access: &mut FilteredAccess<ComponentId>);

    /// 为此 [`WorldQuery`] 类型创建并初始化一个 [`State`](WorldQuery::State)
    ///
    /// 在 World 中创建和初始化查询状态，以便稍后用于执行查询
    fn init_state(world: &mut World) -> Self::State;

    /// Attempts to initialize a [`State`](WorldQuery::State) for this [`WorldQuery`] type using read-only
    /// access to [`Components`].\
    /// 尝试使用对 [`Components`] 的只读访问来初始化此 [`WorldQuery`] 类型的 [`State`](WorldQuery::State)
    ///
    /// 尝试使用只读访问来初始化 WorldQuery 类型的状态
    ///
    fn get_state(components: &Components) -> Option<Self::State>;

    /// 如果此查询与一组组件匹配，则返回 `true`。否则，返回 `false`
    ///
    /// 用于检查哪些 [`Archetype`] 可以被查询跳过（如果没有任何 [`Component`](crate::component::Component) 匹配）
    fn matches_component_set(
        state: &Self::State,
        set_contains_id: &impl Fn(ComponentId) -> bool,
    ) -> bool;
}

// pub(crate) trait WorldQuery {
//     /// 查询的 返回值的类型
//     type Item<'a>;
//     /// 用于如何从 World 中提取数据
//     #[doc(hidden)]
//     type Fetch: Fetch;

//     unsafe fn get<'a>(fetch: &Self::Fetch, n: usize) -> Self::Item<'a>;
// }

// pub(crate) unsafe trait Fetch: Clone + Sized {
//     /// 构建 [`Fetch`] 所需的状态
//     ///
//     /// 该状态被缓存，以减少每次查询时的计算成本
//     type State: Copy;
// }

pub mod impl_world_query {
    use fetch::ReadFetch;

    use super::*;
    use crate::{
        _todo, component::Component, debug::DebugCheckedUnwrap, query::fetch, storage::StorageType
    };

    unsafe impl<T: Component> WorldQuery for &T {
        type Item<'w> = &'w T;

        type Fetch<'w> = ReadFetch<'w, T>;

        type State = ComponentId;

        fn shrink<'wlong: 'wshort, 'wshort>(
            item: Self::Item<'wlong>,
        ) -> Self::Item<'wshort> {
            item
        }

        unsafe fn init_fetch<'w>(
            _world: UnsafeWorldCell<'w>,
            _state: &Self::State,
            _last_run: Tick,
            _this_run: Tick,
        ) -> Self::Fetch<'w> {
            ReadFetch {
                table_components: None,
                sparse_set: (T::STORAGE_TYPE == StorageType::SparseSet).then(
                    || {
                        // SAFETY: The underlying type associated with `component_id` is `T`,
                        // which we are allowed to access since we registered it in `update_archetype_component_access`.
                        // Note that we do not actually access any components in this function, we just get a shared
                        // reference to the sparse set, which is used to access the components in `Self::fetch`.
                        _todo::for_sparse::_sparse();
                        // unsafe {
                        //     world
                        //         .storages()
                        //         .sparse_sets
                        //         .get(component_id)
                        //         .debug_checked_unwrap()
                        // }
                    },
                ),
            }
        }

        const IS_DENSE: bool = {
            match T::STORAGE_TYPE {
                StorageType::Table => true,
                StorageType::SparseSet => false,
            }
        };

        unsafe fn set_archetype<'w>(
            fetch: &mut Self::Fetch<'w>,
            component_id: &Self::State,
            _archetype: &'w Archetype,
            table: &'w Table,
        ) {
            if Self::IS_DENSE {
                // SAFETY: `set_archetype`'s safety rules are a super set of the `set_table`'s ones.
                unsafe {
                    Self::set_table(fetch, component_id, table);
                }
            }
        }

        unsafe fn set_table<'w>(
            fetch: &mut Self::Fetch<'w>,
            &component_id: &Self::State,
            table: &'w Table,
        ) {
            fetch.table_components = Some(
                table
                    .get_column(component_id)
                    .debug_checked_unwrap()
                    .get_data_slice()
                    .into(),
            );
        }

        unsafe fn fetch<'w>(
            fetch: &mut Self::Fetch<'w>,
            entity: Entity,
            table_row: TableRow,
        ) -> Self::Item<'w> {
            match T::STORAGE_TYPE {
                StorageType::Table => {
                    // SAFETY: STORAGE_TYPE = Table
                    let table = unsafe {
                        fetch.table_components.debug_checked_unwrap()
                    };
                    // SAFETY: Caller ensures `table_row` is in range.
                    let item = unsafe { table.get(table_row.as_usize()) };
                    unsafe { &*item.get() }
                }
                StorageType::SparseSet => {
                    _todo::for_sparse::_sparse();
                    // SAFETY: STORAGE_TYPE = SparseSet
                    let sparse_set =
                        unsafe { fetch.sparse_set.debug_checked_unwrap() };
                    // SAFETY: Caller ensures `entity` is in range.
                    let item = unsafe {
                        sparse_set.get(entity).debug_checked_unwrap()
                    };
                    item.deref()
                }
            }
        }

        fn init_state(world: &mut World) -> Self::State {
            world.init_component::<T>()
        }

        fn get_state(components: &Components) -> Option<Self::State> {
            components.component_id::<T>()
        }

        fn matches_component_set(
            &state: &Self::State,
            set_contains_id: &impl Fn(ComponentId) -> bool,
        ) -> bool {
            set_contains_id(state)
        }
    }

    unsafe impl WorldQuery for Entity {
        type Item<'w> = Entity;

        type Fetch<'w> = ();

        type State = ();

        fn shrink<'wlong: 'wshort, 'wshort>(
            item: Self::Item<'wlong>,
        ) -> Self::Item<'wshort> {
            item
        }

        unsafe fn init_fetch<'w>(
            _world: UnsafeWorldCell<'w>,
            _state: &Self::State,
            _last_run: Tick,
            _this_run: Tick,
        ) -> Self::Fetch<'w> {
        }

        const IS_DENSE: bool= true;

        unsafe fn set_archetype<'w>(
            _fetch: &mut Self::Fetch<'w>,
            _state: &Self::State,
            _archetype: &'w Archetype,
            _table: &'w Table,
        ) {
        }

        unsafe fn set_table<'w>(
            _fetch: &mut Self::Fetch<'w>,
            _state: &Self::State,
            _table: &'w Table,
        ) {
        }

        unsafe fn fetch<'w>(
            _fetch: &mut Self::Fetch<'w>,
            entity: Entity,
            _table_row: TableRow,
        ) -> Self::Item<'w> {
            entity
        }

        fn init_state(_world: &mut World) -> Self::State {
        }

        fn get_state(_components: &Components) -> Option<Self::State> {
            Some(())
        }

        fn matches_component_set(
            _state: &Self::State,
            _set_contains_id: &impl Fn(ComponentId) -> bool,
        ) -> bool {
            true
        }
    }

}
use std::{
    alloc::Layout,
    cell::UnsafeCell,
    ops::{Index, IndexMut},
};

use paddy_ptr::{OwningPtr, Ptr, PtrMut, UnsafeCellDeref};
use paddy_utils::hash::HashMap;

use super::sparse_set::{ImmutableSparseSet, SparseSet};
use crate::{
    component::{
        tick::{ComponentTicks, Tick},
        ComponentId, ComponentInfo, Components,
    },
    debug::DebugCheckedUnwrap,
    entity::Entity,
    storage::blob_vec::BlobVec,
};

/// 在一个World中唯一的Table id (多World中不唯一)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TableId(u32);

impl TableId {
    /// 无效的TableId
    pub(crate) const INVALID: TableId = TableId(u32::MAX);

    #[inline]
    pub const fn from_u32(index: u32) -> Self {
        Self(index)
    }
    #[inline]
    pub const fn from_usize(index: usize) -> Self {
        debug_assert!(index as u32 as usize == index);
        Self(index as u32)
    }

    #[inline]
    pub const fn as_u32(self) -> u32 {
        self.0
    }
    #[inline]
    pub const fn as_usize(self) -> usize {
        self.0 as usize
    }

    /// The [`TableId`] of the [`Table`] without any components.
    #[inline]
    pub const fn empty() -> Self {
        Self(0)
    }
}

/// 表示Table中的一行
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TableRow(u32);

impl TableRow {
    /// 无效的TableRow
    pub(crate) const INVALID: TableRow = TableRow(u32::MAX);

    #[inline]
    pub const fn from_u32(index: u32) -> Self {
        Self(index)
    }
    #[inline]
    pub const fn from_usize(index: usize) -> Self {
        debug_assert!(index as u32 as usize == index);
        Self(index as u32)
    }

    #[inline]
    pub const fn as_usize(self) -> usize {
        self.0 as usize
    }
    #[inline]
    pub const fn as_u32(self) -> u32 {
        self.0
    }
}

/// Table的一列\
/// 是一组相同组件类型的集合\
///
/// 一个类型擦除的连续的容器，用于存储同质类型的数据
///
/// 从概念上讲，[`Column`] 非常类似于一个类型擦除的 `Vec<T>`
///
#[derive(Debug)]
pub(crate) struct Column {
    data: BlobVec,
    added_ticks: Vec<UnsafeCell<Tick>>,
    changed_ticks: Vec<UnsafeCell<Tick>>,
}

impl Column {
    /// @return 元素的内存布局信息
    #[inline]
    pub fn item_layout(&self) -> Layout {
        self.data.layout()
    }
    /// @return 当前元素数量
    #[inline]
    pub fn len(&self) -> usize {
        self.data.len()
    }
    /// @return true : is empty
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// 构造一个新的 [`Column`]，它配置了组件的布局并具有初始的容量(`capacity`)
    #[inline]
    pub(crate) fn with_capacity(
        component_info: &ComponentInfo,
        capacity: usize,
    ) -> Self {
        Column {
            // SAFETY: component_info.drop() is valid for the types that will be inserted.
            data: unsafe {
                BlobVec::new(
                    component_info.layout(),
                    component_info.drop(),
                    capacity,
                )
            },
            added_ticks: Vec::with_capacity(capacity),
            changed_ticks: Vec::with_capacity(capacity),
        }
    }

    /// 将组件数据写入指定行的列中
    ///
    /// 对应空间未初始化，不调用 drop\
    /// 如果要覆盖现有的已初始化值，请使用 [`Self::replace`]
    ///
    /// # Safety
    /// - 假设数据已经为指定的行分配好了空间
    /// - @`data` 需是指向正确的类型(被类型擦出前的类型)
    #[inline]
    pub(crate) unsafe fn initialize(
        &mut self,
        row: TableRow,
        data: OwningPtr<'_>,
        tick: Tick,
    ) {
        debug_assert!(row.as_usize() < self.len());
        self.data.initialize_unchecked(row.as_usize(), data);
        *self.added_ticks.get_unchecked_mut(row.as_usize()).get_mut() = tick;
        *self
            .changed_ticks
            .get_unchecked_mut(row.as_usize())
            .get_mut() = tick;
    }

    /// 从 `other` 的 `src_row`行 移除元素, 并将其插入到当前 column 中，以初始化 `dst_row`行 的值\
    /// 即 将 `other`的`src_row`行的元素 移动 到 `self`的`dst_row`行中
    ///
    /// 不进行边界检查
    ///
    /// # 安全性
    ///
    /// - `other` 必须与 `self` 具有相同的内存布局
    /// - `src_row` 必须在 `other` 的有效范围内
    /// - `dst_row` 必须在 `self` 的有效范围内
    /// - `other[src_row]` 必须初始化为有效的值
    /// - `self[dst_row]` 尚未被初始化
    #[inline]
    pub(crate) unsafe fn initialize_from_unchecked(
        &mut self,
        other: &mut Column,
        src_row: TableRow,
        dst_row: TableRow,
    ) {
        debug_assert!(self.data.layout() == other.data.layout());
        let ptr = self.data.get_unchecked_mut(dst_row.as_usize());
        other.data.swap_remove_unchecked(src_row.as_usize(), ptr);
        *self.added_ticks.get_unchecked_mut(dst_row.as_usize()) =
            other.added_ticks.swap_remove(src_row.as_usize());
        *self.changed_ticks.get_unchecked_mut(dst_row.as_usize()) =
            other.changed_ticks.swap_remove(src_row.as_usize());
    }

    /// 将组件数据写入指定行的列中 (用于覆盖数据)
    ///
    /// 若对应空间已经初始化，则会调用 drop\
    ///
    /// # Safety
    /// - 假设数据已经为指定的行分配好了空间
    /// - @`data` 需是指向正确的类型(被类型擦出前的类型)
    #[inline]
    pub(crate) unsafe fn replace(
        &mut self,
        row: TableRow,
        data: OwningPtr<'_>,
        change_tick: Tick,
    ) {
        debug_assert!(row.as_usize() < self.len());
        self.data.replace_unchecked(row.as_usize(), data);
        *self
            .changed_ticks
            .get_unchecked_mut(row.as_usize())
            .get_mut() = change_tick;
    }

    /// 将一个新值添加到此 [`Column`] 的末尾
    ///
    /// # Safety
    /// `ptr` 必须指向此列的 组件类型 的有效数据
    pub(crate) unsafe fn push(
        &mut self,
        ptr: OwningPtr<'_>,
        ticks: ComponentTicks,
    ) {
        self.data.push(ptr);
        self.added_ticks.push(UnsafeCell::new(ticks.added));
        self.changed_ticks.push(UnsafeCell::new(ticks.changed));
    }

    /// 将剩余容量扩展到 additional 大小\
    /// 若 剩余容量>=additional 则 啥也不做
    #[inline]
    pub(crate) fn reserve_exact(&mut self, additional: usize) {
        self.data.reserve_exact(additional);
        self.added_ticks.reserve_exact(additional);
        self.changed_ticks.reserve_exact(additional);
    }

    /// 获取 `row` 行的数据的只读引用
    ///
    /// @return 如果 `row` 越界，则返回 `None`
    #[inline]
    pub fn get_data(&self, row: TableRow) -> Option<Ptr<'_>> {
        (row.as_usize() < self.data.len()).then(|| {
            // SAFETY: The row is length checked before fetching the pointer. This is being
            // accessed through a read-only reference to the column.
            unsafe { self.data.get_unchecked(row.as_usize()) }
        })
    }

    /// 获取 `row` 行的数据的只读引用\
    /// 与 [`Column::get_data`] 不同，此方法不进行边界检查
    ///
    /// # Safety
    /// - `row` 必须在范围 `[0, self.len())` 内
    /// - 同一行的数据在同一时间不能存在其他可变引用
    #[inline]
    pub unsafe fn get_data_unchecked(&self, row: TableRow) -> Ptr<'_> {
        debug_assert!(row.as_usize() < self.data.len());
        self.data.get_unchecked(row.as_usize())
    }

    /// 获取指向 `row`行的数据 的可变引用
    ///
    /// @return 如果 `row` 越界，则返回 `None`
    #[inline]
    pub fn get_data_mut(&mut self, row: TableRow) -> Option<PtrMut<'_>> {
        (row.as_usize() < self.data.len()).then(|| {
            // SAFETY: The row is length checked before fetching the pointer. This is being
            // accessed through an exclusive reference to the column.
            unsafe { self.data.get_unchecked_mut(row.as_usize()) }
        })
    }

    /// 获取指向 `row`行的数据 的可变引用, 不进行边界检查
    /// # Safety
    /// - `index` 必须在有效范围内
    /// - 在同一时间内，不能存在指向同一行数据的其他引用
    #[inline]
    pub(crate) unsafe fn get_data_unchecked_mut(
        &mut self,
        row: TableRow,
    ) -> PtrMut<'_> {
        debug_assert!(row.as_usize() < self.data.len());
        self.data.get_unchecked_mut(row.as_usize())
    }

    /// 获取指向 [`Column`] 数据的切片，并将其转换为指定的类型
    ///
    /// 注意：存储的值是 [`UnsafeCell`] 类型,
    /// 使用此 API 的用户必须确保对每个元素的访问都遵循 [`UnsafeCell`] 的安全不变量
    ///
    /// # Safety
    /// 类型 `T` 必须是此 column 中元素的类型
    pub unsafe fn get_data_slice<T>(&self) -> &[UnsafeCell<T>] {
        self.data.get_slice()
    }

    /// 从 [`Column`] 中移除一个元素
    ///
    /// # Note
    /// - 如果该值实现了 [`Drop`]，它将被释放
    /// - 这个操作不保证元素的顺序，但它是 O(1) 复杂度的操作
    /// - 这个操作不会进行边界检查
    /// - 被移除的元素将由 [`Column`] 中的最后一个元素替换
    ///
    /// # Safety
    /// `row` 必须在范围 `[0, self.len())` 之内
    ///
    #[inline]
    pub(crate) unsafe fn swap_remove_unchecked(&mut self, row: TableRow) {
        self.data.swap_remove_and_drop_unchecked(row.as_usize());
        self.added_ticks.swap_remove(row.as_usize());
        self.changed_ticks.swap_remove(row.as_usize());
    }

    /// 从 [`Column`] 中移除一个元素，并返回它和它的变更检测计时信息
    /// 这个操作不保证元素的顺序，但它是 O(1) 复杂度的操作，并且不会进行边界检查
    ///
    /// 被移除的元素将由 [`Column`] 中的最后一个元素替换
    ///
    /// 调用者有责任确保被移除的值被释放或使用
    /// 如果不这样做，可能会导致资源未被释放（例如，文件句柄未被释放，内存泄漏等）
    ///
    /// # Safety
    /// `row` 必须在范围 `[0, self.len())` 之内
    #[inline]
    #[must_use = "The returned pointer should be used to dropped the removed component"]
    pub(crate) unsafe fn swap_remove_and_forget_unchecked(
        &mut self,
        row: TableRow,
    ) -> (OwningPtr<'_>, ComponentTicks) {
        let data = self.data.swap_remove_and_forget_unchecked(row.as_usize());
        let added = self.added_ticks.swap_remove(row.as_usize()).into_inner();
        let changed =
            self.changed_ticks.swap_remove(row.as_usize()).into_inner();
        (data, ComponentTicks { added, changed })
    }

    /// 清空此列（`Column`），移除其中的所有值
    ///
    /// 此方法不会影响此 [`Column`] 的已分配容量
    pub fn clear(&mut self) {
        self.data.clear();
        self.added_ticks.clear();
        self.changed_ticks.clear();
    }
}

// for tick
impl Column {
    /// 获取指定 `row` 的 "added" change detection tick
    ///
    /// 如果 `row` 超出范围，返回 `None`
    ///
    /// 注意：存储的值是 [`UnsafeCell`] 类型,
    /// 使用此 API 的用户必须确保对每个元素的访问都遵循 [`UnsafeCell`] 的安全不变量
    #[inline]
    pub fn get_added_tick(&self, row: TableRow) -> Option<&UnsafeCell<Tick>> {
        self.added_ticks.get(row.as_usize())
    }

    /// 获取指定 `row` 的 "changed" change detection tick
    ///
    /// 如果 `row` 超出范围，返回 `None`
    ///
    /// 注意：存储的值是 [`UnsafeCell`] 类型,
    /// 使用此 API 的用户必须确保对每个元素的访问都遵循 [`UnsafeCell`] 的安全不变量
    #[inline]
    pub fn get_changed_tick(&self, row: TableRow) -> Option<&UnsafeCell<Tick>> {
        self.changed_ticks.get(row.as_usize())
    }

    /// 获取指定 `row` 的 change detection ticks
    ///
    /// 如果 `row` 超出范围，返回 `None`
    #[inline]
    pub fn get_ticks(&self, row: TableRow) -> Option<ComponentTicks> {
        if row.as_usize() < self.data.len() {
            // SAFETY: The size of the column has already been checked.
            Some(unsafe { self.get_ticks_unchecked(row) })
        } else {
            None
        }
    }

    /// 获取指定 `row` 的 "added" change detection tick
    ///
    /// 这个函数不进行边界检查
    ///
    /// # 安全性
    /// `row` 必须在范围 `[0, self.len())` 内
    #[inline]
    pub unsafe fn get_added_tick_unchecked(
        &self,
        row: TableRow,
    ) -> &UnsafeCell<Tick> {
        debug_assert!(row.as_usize() < self.added_ticks.len());
        self.added_ticks.get_unchecked(row.as_usize())
    }

    /// 获取指定 `row` 的 "changed" change detection tick
    ///
    /// 这个函数不进行边界检查
    ///
    /// # 安全性
    /// `row` 必须在范围 `[0, self.len())` 内
    #[inline]
    pub unsafe fn get_changed_tick_unchecked(
        &self,
        row: TableRow,
    ) -> &UnsafeCell<Tick> {
        debug_assert!(row.as_usize() < self.changed_ticks.len());
        self.changed_ticks.get_unchecked(row.as_usize())
    }

    /// Fetches the change detection ticks for the value at `row`. Unlike [`Column::get_ticks`]
    /// this function does not do any bounds checking.
    ///
    /// # Safety
    /// `row` must be within the range `[0, self.len())`.

    /// 获取指定 `row` 的 change detection ticks
    ///
    /// 这个函数不进行边界检查
    ///
    /// # 安全性
    /// `row` 必须在范围 `[0, self.len())` 内
    #[inline]
    pub unsafe fn get_ticks_unchecked(&self, row: TableRow) -> ComponentTicks {
        debug_assert!(row.as_usize() < self.added_ticks.len());
        debug_assert!(row.as_usize() < self.changed_ticks.len());
        ComponentTicks {
            added: self.added_ticks.get_unchecked(row.as_usize()).read(),
            changed: self.changed_ticks.get_unchecked(row.as_usize()).read(),
        }
    }

    #[inline]
    pub(crate) fn check_change_ticks(&mut self, change_tick: Tick) {
        for component_ticks in &mut self.added_ticks {
            component_ticks.get_mut().check_tick(change_tick);
        }
        for component_ticks in &mut self.changed_ticks {
            component_ticks.get_mut().check_tick(change_tick);
        }
    }
}

/// 用于构建 [`Table`] 的构建器类型
///
///  - 使用 [`with_capacity`](Self::with_capacity) 来初始化构建器
///  - 反复调用 [`add_column`](Self::add_column) 添加组件的列
///  - 最后用 [`build`](Self::build) 来构建 [`Table`]
///
pub(crate) struct TableBuilder {
    columns: SparseSet<ComponentId, Column>,
    capacity: usize,
}

impl TableBuilder {
    /// `column_capacity` 表示 Table的列数
    ///
    /// 每列的初始容量为 `capacity`
    pub fn with_capacity(capacity: usize, column_capacity: usize) -> Self {
        Self {
            columns: SparseSet::with_capacity(column_capacity),
            capacity,
        }
    }

    #[must_use]
    pub fn add_column(mut self, component_info: &ComponentInfo) -> Self {
        self.columns.insert(
            component_info.id(),
            Column::with_capacity(component_info, self.capacity),
        );
        self
    }

    #[must_use]
    pub fn build(self) -> Table {
        Table {
            columns: self.columns.into_immutable(),
            entities: Vec::with_capacity(self.capacity),
        }
    }
}

/// Table 中保存 Entity的Archetype数据\
/// 每一个 Table 对应着一个特定的组件组合(Archetype)
///
/// ```no_run
/// 若 Archetype 包含 Component1,Component2 ,则Table是:
/// +------------+------------+------------+
/// | Entity ID  | Component1 | Component2 |
/// +------------+------------+------------+
/// | Entity 1   | (x1, y1)   | (vx1, vy1) |
/// | Entity 2   | (x2, y2)   | (vx2, vy2) |
/// | ...        | ...        | ...        |
/// +------------+------------+------------+
/// ```
///
///
#[derive(Debug)]
pub(crate) struct Table {
    /// #note : 你在任何情况都不应该添加key或删除key, Table被构造后就是对应于固定的原型
    columns: ImmutableSparseSet<ComponentId, Column>,
    /// 存储在当前Table的Entity
    entities: Vec<Entity>,
}

impl Table {
    /// 获取存储在 [`Table`] 中的Entity的只读切片
    #[inline]
    pub fn entities(&self) -> &[Entity] {
        &self.entities
    }

    /// 检查Table中是否存在Entity
    ///
    /// @return : ture is empty
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }

    /// 获取当前Table中存储的Entity数量
    #[inline]
    pub fn entity_count(&self) -> usize {
        self.entities.len()
    }

    /// 获取当前Table中存储的Component数量
    #[inline]
    pub fn component_count(&self) -> usize {
        self.columns.len()
    }

    /// 获取当前Table在不重新分配底层内存的情况下能够存储的最大Entity数量\
    /// 即获取Table中Entity Vec的容量
    #[inline]
    pub fn entity_capacity(&self) -> usize {
        self.entities.capacity()
    }

    /// 获取表中给定 [`Component`](crate::component::Component) 的 [`Column`] 的只读引用
    ///
    /// 如果相应的Component不属于该Table，返回 `None`
    ///
    #[inline]
    pub fn get_column(&self, component_id: ComponentId) -> Option<&Column> {
        self.columns.get(component_id)
    }

    /// 获取表中给定 [`Component`](crate::component::Component) 的 [`Column`] 的可变引用
    ///
    /// 如果相应的Component不属于该Table，返回 `None`
    ///
    #[inline]
    pub(crate) fn get_column_mut(
        &mut self,
        component_id: ComponentId,
    ) -> Option<&mut Column> {
        self.columns.get_mut(component_id)
    }

    /// 检查Table是否包含给定 [`Component`] 的 [`Column`]
    ///
    /// 如果该column存在，返回 `true`，否则返回 `false`
    ///
    /// [`Component`]: crate::component::Component
    #[inline]
    pub fn has_column(&self, component_id: ComponentId) -> bool {
        self.columns.contains(component_id)
    }

    /// 移除给定行的Entity
    ///
    /// @return 若`row`是table的最后一行,则返回`None`,
    /// 否则它将返回 替换`row`行的Entity (往往是table的最后一行的Entity)
    ///
    /// # 安全性
    /// `row` 必须在有效范围内
    pub(crate) unsafe fn swap_remove_unchecked(
        &mut self,
        row: TableRow,
    ) -> Option<Entity> {
        for column in self.columns.values_mut() {
            column.swap_remove_unchecked(row);
        }
        let is_last = row.as_usize() == self.entities.len() - 1;
        self.entities.swap_remove(row.as_usize());
        if is_last {
            None
        } else {
            Some(self.entities[row.as_usize()])
        }
    }

    /// 将`self`中的`row`行 移动到 `new_table` 中
    ///
    /// 返回类型包含 `new_table`中的行(被移动后的Entity 所属的行) 和 `self`中补充到`row`行的Entity
    ///
    /// missing columns will be "forgotten". It is
    /// the caller's responsibility to drop them.  Failure to do so may result in resources not
    /// being released (i.e. files handles not being released, memory leaks, etc.)\
    /// 缺失的列将被“遗忘”。调用者有责任释放它们。未能释放它们可能导致资源未释放（例如文件句柄未释放、内存泄漏等）
    ///
    /// # 安全性
    /// `row` 必须在有效范围内
    pub(crate) unsafe fn move_to_and_forget_missing_unchecked(
        &mut self,
        row: TableRow,
        new_table: &mut Table,
    ) -> TableMoveResult {
        debug_assert!(row.as_usize() < self.entity_count());
        let is_last = row.as_usize() == self.entities.len() - 1;
        let new_row =
            new_table.allocate(self.entities.swap_remove(row.as_usize()));
        for (component_id, column) in self.columns.iter_mut() {
            // 若存在匹配的Component,则移动进new_table,否则 将Component从 old_table移除
            if let Some(new_column) = new_table.get_column_mut(*component_id) {
                new_column.initialize_from_unchecked(column, row, new_row);
            } else {
                // It's the caller's responsibility to drop these cases.
                // ? 这咋释放内存 ? 难道默认的drop会进行释放?
                let (_, _) = column.swap_remove_and_forget_unchecked(row);
            }
        }
        TableMoveResult {
            new_row,
            swapped_entity: if is_last {
                None
            } else {
                Some(self.entities[row.as_usize()])
            },
        }
    }

    /// 将`self`中的`row`行 移动到 `new_table` 中
    ///
    /// 返回类型包含 `new_table`中的行(被移动后的Entity 所属的行) 和 `self`中补充到`row`行的Entity
    ///
    /// # 安全性
    /// `row` 必须在有效范围内
    pub(crate) unsafe fn move_to_and_drop_missing_unchecked(
        &mut self,
        row: TableRow,
        new_table: &mut Table,
    ) -> TableMoveResult {
        debug_assert!(row.as_usize() < self.entity_count());
        let is_last = row.as_usize() == self.entities.len() - 1;
        let new_row =
            new_table.allocate(self.entities.swap_remove(row.as_usize()));
        for (component_id, column) in self.columns.iter_mut() {
            if let Some(new_column) = new_table.get_column_mut(*component_id) {
                new_column.initialize_from_unchecked(column, row, new_row);
            } else {
                column.swap_remove_unchecked(row);
            }
        }
        TableMoveResult {
            new_row,
            swapped_entity: if is_last {
                None
            } else {
                Some(self.entities[row.as_usize()])
            },
        }
    }

    /// 将`self`中的`row`行 移动到 `new_table` 中
    ///
    /// 返回类型包含 `new_table`中的行(被移动后的Entity 所属的行) 和 `self`中补充到`row`行的Entity
    ///
    /// # 安全性
    /// - `row` 必须在有效范围内
    /// - `new_table` 必须包含此table中的每个组件
    pub(crate) unsafe fn move_to_superset_unchecked(
        &mut self,
        row: TableRow,
        new_table: &mut Table,
    ) -> TableMoveResult {
        debug_assert!(row.as_usize() < self.entity_count());
        let is_last = row.as_usize() == self.entities.len() - 1;
        let new_row =
            new_table.allocate(self.entities.swap_remove(row.as_usize()));
        for (component_id, column) in self.columns.iter_mut() {
            new_table
                .get_column_mut(*component_id)
                .debug_checked_unwrap()
                .initialize_from_unchecked(column, row, new_row);
        }
        TableMoveResult {
            new_row,
            swapped_entity: if is_last {
                None
            } else {
                Some(self.entities[row.as_usize()])
            },
        }
    }

    /// 扩展剩余容量
    pub(crate) fn reserve(&mut self, additional: usize) {
        if self.entities.capacity() - self.entities.len() < additional {
            self.entities.reserve(additional);

            // use entities vector capacity as driving capacity for all related allocations
            let new_capacity = self.entities.capacity();

            for column in self.columns.values_mut() {
                column.reserve_exact(new_capacity - column.len());
            }
        }
    }

    /// 为一个新的Entity 在Table中分配空间
    ///
    /// #note: 归属于这个Entity的所有Component并未初始化
    ///
    /// # Safety
    /// - the allocated row must be written to immediately with valid values in each column\
    ///   分配的行必须立即把每一列都写入有效值
    pub(crate) unsafe fn allocate(&mut self, entity: Entity) -> TableRow {
        self.reserve(1);
        let index = self.entities.len();
        self.entities.push(entity);
        for column in self.columns.values_mut() {
            column.data.set_len(self.entities.len());
            column.added_ticks.push(UnsafeCell::new(Tick::new(0)));
            column.changed_ticks.push(UnsafeCell::new(Tick::new(0)));
        }
        TableRow::from_usize(index)
    }

    pub(crate) fn check_change_ticks(&mut self, change_tick: Tick) {
        for column in self.columns.values_mut() {
            column.check_change_ticks(change_tick);
        }
    }

    /// @return [`Table`]中[`Column`]的迭代器
    pub fn iter(&self) -> impl Iterator<Item = &Column> {
        self.columns.values()
    }

    /// 清除 [`Table`] 中所有存储的Entity和Component数据,但容量不变
    pub(crate) fn clear(&mut self) {
        self.entities.clear();
        for column in self.columns.values_mut() {
            column.clear();
        }
    }
}

pub(crate) struct TableMoveResult {
    /// old table 中 行被移动后 补充到 被移动行 的Entity
    pub swapped_entity: Option<Entity>,
    /// new table 的 行
    pub new_row: TableRow,
}

/// Table 是没必要摧毁的,分配id后就永远是这个id
#[derive(Debug)]
pub(crate) struct Tables {
    /// 下标 是 Table id
    tables: Vec<Table>,
    ///
    table_ids: HashMap<Box<[ComponentId]>, TableId>,
}

impl Tables {
    #[inline]
    pub fn len(&self) -> usize {
        self.tables.len()
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.tables.is_empty()
    }
    #[inline]
    pub fn get(&self, id: TableId) -> Option<&Table> {
        self.tables.get(id.as_usize())
    }

    /// 获取两个不同的 [`Table`] 的可变引用
    ///
    //  #play : 哇~这玩意咋还在借用检查器里,难怪它要写成这样,又学一招挑逗编译器a
    /// # Panics
    ///
    /// 如果 `a` 和 `b` 相等，会导致 panic
    #[inline]
    pub(crate) fn get_2_mut(
        &mut self,
        a: TableId,
        b: TableId,
    ) -> (&mut Table, &mut Table) {
        if a.as_usize() > b.as_usize() {
            let (b_slice, a_slice) = self.tables.split_at_mut(a.as_usize());
            (&mut a_slice[0], &mut b_slice[b.as_usize()])
        } else {
            let (a_slice, b_slice) = self.tables.split_at_mut(b.as_usize());
            (&mut a_slice[a.as_usize()], &mut b_slice[0])
        }
    }

    /// 尝试根据提供的`component_ids` 获取一个Table，
    /// 如果Table不存在，则创建一个新的[`Table`]并返回[`TableId`] 
    ///
    /// # 安全性
    /// `component_ids`中的元素(ComponentId) 必须 在 `components` 中有记录
    pub(crate) unsafe fn get_id_or_insert(
        &mut self,
        component_ids: &[ComponentId],
        components: &Components,
    ) -> TableId {
        let tables = &mut self.tables;
        let (_key, value) = self
            .table_ids
            .raw_entry_mut()
            .from_key(component_ids)
            .or_insert_with(|| {
                let mut table =
                    TableBuilder::with_capacity(0, component_ids.len());
                for component_id in component_ids {
                    table = table.add_column(
                        components.get_info_unchecked(*component_id),
                    );
                }
                tables.push(table.build());
                (component_ids.into(), TableId::from_usize(tables.len() - 1))
            });
        *value
    }

    /// 按 [`TableId`] 顺序(0..)迭代所有存储的表
    pub fn iter(&self) -> std::slice::Iter<'_, Table> {
        self.tables.iter()
    }

    /// 清除所有 [`Table`] 中的所有数据 , 但容量不变
    pub(crate) fn clear(&mut self) {
        for table in &mut self.tables {
            table.clear();
        }
    }

    pub(crate) fn check_change_ticks(&mut self, change_tick: Tick) {
        for table in &mut self.tables {
            table.check_change_ticks(change_tick);
        }
    }
}

impl Default for Tables {
    fn default() -> Self {
        let empty_table = TableBuilder::with_capacity(0, 0).build();
        Tables {
            tables: vec![empty_table],
            table_ids: HashMap::default(),
        }
    }
}

impl Index<TableId> for Tables {
    type Output = Table;
    #[inline]
    fn index(&self, index: TableId) -> &Self::Output {
        &self.tables[index.as_usize()]
    }
}

impl IndexMut<TableId> for Tables {
    #[inline]
    fn index_mut(&mut self, index: TableId) -> &mut Self::Output {
        paddy_utils::dbg(index);
        paddy_utils::dbg(self.tables.capacity());

        &mut self.tables[index.as_usize()]
    }
}

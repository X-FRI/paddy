use std::{
    alloc::Layout,
    cell::UnsafeCell,
    collections::HashMap,
    ops::{Index, IndexMut},
    ptr::NonNull,
};

use paddy_ptr::{OwningPtr, Ptr, PtrMut};

use crate::{
    component::{
        tick::{ComponentTicks, Tick},
        ComponentId, ComponentInfo,
    },
    storage::blob_vec::BlobVec,
};

use crate::entity::Entity;

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
    #[inline]
    pub fn item_layout(&self) -> Layout {
        self.data.layout()
    }
    #[inline]
    pub fn len(&self) -> usize {
        self.data.len()
    }
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
        change_tick: Tick
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

    /// 获取 `row` 行的数据的可变引用
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

    /// Fetches the slice to the [`Column`]'s data cast to a given type.
    ///
    /// Note: The values stored within are [`UnsafeCell`].
    /// Users of this API must ensure that accesses to each individual element
    /// adhere to the safety invariants of [`UnsafeCell`].
    ///
    /// # Safety
    /// The type `T` must be the type of the items in this column.
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

    /// 从 [`Column`] 中移除一个元素，并返回它和它的变更检测计时信息(暂时不打算加入Tick,未来可能加入,所以文档不变)
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
    columns: HashMap<ComponentId, Column>,
    /// 存储在当前Table的Entity
    entities: Vec<Entity>,
}

impl Table {
    ///  获取存储在 [`Table`] 中的Entity的只读切片
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

    /// Fetches a read-only reference to the [`Column`] for a given [`Component`] within the
    /// table.
    ///
    /// Returns `None` if the corresponding component does not belong to the table.
    ///
    /// [`Component`]: crate::component::Component
    #[inline]
    pub fn get_column(&self, component_id: ComponentId) -> Option<&Column> {
        self.columns.get(&component_id)
    }

    /// Fetches a mutable reference to the [`Column`] for a given [`Component`] within the
    /// table.
    ///
    /// Returns `None` if the corresponding component does not belong to the table.
    ///
    /// [`Component`]: crate::component::Component
    #[inline]
    pub(crate) fn get_column_mut(
        &mut self,
        component_id: ComponentId,
    ) -> Option<&mut Column> {
        self.columns.get_mut(&component_id)
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

    /// 为一个新的Entity分配空间
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
        &mut self.tables[index.as_usize()]
    }
}

use std::{
    alloc::Layout,
    collections::HashMap,
    ops::{Index, IndexMut},
    ptr::NonNull,
};

use paddy_ptr::{OwningPtr, Ptr, PtrMut};

use crate::{
    component::{ComponentId, ComponentInfo},
    storage::blob_vec::BlobVec,
};

use super::Entity;

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
struct Column {
    data: BlobVec,
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
    pub(crate) fn with_capacity(component_info: &ComponentInfo, capacity: usize) -> Self {
        Column {
            // SAFETY: component_info.drop() is valid for the types that will be inserted.
            data: unsafe { BlobVec::new(component_info.layout(), component_info.drop(), capacity) },
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
    pub(crate) unsafe fn initialize(&mut self, row: TableRow, data: OwningPtr<'_>) {
        debug_assert!(row.as_usize() < self.len());
        self.data.initialize_unchecked(row.as_usize(), data);
    }

    /// 将组件数据写入指定行的列中 (用于覆盖数据)
    ///
    /// 若对应空间已经初始化，则会调用 drop\
    ///
    /// # Safety
    /// - 假设数据已经为指定的行分配好了空间
    /// - @`data` 需是指向正确的类型(被类型擦出前的类型)
    #[inline]
    pub(crate) unsafe fn replace(&mut self, row: TableRow, data: OwningPtr<'_>) {
        debug_assert!(row.as_usize() < self.len());
        self.data.replace_unchecked(row.as_usize(), data);
    }

    /// 将一个新值添加到此 [`Column`] 的末尾
    ///
    /// # Safety
    /// `ptr` 必须指向此列的 组件类型 的有效数据
    pub(crate) unsafe fn push(&mut self, ptr: OwningPtr<'_>) {
        self.data.push(ptr);
    }

    /// 将剩余容量扩展到 additional 大小\
    /// 若 剩余容量>=additional 则 啥也不做
    #[inline]
    pub(crate) fn reserve_exact(&mut self, additional: usize) {
        self.data.reserve_exact(additional);
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

    /// 清空此列（`Column`），移除其中的所有值
    ///
    /// 此方法不会影响此 [`Column`] 的已分配容量
    pub fn clear(&mut self) {
        self.data.clear();
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

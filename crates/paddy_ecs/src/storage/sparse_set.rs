use core::hash::Hash;
use std::marker::PhantomData;

use nonmax::NonMaxUsize;
use paddy_ptr::{OwningPtr, Ptr};

use crate::{
    component::{
        tick::{ComponentTicks, Tick},
        ComponentId, ComponentInfo,
    },
    entity::Entity,
};

use super::table::{Column, TableRow};

/// Represents something that can be stored in a [`SparseSet`] as an integer.\
/// 表示可以作为整数存储在 [`SparseSet`] 中的某种类型
///
/// Ideally, the `usize` values should be very small (ie: incremented starting from
/// zero), as the number of bits needed to represent a `SparseSetIndex` in a `FixedBitSet`
/// is proportional to the **value** of those `usize`.\
/// 理想情况下，`usize` 值应该非常小（即：从零开始递增），因为在 `FixedBitSet` 中表示 `SparseSetIndex` 所需的位数与这些 `usize` 值的 **大小** 成正比
///
/// 它定义了一种类型，该类型可以作为整数索引 存储在稀疏集合的元素
pub trait SparseSetIndex: Clone + PartialEq + Eq + Hash {
    /// Gets the sparse set index corresponding to this instance.\
    /// 获取与此实例对应的稀疏集合索引
    fn sparse_set_index(&self) -> usize;
    /// Creates a new instance of this type with the specified index.\
    /// 使用指定的索引创建此类型的新实例
    fn get_sparse_set_index(value: usize) -> Self;
}

/// 为一组类型实现 [`SparseSetIndex`]
macro_rules! impl_sparse_set_index {
    ($($ty:ty),+) => {
        $(impl SparseSetIndex for $ty {
            #[inline]
            fn sparse_set_index(&self) -> usize {
                *self as usize
            }

            #[inline]
            fn get_sparse_set_index(value: usize) -> Self {
                value as $ty
            }
        })*
    };
}
impl_sparse_set_index!(u8, u16, u32, u64, usize);

macro_rules! impl_sparse_array {
    ($ty:ident) => {
        impl<I: SparseSetIndex, V> $ty<I, V> {
            /// Returns `true` if the collection contains a value for the specified `index`.
            #[inline]
            pub fn contains(&self, index: I) -> bool {
                let index = index.sparse_set_index();
                self.values.get(index).map(|v| v.is_some()).unwrap_or(false)
            }

            /// Returns a reference to the value at `index`.
            ///
            /// Returns `None` if `index` does not have a value or if `index` is out of bounds.
            #[inline]
            pub fn get(&self, index: I) -> Option<&V> {
                let index = index.sparse_set_index();
                self.values.get(index).map(|v| v.as_ref()).unwrap_or(None)
            }
        }
    };
}
impl_sparse_array!(SparseArray);
impl_sparse_array!(ImmutableSparseArray);

macro_rules! impl_sparse_set {
    ($ty:ident) => {
        impl<I: SparseSetIndex, V> $ty<I, V> {
            /// Returns the number of elements in the sparse set.
            #[inline]
            pub fn len(&self) -> usize {
                self.dense.len()
            }

            /// Returns `true` if the sparse set contains a value for `index`.
            #[inline]
            pub fn contains(&self, index: I) -> bool {
                self.sparse.contains(index)
            }

            /// Returns a reference to the value for `index`.
            ///
            /// Returns `None` if `index` does not have a value in the sparse set.
            pub fn get(&self, index: I) -> Option<&V> {
                self.sparse.get(index).map(|dense_index| {
                    // SAFETY: if the sparse index points to something in the dense vec, it exists
                    unsafe { self.dense.get_unchecked(dense_index.get()) }
                })
            }

            /// Returns a mutable reference to the value for `index`.
            ///
            /// Returns `None` if `index` does not have a value in the sparse set.
            pub fn get_mut(&mut self, index: I) -> Option<&mut V> {
                let dense = &mut self.dense;
                self.sparse.get(index).map(move |dense_index| {
                    // SAFETY: if the sparse index points to something in the dense vec, it exists
                    unsafe { dense.get_unchecked_mut(dense_index.get()) }
                })
            }

            /// Returns an iterator visiting all keys (indices) in arbitrary order.
            pub fn indices(&self) -> impl Iterator<Item = I> + '_ {
                self.indices.iter().cloned()
            }

            /// Returns an iterator visiting all values in arbitrary order.
            pub fn values(&self) -> impl Iterator<Item = &V> {
                self.dense.iter()
            }

            /// Returns an iterator visiting all values mutably in arbitrary order.
            pub fn values_mut(&mut self) -> impl Iterator<Item = &mut V> {
                self.dense.iter_mut()
            }

            /// Returns an iterator visiting all key-value pairs in arbitrary order, with references to the values.
            pub fn iter(&self) -> impl Iterator<Item = (&I, &V)> {
                self.indices.iter().zip(self.dense.iter())
            }

            /// Returns an iterator visiting all key-value pairs in arbitrary order, with mutable references to the values.
            pub fn iter_mut(&mut self) -> impl Iterator<Item = (&I, &mut V)> {
                self.indices.iter().zip(self.dense.iter_mut())
            }
        }
    };
}

impl_sparse_set!(SparseSet);
impl_sparse_set!(ImmutableSparseSet);


type EntityIndex = u32;

#[derive(Debug)]
pub(crate) struct SparseArray<I, V = I> {
    values: Vec<Option<V>>,
    marker: PhantomData<I>,
}
impl<I: SparseSetIndex, V> Default for SparseArray<I, V> {
    fn default() -> Self {
        Self::new()
    }
}
impl<I, V> SparseArray<I, V> {
    #[inline]
    pub const fn new() -> Self {
        Self {
            values: Vec::new(),
            marker: PhantomData,
        }
    }
}
impl<I: SparseSetIndex, V> SparseArray<I, V> {
    /// 在数组的 `index` 处插入 `value`
    ///
    /// 如果 `index` 超出范围，这将扩大缓冲区以适应它
    #[inline]
    pub fn insert(&mut self, index: I, value: V) {
        let index = index.sparse_set_index();
        if index >= self.values.len() {
            self.values.resize_with(index + 1, || None);
        }
        self.values[index] = Some(value);
    }

    /// 返回  `index` 处的 值的可变引用
    ///
    /// 如果 `index` 没有值或超出范围，则返回 `None`
    #[inline]
    pub fn get_mut(&mut self, index: I) -> Option<&mut V> {
        let index = index.sparse_set_index();
        self.values
            .get_mut(index)
            .map(|v| v.as_mut())
            .unwrap_or(None)
    }

    /// 移除并返回存储在 `index` 处的值
    ///
    /// 如果 `index` 没有值或超出范围，则返回 `None`
    #[inline]
    pub fn remove(&mut self, index: I) -> Option<V> {
        let index = index.sparse_set_index();
        self.values.get_mut(index).and_then(|value| value.take())
    }

    /// 清理所有存储的值,但不影响容量
    pub fn clear(&mut self) {
        self.values.clear();
    }

    /// 将 [`SparseArray`] 转换为不可变的变体
    pub(crate) fn into_immutable(self) -> ImmutableSparseArray<I, V> {
        ImmutableSparseArray {
            values: self.values.into_boxed_slice(),
            marker: PhantomData,
        }
    }
}

/// 一种空间优化的 [`SparseArray`] 版本，在构造后无法更改
#[derive(Debug)]
pub(crate) struct ImmutableSparseArray<I, V = I> {
    values: Box<[Option<V>]>,
    marker: PhantomData<I>,
}


/// 一个稀疏的数据结构，用于存储 [`Component`](crate::component::Component)
///
///  设计用于相对快速的插入和删除操作
#[derive(Debug)]
pub struct ComponentSparseSet {
    /// Component实际存储位置
    dense: Column,
    // Internally this only relies on the Entity index to keep track of where the component data is
    // stored for entities that are alive. The generation is not required, but is stored
    // in debug builds to validate that access is correct.
    // 内部仅依赖于 Entity 的索引来跟踪活跃实体的组件数据存储位置。
    // 生成的版本（generation）不是必需的，但在调试版本中会存储，以验证访问的正确性。
    #[cfg(not(debug_assertions))]
    entities: Vec<EntityIndex>,
    #[cfg(debug_assertions)]
    entities: Vec<Entity>,
    /// 映射到dense
    sparse: SparseArray<EntityIndex, TableRow>,
}

impl ComponentSparseSet {
    /// 创建一个 [`ComponentSparseSet`]，具有给定的组件类型布局和初始 `capacity`（容量）
    pub(crate) fn new(component_info: &ComponentInfo, capacity: usize) -> Self {
        Self {
            dense: Column::with_capacity(component_info, capacity),
            entities: Vec::with_capacity(capacity),
            sparse: Default::default(),
        }
    }

    /// 清理所有存储的值
    pub(crate) fn clear(&mut self) {
        self.dense.clear();
        self.entities.clear();
        self.sparse.clear();
    }

    /// @return 返回稀疏集合中的组件值的数量
    #[inline]
    pub fn len(&self) -> usize {
        self.dense.len()
    }

    /// @return 如果稀疏集合不包含任何组件值，则返回 `true`
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.dense.len() == 0
    }
    /// Inserts the `entity` key and component `value` pair into this sparse
    /// set.\
    /// 将 `entity` 键和组件 `value` 对插入到这个稀疏集合中
    ///
    /// # Safety
    /// `value` 指针必须指向一个与构造此稀疏集合时提供的 [`ComponentInfo`] 内部 [`Layout`](std::alloc::Layout)
    /// 匹配的有效地址
    pub(crate) unsafe fn insert(
        &mut self,
        entity: Entity,
        value: OwningPtr<'_>,
        change_tick: Tick,
    ) {
        if let Some(&dense_index) = self.sparse.get(entity.index()) {
            #[cfg(debug_assertions)]
            assert_eq!(entity, self.entities[dense_index.as_usize()]);
            self.dense.replace(dense_index, value, change_tick);
        } else {
            let dense_index = self.dense.len();
            self.dense.push(value, ComponentTicks::new(change_tick));
            self.sparse
                .insert(entity.index(), TableRow::from_usize(dense_index));
            #[cfg(debug_assertions)]
            assert_eq!(self.entities.len(), dense_index);
            #[cfg(not(debug_assertions))]
            self.entities.push(entity.index());
            #[cfg(debug_assertions)]
            self.entities.push(entity);
        }
    }

    /// @return 如果稀疏集合中有提供的 `entity` 的组件值，则返回 `true`
    #[inline]
    pub fn contains(&self, entity: Entity) -> bool {
        #[cfg(debug_assertions)]
        {
            if let Some(&dense_index) = self.sparse.get(entity.index()) {
                #[cfg(debug_assertions)]
                assert_eq!(entity, self.entities[dense_index.as_usize()]);
                true
            } else {
                false
            }
        }
        #[cfg(not(debug_assertions))]
        self.sparse.contains(entity.index())
    }

    /// 返回对 `entity` 的组件值的引用
    ///
    /// 如果 `entity` 在稀疏集合中没有组件，则返回 `None`
    #[inline]
    pub fn get(&self, entity: Entity) -> Option<Ptr<'_>> {
        self.sparse.get(entity.index()).map(|&dense_index| {
            #[cfg(debug_assertions)]
            assert_eq!(entity, self.entities[dense_index.as_usize()]);
            // SAFETY: if the sparse index points to something in the dense vec, it exists
            unsafe { self.dense.get_data_unchecked(dense_index) }
        })
    }

    /// 从这个稀疏集合中移除 `entity` 并返回指向关联值的指针（如果存在）
    #[must_use = "The returned pointer must be used to drop the removed component."]
    pub(crate) fn remove_and_forget(
        &mut self,
        entity: Entity,
    ) -> Option<OwningPtr<'_>> {
        self.sparse.remove(entity.index()).map(|dense_index| {
            #[cfg(debug_assertions)]
            assert_eq!(entity, self.entities[dense_index.as_usize()]);
            self.entities.swap_remove(dense_index.as_usize());
            let is_last = dense_index.as_usize() == self.dense.len() - 1;
            // SAFETY: dense_index was just removed from `sparse`, which ensures that it is valid
            let (value, _) = unsafe {
                self.dense.swap_remove_and_forget_unchecked(dense_index)
            };
            if !is_last {
                let swapped_entity = self.entities[dense_index.as_usize()];
                #[cfg(not(debug_assertions))]
                let index = swapped_entity;
                #[cfg(debug_assertions)]
                let index = swapped_entity.index();
                *self.sparse.get_mut(index).unwrap() = dense_index;
            }
            value
        })
    }

    /// 从稀疏集合中移除（并丢弃）实体的组件值。
    ///
    /// 如果 `entity` 在稀疏集合中有一个组件值，则返回 `true`。
    pub(crate) fn remove(&mut self, entity: Entity) -> bool {
        if let Some(dense_index) = self.sparse.remove(entity.index()) {
            #[cfg(debug_assertions)]
            assert_eq!(entity, self.entities[dense_index.as_usize()]);
            self.entities.swap_remove(dense_index.as_usize());
            let is_last = dense_index.as_usize() == self.dense.len() - 1;
            // SAFETY: if the sparse index points to something in the dense vec, it exists
            unsafe {
                self.dense.swap_remove_unchecked(dense_index);
            }
            if !is_last {
                let swapped_entity = self.entities[dense_index.as_usize()];
                #[cfg(not(debug_assertions))]
                let index = swapped_entity;
                #[cfg(debug_assertions)]
                let index = swapped_entity.index();
                *self.sparse.get_mut(index).unwrap() = dense_index;
            }
            true
        } else {
            false
        }
    }

    pub(crate) fn check_change_ticks(&mut self, change_tick: Tick) {
        self.dense.check_change_ticks(change_tick);
    }
}

/// 一种结合了密集存储和稀疏存储的数据结构
///
/// `I` 是索引的类型，而 `V` 是存储在密集存储中的数据类型
#[derive(Debug)]
pub struct SparseSet<I, V: 'static> {
    dense: Vec<V>,
    indices: Vec<I>,
    sparse: SparseArray<I, NonMaxUsize>,
}

impl<I: SparseSetIndex, V> Default for SparseSet<I, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<I, V> SparseSet<I, V> {
    /// Creates a new [`SparseSet`].
    pub const fn new() -> Self {
        Self {
            dense: Vec::new(),
            indices: Vec::new(),
            sparse: SparseArray::new(),
        }
    }
}
impl<I: SparseSetIndex, V> SparseSet<I, V> {
    /// Creates a new [`SparseSet`] with a specified initial capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            dense: Vec::with_capacity(capacity),
            indices: Vec::with_capacity(capacity),
            sparse: Default::default(),
        }
    }

    /// Returns the total number of elements the [`SparseSet`] can hold without needing to reallocate.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.dense.capacity()
    }

    /// Inserts `value` at `index`.
    ///
    /// If a value was already present at `index`, it will be overwritten.
    pub fn insert(&mut self, index: I, value: V) {
        if let Some(dense_index) = self.sparse.get(index.clone()).cloned() {
            // SAFETY: dense indices stored in self.sparse always exist
            unsafe {
                *self.dense.get_unchecked_mut(dense_index.get()) = value;
            }
        } else {
            self.sparse.insert(
                index.clone(),
                NonMaxUsize::new(self.dense.len()).unwrap(),
            );
            self.indices.push(index);
            self.dense.push(value);
        }
    }

    /// Returns a reference to the value for `index`, inserting one computed from `func`
    /// if not already present.
    pub fn get_or_insert_with(
        &mut self,
        index: I,
        func: impl FnOnce() -> V,
    ) -> &mut V {
        if let Some(dense_index) = self.sparse.get(index.clone()).cloned() {
            // SAFETY: dense indices stored in self.sparse always exist
            unsafe { self.dense.get_unchecked_mut(dense_index.get()) }
        } else {
            let value = func();
            let dense_index = self.dense.len();
            self.sparse
                .insert(index.clone(), NonMaxUsize::new(dense_index).unwrap());
            self.indices.push(index);
            self.dense.push(value);
            // SAFETY: dense index was just populated above
            unsafe { self.dense.get_unchecked_mut(dense_index) }
        }
    }

    /// Returns `true` if the sparse set contains no elements.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.dense.len() == 0
    }

    /// Removes and returns the value for `index`.
    ///
    /// Returns `None` if `index` does not have a value in the sparse set.
    pub fn remove(&mut self, index: I) -> Option<V> {
        self.sparse.remove(index).map(|dense_index| {
            let index = dense_index.get();
            let is_last = index == self.dense.len() - 1;
            let value = self.dense.swap_remove(index);
            self.indices.swap_remove(index);
            if !is_last {
                let swapped_index = self.indices[index].clone();
                *self.sparse.get_mut(swapped_index).unwrap() = dense_index;
            }
            value
        })
    }

    /// Clears all of the elements from the sparse set.
    pub fn clear(&mut self) {
        self.dense.clear();
        self.indices.clear();
        self.sparse.clear();
    }

    /// Converts the sparse set into its immutable variant.
    pub(crate) fn into_immutable(self) -> ImmutableSparseSet<I, V> {
        ImmutableSparseSet {
            dense: self.dense.into_boxed_slice(),
            indices: self.indices.into_boxed_slice(),
            sparse: self.sparse.into_immutable(),
        }
    }
}

/// 一种空间优化的 [`SparseSet`] 版本，在构造后无法更改
#[derive(Debug)]
pub(crate) struct ImmutableSparseSet<I, V: 'static> {
    dense: Box<[V]>,
    indices: Box<[I]>,
    sparse: ImmutableSparseArray<I, NonMaxUsize>,
}

/// 一个由 [`ComponentId`] 索引的 [`ComponentSparseSet`] 存储集合
///
/// 可以通过 [`Storages`](crate::storage::Storages) 访问
#[derive(Debug)]
pub struct SparseSets {
    sets: SparseSet<ComponentId, ComponentSparseSet>,
}

impl SparseSets {
    /// Returns the number of [`ComponentSparseSet`]s this collection contains.
    #[inline]
    pub fn len(&self) -> usize {
        self.sets.len()
    }

    /// Returns true if this collection contains no [`ComponentSparseSet`]s.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.sets.is_empty()
    }

    /// An Iterator visiting all ([`ComponentId`], [`ComponentSparseSet`]) pairs.
    /// NOTE: Order is not guaranteed.
    pub fn iter(
        &self,
    ) -> impl Iterator<Item = (ComponentId, &ComponentSparseSet)> {
        self.sets.iter().map(|(id, data)| (*id, data))
    }

    /// Gets a reference to the [`ComponentSparseSet`] of a [`ComponentId`].
    #[inline]
    pub fn get(
        &self,
        component_id: ComponentId,
    ) -> Option<&ComponentSparseSet> {
        self.sets.get(component_id)
    }

    /// Gets a mutable reference of [`ComponentSparseSet`] of a [`ComponentInfo`].
    /// Create a new [`ComponentSparseSet`] if not exists.
    pub(crate) fn get_or_insert(
        &mut self,
        component_info: &ComponentInfo,
    ) -> &mut ComponentSparseSet {
        if !self.sets.contains(component_info.id()) {
            self.sets.insert(
                component_info.id(),
                ComponentSparseSet::new(component_info, 64),
            );
        }

        self.sets.get_mut(component_info.id()).unwrap()
    }

    /// Gets a mutable reference to the [`ComponentSparseSet`] of a [`ComponentId`].
    pub(crate) fn get_mut(
        &mut self,
        component_id: ComponentId,
    ) -> Option<&mut ComponentSparseSet> {
        self.sets.get_mut(component_id)
    }

    /// Clear entities stored in each [`ComponentSparseSet`]
    pub(crate) fn clear_entities(&mut self) {
        for set in self.sets.values_mut() {
            set.clear();
        }
    }

    pub(crate) fn check_change_ticks(&mut self, change_tick: Tick) {
        for set in self.sets.values_mut() {
            set.check_change_ticks(change_tick);
        }
    }
}

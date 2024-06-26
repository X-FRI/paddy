use core::hash::Hash;
use std::marker::PhantomData;

use nonmax::NonMaxUsize;

use crate::{component::ComponentId, entity::Entity};

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

type EntityIndex = u32;

#[derive(Debug)]
pub(crate) struct SparseArray<I, V = I> {
    values: Vec<Option<V>>,
    marker: PhantomData<I>,
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

/// 一种结合了密集存储和稀疏存储的数据结构
///
/// `I` 是索引的类型，而 `V` 是存储在密集存储中的数据类型
#[derive(Debug)]
pub struct SparseSet<I, V: 'static> {
    dense: Vec<V>,
    indices: Vec<I>,
    sparse: SparseArray<I, NonMaxUsize>,
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

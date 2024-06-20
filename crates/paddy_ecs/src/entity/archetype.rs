use std::{alloc::Layout, any::TypeId, collections::HashMap};

use crate::component::ComponentId;

use super::EntityId;

/// [`Archetype::entities`] 的下标,指向Entity
///
/// 这可以与 [`ArchetypeId`] 结合使用，以找到 [`World`] 中一个 [`Entity`] 的确切位置
///
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
// #safety : 由于对 EntityLocation 的安全要求，必须是 repr(transparent)
#[repr(transparent)]
pub struct ArchetypeRow(u32);

impl ArchetypeRow {
    /// 这是无效 `ArchetypeRow` 的索引
    /// 这个索引用作占位符
    pub const INVALID: ArchetypeRow = ArchetypeRow(u32::MAX);

    #[inline]
    pub const fn new(index: usize) -> Self {
        Self(index as u32)
    }

    #[inline]
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

/// 用于表示在 [`World`] 中唯一的 [`Archetype`] 标识
///
/// `Archetype` id 只对 对应的 `World` 有效，且不是全局唯一的
///
/// 唯一的例外是 [`EMPTY`](ArchetypeId::EMPTY)，它在所有 `World` 中都是相同的id,\
/// 表示没有任何Component的 [`Archetype`]
///
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, PartialOrd, Ord)]
// #safety : 由于对 EntityLocation 的安全要求，必须是 repr(transparent)
#[repr(transparent)]
pub struct ArchetypeId(u32);

impl ArchetypeId {
    /// 没有任何Component的 [`Archetype`] 的 id
    pub const EMPTY: ArchetypeId = ArchetypeId(0);
    /// 一个无效的id
    /// # Safety:
    /// - This must always have an all-1s bit pattern to ensure soundness in fast entity id space allocation.\
    ///   为了确保在快速实体 ID 空间分配中的健全性，这必须始终具有全1位模式
    pub const INVALID: ArchetypeId = ArchetypeId(u32::MAX);

    #[inline]
    pub const fn new(index: usize) -> Self {
        ArchetypeId(index as u32)
    }

    #[inline]
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

/// 一种Entity所有组件的类型集合
/// Archetype 表示一种组件组合
#[derive(Debug)]
struct Archetype {
    archetype_id: ArchetypeId,
    /// 存储 组件类型的元数据
    types: Vec<TypeInfo>,
    /// 存储组件的id
    type_ids: Box<[ComponentId]>,
    /// index 将组件类型 ID 映射到 types 中的索引
    index: HashMap<ComponentId, usize>,
    /// 表示实体的数量
    len: u32,
    /// 存储 [`Archetype`] 中的所有 Entity id
    entities: Box<[EntityId]>,
    // data: Box<[Data]>,
}

#[derive(Debug)]
struct Archetypes {
    archetypes: Vec<Archetype>,
    archetype_component_count: u32,
}

/// 存储类型的元信息
#[derive(Debug)]
struct TypeInfo {
    id: TypeId,
    /// 类型的内存布局信息，包括大小（size）和对齐（alignment）
    layout: Layout,
    /// 用于正确地销毁组件，释放资源
    drop: unsafe fn(*mut u8),
    #[cfg(debug_assertions)]
    type_name: &'static str,
}
impl TypeInfo {
    pub fn of<T: 'static>() -> Self {
        unsafe fn drop_ptr<T>(x: *mut u8) {
            x.cast::<T>().drop_in_place()
        }

        Self {
            id: TypeId::of::<T>(),
            layout: Layout::new::<T>(),
            drop: drop_ptr::<T>,
            #[cfg(debug_assertions)]
            type_name: core::any::type_name::<T>(),
        }
    }
}

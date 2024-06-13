use std::{alloc::Layout, any::TypeId, collections::HashMap};

use crate::component::ComponentId;

use super::EntityId;


type ArchetypeId = u32;

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
    index: HashMap<ComponentId,usize>,
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

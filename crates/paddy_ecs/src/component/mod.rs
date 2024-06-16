use std::{alloc::Layout, any::{Any, TypeId}, borrow::Cow, collections::HashMap, ptr::NonNull};



pub(crate) type ComponentId = TypeId;

/// 组件必须实现的trait
/// #todo
pub trait Component : Any + Send + Sync + 'static{
    
}


#[derive(Debug)]
pub struct Components {
    components: Vec<ComponentInfo>,
    indices: HashMap<TypeId,ComponentId>,
    // resource_indices: HashMap<TypeId,ComponentId>,
}


#[derive(Debug, Clone)]
pub struct ComponentInfo {
    id: ComponentId,
    // descriptor: ComponentDescriptor,
    // hooks: ComponentHooks,
}

pub struct ComponentDescriptor {
    name: Cow<'static, str>,
    type_id: Option<TypeId>,
    layout: Layout,
    drop: Option<unsafe fn(NonNull<u8>)>,
}




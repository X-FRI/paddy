use std::any::{Any, TypeId};



pub(crate) type ComponentId = TypeId;

/// 组件必须实现的trait
/// #todo
pub trait Component : Any + Send + Sync + 'static{
    
}






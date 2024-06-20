use crate::entity::table::Tables;

pub(crate) mod blob_vec;

/// 用于 [`World`](crate::world::World) 的原始数据存储
#[derive(Debug)]
pub struct Storages {
    pub tables: Tables,
}

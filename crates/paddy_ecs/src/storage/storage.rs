use super::{sparse_set::SparseSets, table::Tables};

/// 用于 [`World`](crate::world::World) 的原始数据存储
#[derive(Debug)]
pub struct Storages {
    pub tables: Tables,
    pub sparse_sets: SparseSets,
}

#[derive(Debug, Copy, Clone, Default, Eq, PartialEq)]
pub enum StorageType {
    #[default]
    Table,
    SparseSet,
}
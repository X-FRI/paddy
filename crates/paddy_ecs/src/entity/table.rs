use std::{alloc::Layout, collections::HashMap, ops::{Index, IndexMut}, ptr::NonNull};

use crate::{component::ComponentId, storage::blob_vec::BlobVec};

use super::Entity;

struct TableId(u32);

impl TableId {
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
}

/// Table中的 第几行
struct TableRow(u32);

impl TableRow {
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
/// 是一组相同组件类型的集合
struct Column {
    data: BlobVec,
}

impl Column {
    
    #[inline]
    pub fn item_layout(&self) -> Layout {
        self.data.layout()
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
/// #plan : 优化性能,Table存储改为密集性存储(核心是修改[`Column`])
struct Table {
    columns: HashMap<ComponentId, Column>,
    entities: Vec<Entity>,
}

/// Table 是没必要摧毁的,分配id后就永远是这个id
struct Tables {
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



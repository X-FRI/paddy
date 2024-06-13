use std::collections::HashMap;

use crate::component::ComponentId;

use super::Entity;




struct TableId(u32);

/// Table中的 第几行
struct TableRow(u32);


/// Table的一列\
/// 是一组相同组件类型的集合
struct Column {

}

/// Table 中保存 Entity的Archetype数据\
/// 每一个 Table 对应着一个特定的组件组合(Archetype)
///
/// ```
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
    columns:HashMap<ComponentId,Column>,
    entities: Vec<Entity>,
}



struct Tables {
    tables: Vec<Table>,
    table_ids: HashMap<Box<[ComponentId]>, TableId>,
}

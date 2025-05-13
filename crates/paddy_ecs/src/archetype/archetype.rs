use std::ops::{Index, IndexMut, RangeFrom};

use super::Edges;
use crate::{
    component::{ComponentId, Components},
    entity::{Entity, EntityLocation},
    storage::{
        sparse_set::{ImmutableSparseSet, SparseSet},
        table::{TableId, TableRow},
        StorageType,
    },
};

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

/// 在 [`World`] 内，用于唯一标识 [`Archetype`] 中 [`Component`] 的不透明联合 ID。
///
/// 一个组件可以存在于多个 archetype 中，但每个 archetype 中的每个组件都有自己唯一的 `ArchetypeComponentId`。
/// 系统调度器利用这一点来并行运行多个本来会冲突的系统。例如，`Query<&mut A, With<B>>` 和 `Query<&mut A, Without<B>>`
/// 可以并行运行，因为两者的 `ArchetypeComponentId` 集合是不相交的，尽管两个查询中的 `&mut A` 指向相同的 [`ComponentId`]。
///
/// 在 SQL 术语中，这些 ID 是在 archetypes 和组件之间的[多对多关系]上的复合键。
/// 每种组件类型只有一个 [`ComponentId`]，但可能有多个 [`ArchetypeComponentId`]，每个组件在所在的每个 archetype 中都有一个。
/// 同样，每个 archetype 只有一个 [`ArchetypeId`]，但可能有多个 [`ArchetypeComponentId`]，每个属于该 archetype 的组件都有一个。
///
/// 每个 [`Resource`] 也被分配了一个这样的 ID。由于资源不属于任何特定的 archetype，资源的 ID 独立标识了它。
///
/// 这些 ID 仅在给定的 World 内有效，并且不是全局唯一的。
/// 试图在其来源世界之外使用 ID 将不会指向相同的 archetype 或相同的组件。
///
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub struct ArchetypeComponentId(usize);

impl ArchetypeComponentId {
    pub(crate) fn new(index: usize) -> Self {
        Self(index)
    }

    pub(crate) fn index(&self) -> usize {
        self.0
    }
}

/// 在一个[`Archetype`]中 关于[`Entity`]的元数据
#[derive(Debug)]
pub(crate) struct ArchetypeEntity {
    entity: Entity,
    table_row: TableRow,
}

impl ArchetypeEntity {
    /// Entity 的 id
    #[inline]
    pub const fn id(&self) -> Entity {
        self.entity
    }

    /// [`Table`] 中存储 当前Entity 的行
    #[inline]
    pub const fn table_row(&self) -> TableRow {
        self.table_row
    }
}

/// 从 [`Archetype`] 中移除 [`Entity`] 的内部元数据
pub(crate) struct ArchetypeSwapRemoveResult {
    /// 如果 [`Entity`] 不是 [`Archetype`] 中的最后一个，它会被通过与最后一个实体交换来移除,
    /// 在这种情况下，这个字段包含被交换的实体(不是被移除的实体)
    pub(crate) swapped_entity: Option<Entity>,
    /// 被移除实体的组件在 [`Table`] 中存储的位置 [`TableRow`]
    pub(crate) table_row: TableRow,
}

/// 给定 [`Archetype`] 中 [`Component`] 的 内部元数据
#[derive(Debug)]
pub(crate) struct ArchetypeComponentInfo {
    storage_type: StorageType,
    archetype_component_id: ArchetypeComponentId,
}

/// `Archetype` 中的 `Component` 集合
#[derive(Debug, Hash, PartialEq, Eq)]
struct ArchetypeComponents {
    table_components: Box<[ComponentId]>,
    sparse_set_components: Box<[ComponentId]>,
}

/// Archetype 表示一种组件组合
///
/// 从Entity中移除或添加Component,只需要切换Archetype即可
#[derive(Debug)]
pub(crate) struct Archetype {
    id: ArchetypeId,
    /// Archetype 对应的 Table
    table_id: TableId,
    edges: Edges,
    /// 下标是 ArchetypeRow
    entities: Vec<ArchetypeEntity>,
    /// 一旦Archetype被构造后,这个字段就不可变
    components: ImmutableSparseSet<ComponentId, ArchetypeComponentInfo>,
}

impl Archetype {
    ///
    pub(crate) fn new(
        _components: &Components,
        id: ArchetypeId,
        table_id: TableId,
        table_components: impl Iterator<Item = (ComponentId, ArchetypeComponentId)>,
        sparse_set_components: impl Iterator<
            Item = (ComponentId, ArchetypeComponentId),
        >,
    ) -> Self {
        let (min_table, _) = table_components.size_hint();
        let (min_sparse, _) = sparse_set_components.size_hint();
        // let mut flags = ArchetypeFlags::empty();
        let mut archetype_components =
            SparseSet::with_capacity(min_table + min_sparse);
        for (component_id, archetype_component_id) in table_components {
            // SAFETY: We are creating an archetype that includes this component so it must exist
            // let info = unsafe { components.get_info_unchecked(component_id) };
            // info.update_archetype_flags(&mut flags);
            archetype_components.insert(
                component_id,
                ArchetypeComponentInfo {
                    storage_type: StorageType::Table,
                    archetype_component_id,
                },
            );
        }

        for (component_id, archetype_component_id) in sparse_set_components {
            // SAFETY: We are creating an archetype that includes this component so it must exist
            // let info = unsafe { components.get_info_unchecked(component_id) };
            // info.update_archetype_flags(&mut flags);
            archetype_components.insert(
                component_id,
                ArchetypeComponentInfo {
                    storage_type: StorageType::SparseSet,
                    archetype_component_id,
                },
            );
        }
        Self {
            id,
            table_id,
            entities: Vec::new(),
            components: archetype_components.into_immutable(),
            edges: Default::default(),
            // flags,
        }
    }

    /// 获取 `archetype` 的 ID
    #[inline]
    pub fn id(&self) -> ArchetypeId {
        self.id
    }
    /// 获取 `archetype` 的 [`TableId`]
    #[inline]
    pub fn table_id(&self) -> TableId {
        self.table_id
    }
    /// 获取此 `archetype` 中包含的所有Entity
    #[inline]
    pub fn entities(&self) -> &[ArchetypeEntity] {
        &self.entities
    }
    /// 获取 属于当前`archetype` 的Entity数量
    #[inline]
    pub fn len(&self) -> usize {
        self.entities.len()
    }

    /// 检查 `archetype` 是否包含Entity,一个Entity都没有,则返回true
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }

    /// Fetches a immutable reference to the archetype's [`Edges`], a cache of
    /// archetypal relationships.\
    /// 获取 `archetype` 的 [`Edges`] 的不可变引用，它是 `archetypal relationships` 的缓存
    #[inline]
    pub fn edges(&self) -> &Edges {
        &self.edges
    }

    /// Fetches a mutable reference to the archetype's [`Edges`], a cache of
    /// archetypal relationships.\
    /// 获取 `archetype` 的 [`Edges`] 的可变引用，它是 `archetypal relationships` 的缓存
    #[inline]
    pub(crate) fn edges_mut(&mut self) -> &mut Edges {
        &mut self.edges
    }

    /// 检查 `archetype` 是否包含特定的组件
    ///
    /// 此操作时间复杂度为 `O(1)`
    #[inline]
    pub fn contains(&self, component_id: ComponentId) -> bool {
        self.components.contains(component_id)
    }

    /// 获取 `archetype` 中所有组件的迭代器。
    ///
    /// 所有的 ID 都是唯一的
    #[inline]
    pub fn components(&self) -> impl Iterator<Item = ComponentId> + '_ {
        self.components.indices()
    }

    /// 返回 `archetype` 中组件的总数量
    #[inline]
    pub fn component_count(&self) -> usize {
        self.components.len()
    }

    /// 通过[`ComponentId`] 获取 `archetype` 中某个组件的[`StorageType`]
    ///
    /// 如果该组件不是 `archetype` 的一部分，返回 `None`
    ///
    /// 此操作时间复杂度为 `O(1)`
    #[inline]
    pub fn get_storage_type(
        &self,
        component_id: ComponentId,
    ) -> Option<StorageType> {
        self.components
            .get(component_id)
            .map(|info| info.storage_type)
    }

    /// 获取 `archetype` 中某个组件的对应 [`ArchetypeComponentId`]
    ///
    /// 如果该组件不是 `archetype` 的一部分，返回 `None`
    ///
    /// 此操作时间复杂度为 `O(1)`
    #[inline]
    pub fn get_archetype_component_id(
        &self,
        component_id: ComponentId,
    ) -> Option<ArchetypeComponentId> {
        self.components
            .get(component_id)
            .map(|info| info.archetype_component_id)
    }

    /// 获取所有存储在 [`Table`] 中的Component 的 ComponentId的迭代器
    ///
    /// 所有的 ID 都是唯一的
    #[inline]
    pub fn table_components(&self) -> impl Iterator<Item = ComponentId> + '_ {
        self.components
            .iter()
            .filter(|(_, component)| {
                component.storage_type == StorageType::Table
            })
            .map(|(id, _)| *id)
    }

    /// 获取 `row` 处实体的组件存储在 [`Table`] 中的行
    ///
    /// 可以从 [`EntityLocation::archetype_row`] 中获取实体的 `archetype` 行，
    /// 该行可以从 [`Entities::get`] 中检索到
    ///
    /// # Panic
    /// 如果 `index >= self.len()`，此函数会导致 panic
    #[inline]
    pub fn entity_table_row(&self, row: ArchetypeRow) -> TableRow {
        self.entities[row.index()].table_row
    }

    /// 修改 在`row`处的实体的组件 的 table_row
    ///
    /// # Panics
    /// 如果 `index >= self.len()`，此函数会导致 panic
    #[inline]
    pub(crate) fn set_entity_table_row(
        &mut self,
        row: ArchetypeRow,
        table_row: TableRow,
    ) {
        self.entities[row.index()].table_row = table_row;
    }

    /// 给 `archetype` 分配一个Entity
    ///
    /// # Safety
    /// 有效的组件值必须立即写入相关的Storage
    /// `table_row` 必须有效 且对应正确的Entity
    #[inline]
    pub(crate) unsafe fn allocate(
        &mut self,
        entity: Entity,
        table_row: TableRow,
    ) -> EntityLocation {
        let archetype_row = ArchetypeRow::new(self.entities.len());
        self.entities.push(ArchetypeEntity { entity, table_row });

        EntityLocation {
            archetype_id: self.id,
            archetype_row,
            table_id: self.table_id,
            table_row,
        }
    }

    #[inline]
    pub(crate) fn reserve(&mut self, additional: usize) {
        self.entities.reserve(additional);
    }

    /// 通过交换移除 `index` 处的Entity
    ///
    /// 返回 被替换的Entity(并非是被移除的) 和 被移除Entity的TableRow
    ///
    /// # Panic
    /// 如果 `index >= self.len()`，此函数会导致 panic
    #[inline]
    pub(crate) fn swap_remove(
        &mut self,
        row: ArchetypeRow,
    ) -> ArchetypeSwapRemoveResult {
        let is_last = row.index() == self.entities.len() - 1;
        let entity = self.entities.swap_remove(row.index());
        ArchetypeSwapRemoveResult {
            swapped_entity: if is_last {
                None
            } else {
                Some(self.entities[row.index()].entity)
            },
            table_row: entity.table_row,
        }
    }

    /// 获取所有存储在 [`ComponentSparseSet`] 中的组件的迭代器,
    /// 即 稀疏存储的ComponentId
    ///
    /// 所有的 ID 都是唯一的
    #[inline]
    pub fn sparse_set_components(
        &self,
    ) -> impl Iterator<Item = ComponentId> + '_ {
        self.components
            .iter()
            .filter(|(_, component)| {
                component.storage_type == StorageType::SparseSet
            })
            .map(|(id, _)| *id)
    }
    /// 清除 `archetype` 中的所有实体, 但不影响容量
    pub(crate) fn clear_entities(&mut self) {
        self.entities.clear();
    }
}

#[derive(Debug, Default)]
pub(crate) struct Archetypes {
    /// 下标是ArchetypeId
    archetypes: Vec<Archetype>,
    /// 并非是Component数量,而是 所有Archetype中Component(不去重)的数量
    ///
    /// 为了分配 [`ArchetypeComponentId`]
    archetype_component_count: usize,
    by_components: paddy_utils::hash::HashMap<ArchetypeComponents, ArchetypeId>,
}

impl Archetypes {
    pub(crate) fn new() -> Self {
        let mut archetypes = Archetypes {
            archetypes: Vec::new(),
            by_components: Default::default(),
            archetype_component_count: 0,
        };
        // SAFETY: Empty archetype has no components
        // pull 一个 ArchetypeId=0 的 Archetype, 即这个Archetype表示没有任何Component的Archetype
        unsafe {
            archetypes.get_id_or_insert(
                &Components::default(),
                TableId::empty(),
                Vec::new(),
                Vec::new(),
            );
        }
        archetypes
    }

    /// 返回 当前最大的[`ArchetypeId`] (这个id还未被分配)
    #[inline]
    pub fn generation(&self) -> ArchetypeGeneration {
        let id = ArchetypeId::new(self.archetypes.len());
        ArchetypeGeneration(id)
    }
    /// 获取World中的 [`Archetype`] 总数
    #[inline]
    #[allow(clippy::len_without_is_empty)] // 这个 vec 永远不会为空
    pub fn len(&self) -> usize {
        self.archetypes.len()
    }

    /// 获取没有任何组件的 `archetype` 的不可变引用
    ///
    /// `archetypes.get(ArchetypeId::EMPTY).unwrap()` 的简写
    #[inline]
    pub fn empty(&self) -> &Archetype {
        // SAFETY: empty archetype always exists
        unsafe { self.archetypes.get_unchecked(ArchetypeId::EMPTY.index()) }
    }
    /// 获取没有任何组件的 `archetype` 的可变引用
    #[inline]
    pub(crate) fn empty_mut(&mut self) -> &mut Archetype {
        // SAFETY: empty archetype always exists
        unsafe {
            self.archetypes
                .get_unchecked_mut(ArchetypeId::EMPTY.index())
        }
    }

    /// 生成并存储一个新的 [`ArchetypeComponentId`]
    ///
    /// 这只是简单地增加计数器并返回新值
    ///
    /// # Panic
    ///
    /// 如果 `archetype component id` 溢出，将导致 panic
    pub(crate) fn new_archetype_component_id(
        &mut self,
    ) -> ArchetypeComponentId {
        let id = ArchetypeComponentId(self.archetype_component_count);
        self.archetype_component_count = self
            .archetype_component_count
            .checked_add(1)
            .expect("archetype_component_count overflow");
        id
    }
    /// 使用其 ID 获取 [`Archetype`] 的不可变引用
    ///
    /// 如果没有对应的 `archetype` 存在，则返回 `None`
    #[inline]
    pub fn get(&self, id: ArchetypeId) -> Option<&Archetype> {
        self.archetypes.get(id.index())
    }
    /// # Panic
    ///
    /// 如果 `a` 和 `b` 相等，将导致 panic
    #[inline]
    pub(crate) fn get_2_mut(
        &mut self,
        a: ArchetypeId,
        b: ArchetypeId,
    ) -> (&mut Archetype, &mut Archetype) {
        if a.index() > b.index() {
            let (b_slice, a_slice) = self.archetypes.split_at_mut(a.index());
            (&mut a_slice[0], &mut b_slice[b.index()])
        } else {
            let (a_slice, b_slice) = self.archetypes.split_at_mut(b.index());
            (&mut a_slice[a.index()], &mut b_slice[0])
        }
    }

    /// 获取匹配给定输入的 [`ArchetypeId`]，如果不存在则插入一个新的
    ///
    /// `table_components` 和 `sparse_set_components` 必须是已排序的
    ///
    /// # Safety
    /// - [`TableId`] 必须存在于 `tables` 中
    /// - `table_components` 和 `sparse_set_components`的ComponentId 必须存在于 `components` 中
    pub(crate) unsafe fn get_id_or_insert(
        &mut self,
        components: &Components,
        table_id: TableId,
        table_components: Vec<ComponentId>,
        sparse_set_components: Vec<ComponentId>,
    ) -> ArchetypeId {
        let archetype_identity = ArchetypeComponents {
            sparse_set_components: sparse_set_components
                .clone()
                .into_boxed_slice(),
            table_components: table_components.clone().into_boxed_slice(),
        };

        let archetypes = &mut self.archetypes;
        let archetype_component_count = &mut self.archetype_component_count;
        *self.by_components.entry(archetype_identity).or_insert_with(
            move || {
                let id = ArchetypeId::new(archetypes.len());
                let table_start = *archetype_component_count;
                *archetype_component_count += table_components.len();
                let table_archetype_components = (table_start
                    ..*archetype_component_count)
                    .map(ArchetypeComponentId);
                let sparse_start = *archetype_component_count;
                *archetype_component_count += sparse_set_components.len();
                let sparse_set_archetype_components = (sparse_start
                    ..*archetype_component_count)
                    .map(ArchetypeComponentId);
                archetypes.push(Archetype::new(
                    components,
                    id,
                    table_id,
                    table_components
                        .into_iter()
                        .zip(table_archetype_components),
                    sparse_set_components
                        .into_iter()
                        .zip(sparse_set_archetype_components),
                ));
                id
            },
        )
    }

    /// 返回一个只读迭代器，遍历所有的 `archetypes`
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = &Archetype> {
        self.archetypes.iter()
    }

    /// 返回存储在 `archetypes` 中的组件数量
    /// 
    /// 请注意，如果某个组件 `T` 存储在多个 `archetypes` 中，它会被计数多次
    #[inline]
    pub fn archetype_components_len(&self) -> usize {
        self.archetype_component_count
    }

    /// 清除所有 `archetypes` 中的所有实体, 但不影响容量
    pub(crate) fn clear_entities(&mut self) {
        for archetype in &mut self.archetypes {
            archetype.clear_entities();
        }
    }
    
}

impl Index<RangeFrom<ArchetypeGeneration>> for Archetypes {
    type Output = [Archetype];

    #[inline]
    fn index(&self, index: RangeFrom<ArchetypeGeneration>) -> &Self::Output {
        &self.archetypes[index.start.0.index()..]
    }
}
impl Index<ArchetypeId> for Archetypes {
    type Output = Archetype;

    #[inline]
    fn index(&self, index: ArchetypeId) -> &Self::Output {
        &self.archetypes[index.index()]
    }
}

impl IndexMut<ArchetypeId> for Archetypes {
    #[inline]
    fn index_mut(&mut self, index: ArchetypeId) -> &mut Self::Output {
        &mut self.archetypes[index.index()]
    }
}

/// [`Archetypes`] 集合中的下一个 [`ArchetypeId`]
/// 
/// This is used in archetype update methods to limit archetype updates to the
/// ones added since the last time the method ran.\
/// 这用于 `archetype` 更新方法，以限制 `archetype` 更新仅限于自上次方法运行以来添加的 `archetype`
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct ArchetypeGeneration(ArchetypeId);

impl ArchetypeGeneration {
    /// @return ArchetypeGeneration(ArchetypeId(0))
    #[inline]
    pub const fn initial() -> Self {
        ArchetypeGeneration(ArchetypeId::EMPTY)
    }
}

use std::{
    alloc::Layout,
    any::TypeId,
    collections::HashMap,
    ops::{Index, IndexMut, RangeFrom},
};

use crate::{
    component::{ComponentId, Components},
    storage::{sparse_set::{ImmutableSparseSet, SparseSet}, StorageType},
};

use crate::{
    entity::{Entity, EntityId, EntityLocation},
    storage::table::{TableId, TableRow},
};

use super::Edges;

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
    entities: Vec<ArchetypeEntity>,
    /// 一旦Archetype被构造后,这个字段就不可变
    components: ImmutableSparseSet<ComponentId, ArchetypeComponentInfo>,
}

impl Archetype {
    pub(crate) fn new(
        components: &Components,
        id: ArchetypeId,
        table_id: TableId,
        table_components: impl Iterator<Item = (ComponentId, ArchetypeComponentId)>,
        sparse_set_components: impl Iterator<Item = (ComponentId, ArchetypeComponentId)>,
    ) -> Self {
        let (min_table, _) = table_components.size_hint();
        let (min_sparse, _) = sparse_set_components.size_hint();
        // let mut flags = ArchetypeFlags::empty();
        let mut archetype_components = 
            SparseSet::with_capacity(min_table + min_sparse);
        for (component_id, archetype_component_id) in table_components {
            // SAFETY: We are creating an archetype that includes this component so it must exist
            let info = unsafe { components.get_info_unchecked(component_id) };
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
            let info = unsafe { components.get_info_unchecked(component_id) };
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

    /// Fetches the ID for the archetype.
    #[inline]
    pub fn id(&self) -> ArchetypeId {
        self.id
    }
    /// Fetches the archetype's [`Table`] ID.
    ///
    /// [`Table`]: crate::storage::Table
    #[inline]
    pub fn table_id(&self) -> TableId {
        self.table_id
    }
    /// Fetches the entities contained in this archetype.
    #[inline]
    pub fn entities(&self) -> &[ArchetypeEntity] {
        &self.entities
    }
    /// Gets the total number of entities that belong to the archetype.
    #[inline]
    pub fn len(&self) -> usize {
        self.entities.len()
    }

    /// Checks if the archetype has any entities.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }

    /// Fetches a immutable reference to the archetype's [`Edges`], a cache of
    /// archetypal relationships.
    #[inline]
    pub fn edges(&self) -> &Edges {
        &self.edges
    }

    /// Fetches a mutable reference to the archetype's [`Edges`], a cache of
    /// archetypal relationships.
    #[inline]
    pub(crate) fn edges_mut(&mut self) -> &mut Edges {
        &mut self.edges
    }

    /// Checks if the archetype contains a specific component. This runs in `O(1)` time.
    #[inline]
    pub fn contains(&self, component_id: ComponentId) -> bool {
        self.components.contains(component_id)
    }

    /// Gets an iterator of all of the components in the archetype.
    ///
    /// All of the IDs are unique.
    #[inline]
    pub fn components(&self) -> impl Iterator<Item = ComponentId> + '_ {
        self.components.indices()
    }

    /// Returns the total number of components in the archetype
    #[inline]
    pub fn component_count(&self) -> usize {
        self.components.len()
    }

    /// Gets an iterator of all of the components stored in [`Table`]s.
    ///
    /// All of the IDs are unique.
    ///
    /// [`Table`]: crate::storage::Table
    #[inline]
    pub fn table_components(&self) -> impl Iterator<Item = ComponentId> + '_ {
        self.components.iter().map(|(id, _)| *id)
    }

    /// Updates if the components for the entity at `index` can be found
    /// in the corresponding table.
    ///
    /// # Panics
    /// This function will panic if `index >= self.len()`.
    #[inline]
    pub(crate) fn set_entity_table_row(
        &mut self,
        row: ArchetypeRow,
        table_row: TableRow,
    ) {
        self.entities[row.index()].table_row = table_row;
    }
    /// Allocates an entity to the archetype.
    ///
    /// # Safety
    /// valid component values must be immediately written to the relevant storages
    /// `table_row` must be valid
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

    /// Gets an iterator of all of the components stored in [`ComponentSparseSet`]s.
    ///
    /// All of the IDs are unique.
    ///
    /// [`ComponentSparseSet`]: crate::storage::ComponentSparseSet
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
}

#[derive(Debug)]
pub(crate) struct Archetypes {
    archetypes: Vec<Archetype>,
    archetype_component_count: usize,
    by_components: HashMap<ArchetypeComponents, ArchetypeId>,
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
            // archetypes.get_id_or_insert(
            //     &Components::default(),
            //     TableId::empty(),
            //     Vec::new(),
            //     Vec::new(),
            // );
        }
        archetypes
    }

    /// Fetches an immutable reference to an [`Archetype`] using its
    /// ID. Returns `None` if no corresponding archetype exists.
    #[inline]
    pub fn get(&self, id: ArchetypeId) -> Option<&Archetype> {
        self.archetypes.get(id.index())
    }

    /// Returns the "generation", a handle to the current highest archetype ID.
    ///
    /// This can be used with the `Index` [`Archetypes`] implementation to
    /// iterate over newly introduced [`Archetype`]s since the last time this
    /// function was called.
    #[inline]
    pub fn generation(&self) -> ArchetypeGeneration {
        let id = ArchetypeId::new(self.archetypes.len());
        ArchetypeGeneration(id)
    }

    /// Fetches an mutable reference to the archetype without any components.
    #[inline]
    pub(crate) fn empty_mut(&mut self) -> &mut Archetype {
        // SAFETY: empty archetype always exists
        unsafe {
            self.archetypes
                .get_unchecked_mut(ArchetypeId::EMPTY.index())
        }
    }

    /// Gets the archetype id matching the given inputs or inserts a new one if it doesn't exist.
    /// `table_components` and `sparse_set_components` must be sorted
    ///
    /// # Safety
    /// [`TableId`] must exist in tables
    /// `table_components` and `sparse_set_components` must exist in `components`
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

/// The next [`ArchetypeId`] in an [`Archetypes`] collection.
///
/// This is used in archetype update methods to limit archetype updates to the
/// ones added since the last time the method ran.
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct ArchetypeGeneration(ArchetypeId);

impl ArchetypeGeneration {
    /// The first archetype.
    #[inline]
    pub const fn initial() -> Self {
        ArchetypeGeneration(ArchetypeId::EMPTY)
    }
}

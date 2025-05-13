use super::ArchetypeId;
use crate::{bundle::BundleId, storage::sparse_set::SparseArray};

/// Archetypes and bundles form a graph. Adding or removing a bundle moves
/// an [`Entity`] to a new [`Archetype`].
///
/// [`Edges`] caches the results of these moves. Each archetype caches
/// the result of a structural alteration. This can be used to monitor the
/// state of the archetype graph.
///
/// Note: This type only contains edges the [`World`] has already traversed.
/// If any of functions return `None`, it doesn't mean there is guaranteed
/// not to be a result of adding or removing that bundle, but rather that
/// operation that has moved an entity along that edge has not been performed
/// yet.\

/// 原型和组件包（bundle）形成一个图结构.
/// 添加或移除一个组件包会将一个 [`Entity`] 移动到一个新的 [`Archetype`]
///
/// [`Edges`] 缓存了这些移动的结果.
/// 每个原型缓存了结构性更改的结果.
/// 这可以用来监控原型图的状态.
///
/// 注意：该类型仅包含 [`World`] 已经遍历过的edges.
/// 如果任何函数返回 `None`，这并不意味着添加或移除该组件包没有结果，
/// 而是表示沿该edges移动实体的操作尚未执行.
#[derive(Debug, Default)]
pub struct Edges {
    /// 缓存 当Bundle中的组件 添加到 当前原型 时,会变成 哪个目标原型
    ///
    /// Added 表示哪些组件是新增的\
    /// Mutated 表示那些组件是被修改的(即 重复的(Bundle的组件和原型的组件重复),将会覆盖原值)
    add_bundle: SparseArray<BundleId, AddBundle>,
    /// 缓存 当前原型 移除Bundle中的组件后 会变成 哪个目标原型
    ///
    /// None 意味着 当前原型 无法移除Bundle中的组件 (可能是 BundleId中的组件 不存在于 当前原型)
    /// 
    /// ? 好像不对啊, 我看源码 似乎 take_bundle 具有 None 的含义 
    remove_bundle: SparseArray<BundleId, Option<ArchetypeId>>,
    /// ? 似乎 remove_bundle 是 不关心Bundle中的组件 是否 都存在于 当前原型
    /// ? 而 take_bundle 要求 Bundle中的组件 必须都 存在与 当前原型 ,否则失败,即为 None
    take_bundle: SparseArray<BundleId, Option<ArchetypeId>>,
}

impl Edges {
    /// 通过[`BundleId`]检查是否存在 add_bundle缓存, None没有缓存
    #[inline]
    pub fn get_add_bundle(&self, bundle_id: BundleId) -> Option<ArchetypeId> {
        self.get_add_bundle_internal(bundle_id)
            .map(|bundle| bundle.archetype_id)
    }

    /// `get_add_bundle` 的内部版本，用于获取完整的 `AddBundle`
    #[inline]
    pub(crate) fn get_add_bundle_internal(
        &self,
        bundle_id: BundleId,
    ) -> Option<&AddBundle> {
        self.add_bundle.get(bundle_id)
    }

    /// 添加一个 add_bundle 缓存
    #[inline]
    pub(crate) fn insert_add_bundle(
        &mut self,
        bundle_id: BundleId,
        archetype_id: ArchetypeId,
        bundle_status: Vec<ComponentStatus>,
    ) {
        self.add_bundle.insert(
            bundle_id,
            AddBundle {
                archetype_id,
                bundle_status,
            },
        );
    }

    /// 通过[`BundleId`]检查是否存在 remove_bundle缓存,\
    /// None没有缓存\
    /// Some(None)意味着 当前原型 无法移除Bundle中的组件
    #[inline]
    pub fn get_remove_bundle(
        &self,
        bundle_id: BundleId,
    ) -> Option<Option<ArchetypeId>> {
        self.remove_bundle.get(bundle_id).cloned()
    }

    /// 添加一个 remove_bundle 缓存
    #[inline]
    pub(crate) fn insert_remove_bundle(
        &mut self,
        bundle_id: BundleId,
        archetype_id: Option<ArchetypeId>,
    ) {
        self.remove_bundle.insert(bundle_id, archetype_id);
    }

    /// 通过[`BundleId`]检查是否存在 take_bundle缓存,\
    /// None没有缓存\
    /// Some(None)意味着 当前原型 无法移除Bundle中的组件
    #[inline]
    pub fn get_take_bundle(
        &self,
        bundle_id: BundleId,
    ) -> Option<Option<ArchetypeId>> {
        self.take_bundle.get(bundle_id).cloned()
    }

    /// 添加一个 take_bundle 缓存
    #[inline]
    pub(crate) fn insert_take_bundle(
        &mut self,
        bundle_id: BundleId,
        archetype_id: Option<ArchetypeId>,
    ) {
        self.take_bundle.insert(bundle_id, archetype_id);
    }
}

/// 表示一个组件的状态：是被添加还是被修改
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) enum ComponentStatus {
    Added,
    Mutated,
}

#[derive(Debug)]
pub(crate) struct AddBundle {
    /// bundle的组件添加到 当前原型 后 变成的 目标原型id
    pub archetype_id: ArchetypeId,
    /// For each component iterated in the same order as the source [`Bundle`](crate::bundle::Bundle),
    /// indicate if the component is newly added to the target archetype or if it already existed
    pub bundle_status: Vec<ComponentStatus>,
}

/// This trait is used to report the status of [`Bundle`](crate::bundle::Bundle) components
/// being added to a given entity, relative to that entity's original archetype.
/// See [`crate::bundle::BundleInfo::write_components`] for more info.\
/// 该 trait 用于报告将 [`Bundle`](crate::bundle::Bundle) 组件添加到给定实体时的状态，
/// 相对于该实体的原始 `archetype`。
/// 有关更多信息，请参见 [`crate::bundle::BundleInfo::write_components`]。
pub(crate) trait BundleComponentStatus {
    /// 返回 给定`index` 的 Bundle的组件状态
    ///
    /// # 安全性
    /// 调用者必须确保 `index` 始终是与此 [`BundleComponentStatus`] 相关联的 `Bundle` 的有效 `index`
    unsafe fn get_status(&self, index: usize) -> ComponentStatus;
}

impl BundleComponentStatus for AddBundle {
    #[inline]
    unsafe fn get_status(&self, index: usize) -> ComponentStatus {
        // SAFETY: caller has ensured index is a valid bundle index for this bundle
        unsafe { *self.bundle_status.get_unchecked(index) }
    }
}

pub(crate) struct SpawnBundleStatus;

impl BundleComponentStatus for SpawnBundleStatus {
    #[inline]
    unsafe fn get_status(&self, _index: usize) -> ComponentStatus {
        //  在 `spawn` 调用期间添加的组件总是被视为新添加的
        ComponentStatus::Added
    }
}

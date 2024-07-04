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
///
/// 原型和组件包（bundle）形成一个图结构。添加或移除一个组件包会将一个 [`Entity`] 移动到一个新的 [`Archetype`]。
///
/// [`Edges`] 缓存了这些移动的结果。每个原型缓存了结构性更改的结果。这可以用来监控原型图的状态。
///
/// 注意：该类型仅包含 [`World`] 已经遍历过的边。如果任何函数返回 `None`，这并不意味着添加或移除该组件包没有结果，
/// 而是表示沿该边移动实体的操作尚未执行。
#[derive(Debug, Default)]
pub struct Edges {
    add_bundle: SparseArray<BundleId, AddBundle>,
    remove_bundle: SparseArray<BundleId, Option<ArchetypeId>>,
    take_bundle: SparseArray<BundleId, Option<ArchetypeId>>,
}

impl Edges {
    /// Checks the cache for the target archetype when adding a bundle to the
    /// source archetype. For more information, see [`EntityWorldMut::insert`].
    ///
    /// If this returns `None`, it means there has not been a transition from
    /// the source archetype via the provided bundle.
    ///
    /// [`EntityWorldMut::insert`]: crate::world::EntityWorldMut::insert
    #[inline]
    pub fn get_add_bundle(&self, bundle_id: BundleId) -> Option<ArchetypeId> {
        self.get_add_bundle_internal(bundle_id)
            .map(|bundle| bundle.archetype_id)
    }

    /// Internal version of `get_add_bundle` that fetches the full `AddBundle`.
    #[inline]
    pub(crate) fn get_add_bundle_internal(
        &self,
        bundle_id: BundleId,
    ) -> Option<&AddBundle> {
        self.add_bundle.get(bundle_id)
    }

    /// Caches the target archetype when adding a bundle to the source archetype.
    /// For more information, see [`EntityWorldMut::insert`].
    ///
    /// [`EntityWorldMut::insert`]: crate::world::EntityWorldMut::insert
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
}

/// 表示一个组件的状态：是被添加还是被修改
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(crate) enum ComponentStatus {
    Added,
    Mutated,
}

#[derive(Debug)]
pub(crate) struct AddBundle {
    /// The target archetype after the bundle is added to the source archetype
    pub archetype_id: ArchetypeId,
    /// For each component iterated in the same order as the source [`Bundle`](crate::bundle::Bundle),
    /// indicate if the component is newly added to the target archetype or if it already existed
    pub bundle_status: Vec<ComponentStatus>,
}

/// This trait is used to report the status of [`Bundle`](crate::bundle::Bundle) components
/// being added to a given entity, relative to that entity's original archetype.
/// See [`crate::bundle::BundleInfo::write_components`] for more info.
pub(crate) trait BundleComponentStatus {
    /// Returns the Bundle's component status for the given "bundle index"
    ///
    /// # Safety
    /// Callers must ensure that index is always a valid bundle index for the
    /// Bundle associated with this [`BundleComponentStatus`]
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
        // Components added during a spawn call are always treated as added
        ComponentStatus::Added
    }
}

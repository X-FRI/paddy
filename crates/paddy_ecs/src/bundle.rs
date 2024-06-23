use std::{
    any::TypeId,
    collections::{HashMap, HashSet},
};

use paddy_ptr::OwningPtr;

use crate::{
    component::{ComponentId, Components},
    storage::{StorageType, Storages},
};

/// `Bundle` trait 使得可以在一个实体上插入和移除 [`Component`]
///
/// 实现 `Bundle` trait 的类型被称为“bundles”
///
/// 每个 bundle 代表一组静态类型的 [`Component`]
///
/// 当前，bundle 不能包含相同的 [`Component`] ，如果不满足这个条件，将在初始化时触发 panic
///
/// ## Insertion
///
/// bundle 的主要用途是向一个Entity添加有用的组件集合
///
/// 将一个 bundle 的值添加到一个Enitty时，会将 bundle 所代表的组件集合中的组件 添加到该实体.
///
/// 这些组件的值来自于 bundle.
/// 如果实体已经包含了其中的某个组件，实体原有的组件值将被覆盖
///
/// 重要的是，bundle 只是其组成的组件集合。你 **不应** 使用 bundle 作为行为的单元.
/// 你的应用程序的行为只能用组件来考虑，因为驱动 `paddy` 应用行为的系统，是基于组件组合操作的
///
/// This rule is also important because multiple bundles may contain the same component type,
/// calculated in different ways &mdash; adding both of these bundles to one entity
/// would create incoherent behavior.\
/// 这一规则也很重要，因为多个 bundle 可能包含相同类型的组件，但这些组件的计算方式不同 —— 将这两个 bundle 都添加到一个实体上会导致行为不一致
/// This would be unexpected if bundles were treated as an abstraction boundary, as
/// the abstraction would be unmaintainable for these cases.\
/// 如果 bundle 被视为一个抽象边界，那么在这种情况下这种抽象将是不可维护的
/// For example, both `Camera3dBundle` and `Camera2dBundle` contain the `CameraRenderGraph`
/// component, but specifying different render graphs to use.
/// If the bundles were both added to the same entity, only one of these two bundles would work.\
/// 例如，`Camera3dBundle` 和 `Camera2dBundle` 都包含 `CameraRenderGraph` 组件，但它们指定使用不同的渲染图。
/// 如果将这两个 bundle 都添加到一个实体上，只有其中一个 bundle 会正常工作。
///
///
/// For this reason, there is intentionally no [`Query`] to match whether an entity
/// contains the components of a bundle.
/// Queries should instead only select the components they logically operate on.\
/// 因此，故意没有提供 [`Query`] 来检查实体是否包含某个 bundle 的组件。
/// 查询应该只选择它们逻辑上操作的组件。
///  
/// ## Removal
///
/// bundle 也可以用于从实体中移除组件
/// 
/// 从一个实体中移除一个 bundle 时，bundle 中存在的任何组件都会从实体上移除。
/// 如果实体不包含 bundle 的所有组件，那些存在的组件将被移除。
///
/// # Implementors
///
/// 每个实现了 [`Component`] 的类型也会实现 `Bundle`，因为 [`Component`] 类型可以被添加到或从实体中移除
///
/// 此外，元组（`tuple`）类型的 bundle 也是 [`Bundle`]（最多可包含 15 个 bundle）。
/// 这些 bundle 包含了“内部” bundle 的项目。这是一种方便的简写，主要用于创建实体时。
/// 
/// `unit`，也就是 `()`（即空元组），是一个包含没有组件的 [`Bundle`]。这在使用 [`World::spawn_batch`](crate::world::World::spawn_batch) 创建大量空实体时很有用
///
/// Tuple bundles can be nested, which can be used to create an anonymous bundle with more than
/// 15 items.
/// However, in most cases where this is required, the derive macro [`derive@Bundle`] should be
/// used instead.
/// The derived `Bundle` implementation contains the items of its fields, which all must
/// implement `Bundle`.
/// As explained above, this includes any [`Component`] type, and other derived bundles.\
/// 元组 bundle 可以嵌套，这可以用于创建包含超过 15 个项目的匿名 bundle。然而，在大多数需要这种情况下，应该使用派生宏 [`derive@Bundle`]。
/// 派生的 `Bundle` 实现包含其字段的项目，这些字段必须全部实现 `Bundle`。
/// 如上所述，这包括任何 [`Component`] 类型和其他派生的 bundle。
/// 
/// If you want to add `PhantomData` to your `Bundle` you have to mark it with `#[bundle(ignore)]`.
/// 如果你想在你的 `Bundle` 中添加 `PhantomData`，你必须将其标记为 `#[bundle(ignore)]`
/// 
/// # Safety
///
/// 手动实现这个 trait 是不被支持的。这意味着没有安全的方法来实现这个 trait，且你绝不应该尝试这样做。
/// 如果你希望某个类型实现 [`Bundle`]，你必须使用 [`derive@Bundle`](derive@Bundle)
/// 
// #safety :
// - [`Bundle::component_ids`] 必须返回 bundle 中每个组件类型的 [`ComponentId`]，
//   顺序必须与 [`DynamicBundle::get_components`] 被调用的顺序完全一致。
// - [`Bundle::from_components`] 必须对每个由 [`Bundle::component_ids`] 返回的 [`ComponentId`] 恰好调用一次 `func`。
// #plan : 添加多个bundle时,出现重复组件,则发生panic ,而非新值覆盖旧值
// #plan : 开发 允许相同组件的存在,可能会使用一个包装来做,类似 A<B> 包装体为A,被包装为B
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a `Bundle`",
    label = "invalid `Bundle`",
    note = "consider annotating `{Self}` with `#[derive(Component)]` or `#[derive(Bundle)]`"
)]
pub unsafe trait Bundle: DynamicBundle + Send + Sync + 'static {
    /// Gets this [`Bundle`]'s component ids, in the order of this bundle's [`Component`]s\
    /// 获取这个 [`Bundle`] 的组件 ID，按该 bundle 的 [`Component`] 顺序排列
    #[doc(hidden)]
    fn component_ids(
        components: &mut Components,
        storages: &mut Storages,
        ids: &mut impl FnMut(ComponentId),
    );

    /// Calls `func`, which should return data for each component in the bundle, in the order of
    /// this bundle's [`Component`]s\
    /// 调用 `func`，该函数应返回这个 bundle 中每个组件的数据，按该 bundle 的 [`Component`] 顺序排列
    ///
    /// # Safety
    /// Caller must return data for each component in the bundle, in the order of this bundle's
    /// [`Component`]s\
    /// 调用者必须返回这个 bundle 中每个组件的数据，按该 bundle 的 [`Component`] 顺序排列
    #[doc(hidden)]
    unsafe fn from_components<T, F>(ctx: &mut T, func: &mut F) -> Self
    where
        // Ensure that the `OwningPtr` is used correctly
        F: for<'a> FnMut(&'a mut T) -> OwningPtr<'a>,
        Self: Sized;
}

/// [`Bundle`] 中不需要在编译时静态知道组件的部分
pub trait DynamicBundle {
    // SAFETY:
    // The `StorageType` argument passed into [`Bundle::get_components`] must be correct for the
    // component being fetched.\
    // 传递给 [`Bundle::get_components`] 的 `StorageType` 参数必须对于被获取的组件是正确的
    //
    /// Calls `func` on each value, in the order of this bundle's [`Component`]s. This passes
    /// ownership of the component values to `func`.\
    /// 按这个 bundle 的 [`Component`] 顺序调用 `func` 处理每个值。这会将组件值的所有权传递给 `func`
    #[doc(hidden)]
    fn get_components(self, func: &mut impl FnMut(StorageType, OwningPtr<'_>));
}

/// 对于对应的 [`World`]，它存储了一个唯一的值，用于标识已注册的 [`Bundle`] 类型
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct BundleId(usize);

impl BundleId {
    /// #note : 这个索引在每个 `World` 中都是唯一的，不应在不同的 `World` 间重复使用
    #[inline]
    pub fn index(self) -> usize {
        self.0
    }
}
/// 存储在对应 [`World`] 中某个 [`Bundle`] 类型相关的元数据
#[derive(Debug)]
pub struct BundleInfo {
    id: BundleId,
    /// #safety : 此Vec中的每个 ID 必须 在拥有 BundleInfo 的 World 中有效，
    /// 必须已经初始化其存储（即在表中创建了列，创建了稀疏集），
    /// 并且必须与源 Bundle 类型写入其组件的顺序相同。
    component_ids: Vec<ComponentId>,
}

impl BundleInfo {
    /// 创建一个新的 [`BundleInfo`]
    ///
    /// # Safety
    ///
    /// `component_ids` 中的每个 ID 必须 在拥有 BundleInfo 的 World 中有效，
    /// 必须已经初始化其存储（即在表中创建了列，创建了稀疏集），
    /// 并且必须与源 Bundle 类型写入其组件的顺序相同。
    unsafe fn new(
        bundle_type_name: &'static str,
        components: &Components,
        component_ids: Vec<ComponentId>,
        id: BundleId,
    ) -> BundleInfo {
        let mut deduped = component_ids.clone();
        deduped.sort();
        deduped.dedup();
        //if 存在重复ComponentId
        if deduped.len() != component_ids.len() {
            // #todo : Replace with `Vec::partition_dedup` once https://github.com/rust-lang/rust/issues/54279 is stabilized
            let mut seen = HashSet::new();
            // 存储重复元素(ComponentId)
            let mut dups = Vec::new();
            for id in component_ids {
                if !seen.insert(id) {
                    dups.push(id);
                }
            }

            let names = dups
                .into_iter()
                .map(|id| {
                    // SAFETY: the caller ensures component_id is valid.
                    unsafe { components.get_info_unchecked(id).name() }
                })
                .collect::<Vec<_>>()
                .join(", ");
            // panic 输出重复Component的name
            panic!("Bundle {bundle_type_name} has duplicate components: {names}");
        }

        // SAFETY: The caller ensures that component_ids:
        // - is valid for the associated world
        // - has had its storage initialized
        // - is in the same order as the source bundle type
        BundleInfo { id, component_ids }
    }
    /// @return Bundle id
    #[inline]
    pub const fn id(&self) -> BundleId {
        self.id
    }

    /// @return 存储在此 Bundle 中所有的 ComponentId
    #[inline]
    pub fn components(&self) -> &[ComponentId] {
        &self.component_ids
    }

    /// @return 返回一个迭代器，用于遍历存储在此 Bundle 中所有的 ComponentId
    #[inline]
    pub fn iter_components(&self) -> impl Iterator<Item = ComponentId> + '_ {
        self.component_ids.iter().cloned()
    }
}

/// 存储所有 bundle 的元数据,
/// 存储每个在 对应 `world` 中的 [`Bundle`] 类型的 [`BundleInfo`]
#[derive(Debug, Default)]
pub struct Bundles {
    /// 下标为BundleId
    bundle_infos: Vec<BundleInfo>,
    /// Cache static [`BundleId`]
    bundle_ids: HashMap<TypeId, BundleId>,
    /// Cache dynamic [`BundleId`] with multiple components
    dynamic_bundle_ids: HashMap<Box<[ComponentId]>, BundleId>,
    dynamic_bundle_storages: HashMap<BundleId, Vec<StorageType>>,
    /// Cache optimized dynamic [`BundleId`] with single component
    dynamic_component_bundle_ids: HashMap<ComponentId, BundleId>,
    dynamic_component_storages: HashMap<BundleId, StorageType>,
}

impl Bundles {
    /// 
    /// @return 如果 bundle 没有在 world 中注册，则返回 `None`
    #[inline]
    pub fn get(&self, bundle_id: BundleId) -> Option<&BundleInfo> {
        self.bundle_infos.get(bundle_id.index())
    }

    /// 
    /// @return 如果 bundle 不存在于 world 中，或者 `type_id` 不对应任何 bundle 类型，则返回 `None`。
    #[inline]
    pub fn get_id(&self, type_id: TypeId) -> Option<BundleId> {
        self.bundle_ids.get(&type_id).cloned()
    }

    /// 为静态已知类型初始化一个新的 [`BundleInfo`]
    ///
    /// 还会初始化 bundle 中的所有组件
    pub(crate) fn init_info<T: Bundle>(
        &mut self,
        components: &mut Components,
        storages: &mut Storages,
    ) -> BundleId {
        let bundle_infos = &mut self.bundle_infos;
        let id = *self.bundle_ids.entry(TypeId::of::<T>()).or_insert_with(|| {
            let mut component_ids = Vec::new();
            T::component_ids(components, storages, &mut |id| component_ids.push(id));
            let id = BundleId(bundle_infos.len());
            let bundle_info =
                // SAFETY: T::component_id ensures:
                // - its info was created
                // - appropriate storage for it has been initialized.
                // - it was created in the same order as the components in T
                unsafe { BundleInfo::new(std::any::type_name::<T>(), components, component_ids, id) };
            bundle_infos.push(bundle_info);
            id
        });
        id
    }
}

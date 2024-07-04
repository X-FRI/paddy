use std::{
    alloc::Layout,
    any::{Any, TypeId},
    borrow::Cow,
    collections::HashMap,
    fmt::Debug,
    mem::needs_drop,
    ptr::NonNull,
};

use paddy_ptr::OwningPtr;

use crate::storage::{sparse_set::SparseSetIndex, StorageType, Storages};

/// 用于唯一标识 [`World`] 中某个 [`Component`] 或 [`Resource`] ,便于跟踪组件或资源
///
/// `World` 中可能还会存在其他 `ComponentId` 来跟踪那些无法表示为 Rust 类型的组件, 所以`ComponentId`不应该单纯使用[`TypeId`]
///
/// `ComponentId` 与其所属的 `World` 紧密关联,
/// 不应该使用一个 `World` 的 `ComponentId`,去访问另一个 `World` 中 `Component` 的元数据
///
#[derive(Debug, Copy, Clone, Hash, Ord, PartialOrd, Eq, PartialEq)]
pub struct ComponentId(usize);

impl ComponentId {
    /// 创建一个 [`ComponentId`].
    ///
    /// 你需要保证id在world中的唯一性
    #[inline]
    pub const fn new(id: usize) -> ComponentId {
        ComponentId(id)
    }

    /// Get component id
    #[inline]
    pub fn id(self) -> usize {
        self.0
    }
}

impl SparseSetIndex for ComponentId {
    #[inline]
    fn sparse_set_index(&self) -> usize {
        self.id()
    }

    #[inline]
    fn get_sparse_set_index(value: usize) -> Self {
        Self(value)
    }
}

/// 组件必须实现的trait
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a `Component`",
    label = "invalid `Component`",
    note = "consider annotating `{Self}` with `#[derive(Component)]`"
)]
pub trait Component: Any + Send + Sync + 'static {
    const STORAGE_TYPE: StorageType;
}

/// 在对应World中,用于管理和存储所有注册的组件类型的元信息
///
#[derive(Debug, Default)]
pub struct Components {
    /// ComponentId为下标
    components: Vec<ComponentInfo>,
    /// 用于快速通过TypeId寻找到ComponentId
    indices: HashMap<TypeId, ComponentId>,
}

impl Components {
    /// @return 注册的组件数量
    #[inline]
    pub fn len(&self) -> usize {
        self.components.len()
    }

    /// 一个组件都没有,则返回ture
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.components.len() == 0
    }

    /// 获取与给定组件相关联的元信息
    ///
    /// 如果 `id` 并非来自与 `self` 相同的World, 可能返回 `None` 或 错误值(其他World中的[`ComponentInfo`])
    #[inline]
    pub fn get_info(&self, id: ComponentId) -> Option<&ComponentInfo> {
        self.components.get(id.0)
    }

    /// 返回与给定组件相关联的名称
    ///
    /// 如果 `id` 并非来自与 `self` 相同的World, 可能返回 `None` 或 错误值(其他World中的[`ComponentInfo`]所提供的name)
    #[inline]
    pub fn get_name(&self, id: ComponentId) -> Option<&str> {
        self.get_info(id).map(|descriptor| descriptor.name())
    }

    /// 获取与给定组件相关联的元数据
    /// # Safety
    /// - `id` 必须是一个有效的 [`ComponentId`]
    ///
    #[inline]
    pub unsafe fn get_info_unchecked(&self, id: ComponentId) -> &ComponentInfo {
        debug_assert!(id.id() < self.components.len());
        // SAFETY: The caller ensures `id` is valid.
        unsafe { self.components.get_unchecked(id.0) }
    }

    /// 类型擦出后 通过 [`TypeId`] 获取[`ComponentId`]
    #[inline]
    pub fn get_id(&self, type_id: TypeId) -> Option<ComponentId> {
        self.indices.get(&type_id).copied()
    }
    #[inline]
    pub fn component_id<T: Component>(&self) -> Option<ComponentId> {
        self.get_id(TypeId::of::<T>())
    }

    /// 初始化`T`类型的组件
    ///
    /// @return 如果该类型的组件已经被初始化过，那么此方法会返回之前已经存在的 ComponentId
    #[inline]
    pub fn init_component<T: Component>(
        &mut self,
        storages: &mut Storages,
    ) -> ComponentId {
        let type_id = TypeId::of::<T>();

        let Components {
            indices,
            components,
            ..
        } = self;
        *indices.entry(type_id).or_insert_with(|| {
            let index = Components::init_component_inner(
                components,
                storages,
                ComponentDescriptor::new::<T>(),
            );
            // T::register_component_hooks(&mut components[index.index()].hooks);
            index
        })
    }

    /// 使用 `descriptor` 初始化组件
    ///
    /// ## Note
    ///
    /// 如果多次调用此方法，并且使用相同的[`ComponentDescriptor`]（`descriptor`），将为每个`descriptor`创建不同的 [`ComponentId`]。
    ///
    // #[inline]
    // pub fn init_component_with_descriptor(
    //     &mut self,
    //     descriptor: ComponentDescriptor,
    // ) -> ComponentId {
    //     Components::init_component_inner(&mut self.components, descriptor)
    // }

    #[inline]
    fn init_component_inner(
        components: &mut Vec<ComponentInfo>,
        storages: &mut Storages,
        descriptor: ComponentDescriptor,
    ) -> ComponentId {
        let component_id = ComponentId(components.len());
        let info = ComponentInfo::new(component_id, descriptor);
        if info.descriptor.storage_type == StorageType::SparseSet {
            storages.sparse_sets.get_or_insert(&info);
        }
        components.push(info);
        component_id
    }

    pub fn iter(&self) -> impl Iterator<Item = &ComponentInfo> + '_ {
        self.components.iter()
    }
}

/// 存储Component类型的信息
#[derive(Debug, Clone)]
pub struct ComponentInfo {
    id: ComponentId,
    descriptor: ComponentDescriptor,
    // hooks: ComponentHooks,
}

impl ComponentInfo {
    #[inline]
    pub fn id(&self) -> ComponentId {
        self.id
    }

    #[inline]
    pub fn name(&self) -> &str {
        &self.descriptor.name
    }
    #[inline]
    pub fn type_id(&self) -> Option<TypeId> {
        self.descriptor.type_id
    }
    #[inline]
    pub fn layout(&self) -> Layout {
        self.descriptor.layout
    }
    #[inline]
    pub fn drop(&self) -> Option<unsafe fn(OwningPtr<'_>)> {
        self.descriptor.drop
    }

    pub(crate) fn new(
        id: ComponentId,
        descriptor: ComponentDescriptor,
    ) -> Self {
        ComponentInfo { id, descriptor }
    }

    /// Returns a value indicating the storage strategy for the current component.
    #[inline]
    pub fn storage_type(&self) -> StorageType {
        self.descriptor.storage_type
    }
}

/// 用于描述组件或资源的元信息，它可能不对应 Rust 类型
#[derive(Clone)]
pub struct ComponentDescriptor {
    name: Cow<'static, str>,
    storage_type: StorageType,
    type_id: Option<TypeId>,
    layout: Layout,
    drop: Option<for<'a> unsafe fn(OwningPtr<'a>)>,
}

impl ComponentDescriptor {
    /// Returns the [`TypeId`] of the underlying component type.
    /// Returns `None` if the component does not correspond to a Rust type.
    #[inline]
    pub fn type_id(&self) -> Option<TypeId> {
        self.type_id
    }

    /// Returns the name of the current component.
    #[inline]
    pub fn name(&self) -> &str {
        self.name.as_ref()
    }

    /// # SAFETY
    ///
    /// `x` must points to a valid value of type `T`.
    unsafe fn drop_ptr<T>(x: OwningPtr<'_>) {
        // SAFETY: Contract is required to be upheld by the caller.
        unsafe {
            x.drop_as::<T>();
        }
    }
    /// Create a new `ComponentDescriptor` for the type `T`.
    pub fn new<T: Component>() -> Self {
        Self {
            name: Cow::Borrowed(std::any::type_name::<T>()),
            storage_type: T::STORAGE_TYPE,
            type_id: Some(TypeId::of::<T>()),
            layout: Layout::new::<T>(),
            drop: needs_drop::<T>().then_some(Self::drop_ptr::<T> as _),
        }
    }
}

impl std::fmt::Debug for ComponentDescriptor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ComponentDescriptor")
            .field("name", &self.name)
            .field("type_id", &self.type_id)
            .field("layout", &self.layout)
            .finish()
    }
}

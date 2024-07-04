use core::fmt;
use std::{
    error::Error,
    mem,
    num::{NonZeroU32, NonZeroU64},
    sync::atomic::{AtomicIsize, Ordering},
};

use table::{TableId, TableRow};

use crate::{
    archetype::{ArchetypeId, ArchetypeRow},
    storage::table,
    world::World,
};

pub(crate) type EntityId = u32;
/// Id 重分配后 标识于前Entity的不同
pub(crate) type EntityGeneration = NonZeroU32;
#[derive(Debug, Clone, Copy)]
pub(crate) struct Entity {
    entity_id: EntityId,
    generation: EntityGeneration,
}

impl Entity {
    /// 一个虚拟的Entity(这个Entity是无效) \
    /// 用处 :
    /// 1. 往往作为一个占位的Entity , 不参与World中
    /// 2. 错误处理和检测
    /// 3. 防止未初始化使用
    /// 4. 特定场景的标识符
    pub(crate) const PLACEHOLDER: Entity = Entity {
        generation: NonZeroU32::MIN,
        entity_id: u32::MAX,
    };

    /// Construct an [`Entity`] from a raw `index` value and a non-zero `generation` value.
    /// Ensure that the generation value is never greater than `0x7FFF_FFFF`.
    #[inline(always)]
    pub(crate) const fn from_raw_and_generation(
        index: u32,
        generation: NonZeroU32,
    ) -> Entity {
        // debug_assert!(generation.get() <= HIGH_MASK);

        Self {
            entity_id: index,
            generation,
        }
    }
    /// Creates a new entity ID with the specified `index` and a generation of 1.
    ///
    /// # Note
    ///
    /// Spawning a specific `entity` value is __rarely the right choice__. Most apps should favor
    /// [`Commands::spawn`](crate::system::Commands::spawn). This method should generally
    /// only be used for sharing entities across apps, and only when they have a scheme
    /// worked out to share an index space (which doesn't happen by default).
    ///
    /// In general, one should not try to synchronize the ECS by attempting to ensure that
    /// `Entity` lines up between instances, but instead insert a secondary identifier as
    /// a component.
    #[inline(always)]
    pub const fn from_raw(index: u32) -> Entity {
        Self::from_raw_and_generation(index, NonZeroU32::MIN)
    }

    #[inline]
    pub(crate) const fn from_bits(bits: u64) -> Option<Self> {
        Some(Self {
            generation: match NonZeroU32::new((bits >> 32) as u32) {
                Some(g) => g,
                None => return None,
            },
            entity_id: bits as u32,
        })
    }

    /// @return Entity唯一标识
    #[inline(always)]
    pub(crate) const fn to_bits(self) -> NonZeroU64 {
        unsafe {
            NonZeroU64::new_unchecked(
                (self.generation.get() as u64) << 32 | (self.entity_id as u64),
            )
        }
    }

    /// 需要注意 这个id并非唯一的, 它与上一个使用这个id的Entity(以被摧毁的) 是相同id \
    /// 但一定保证没有 2个 live Entity(活实体) 存在相同id \
    /// 只可能存在 1个 live Entity 和 多个 dead Entity 存在相同id \
    #[inline]
    pub(crate) const fn index(self) -> u32 {
        self.entity_id
    }

    #[inline]
    pub const fn generation(self) -> u32 {
        self.generation.get()
    }
}

impl PartialEq for Entity {
    fn eq(&self, other: &Self) -> bool {
        self.to_bits() == other.to_bits()
    }
}

impl Eq for Entity {}

// The derive macro codegen output is not optimal and can't be optimised as well
// by the compiler. This impl resolves the issue of non-optimal codegen by relying
// on comparing against the bit representation of `Entity` instead of comparing
// the fields. The result is then LLVM is able to optimise the codegen for Entity
// far beyond what the derive macro can.
// See <https://github.com/rust-lang/rust/issues/106107>
impl PartialOrd for Entity {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        // Make use of our `Ord` impl to ensure optimal codegen output
        Some(self.cmp(other))
    }
}

// The derive macro codegen output is not optimal and can't be optimised as well
// by the compiler. This impl resolves the issue of non-optimal codegen by relying
// on comparing against the bit representation of `Entity` instead of comparing
// the fields. The result is then LLVM is able to optimise the codegen for Entity
// far beyond what the derive macro can.
// See <https://github.com/rust-lang/rust/issues/106107>
impl Ord for Entity {
    #[inline]
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // This will result in better codegen for ordering comparisons, plus
        // avoids pitfalls with regards to macro codegen relying on property
        // position when we want to compare against the bit representation.
        self.to_bits().cmp(&other.to_bits())
    }
}

impl core::hash::Hash for Entity {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.to_bits().hash(state);
    }
}

// The contents of `pending` look like this:
// freelist : 可用id区域
// reserved : 被预留的id区域
// ```
// ----------------------------
// |  freelist  |  reserved   |
// ----------------------------
//              ^             ^
//          free_cursor   pending.len()
// ```
/// 用于 管理与分配 Entity
/// #plan : 心智负担较重,等ecs计划第一步完成后,重新设计这个结构
#[derive(Debug,Default)]
pub(crate) struct Entities {
    /// 下标对应的是entity_id\
    /// 存在meta中并不代表是 live Entity ,可能是 dead Entity
    meta: Vec<EntityMeta>,
    /// 存储已被销毁但尚未被重新分配的Entity id
    /// 注意: Entity被摧毁后,对应id的generation会加一
    pending: Vec<EntityId>,
    /// 如果 free_cursor 是正的，表示有那么多 ID 在 pending 列表中是可用的，可以从中分配。\
    /// 如果 free_cursor 是负的(包括0)，表示需要分配新的 ID（即，meta.len() 后面的 ID），这些新 ID 还没有被实际使用
    free_cursor: AtomicIsize,
    /// 当前 live Entity 的数量
    len: u32,
}

impl Entities {
    pub(crate) fn new() -> Entities {
        Self {
            meta: Vec::new(),
            pending: Vec::new(),
            free_cursor: AtomicIsize::new(0),
            len: 0,
        }
    }

    /// 预留一个未来可以使用的 Entity id
    pub(crate) fn reserve_entity(&self) -> Entity {
        let n = self.free_cursor.fetch_sub(1, Ordering::Relaxed);
        if n > 0 {
            // 从freelist中分配一个id
            let entity_id = self.pending[(n - 1) as usize];
            Entity {
                generation: self.meta[entity_id as usize].generation,
                entity_id,
            }
        } else {
            Entity {
                generation: NonZeroU32::new(1).unwrap(),
                entity_id: u32::try_from(self.meta.len() as isize - n)
                    .expect("too many entities"),
            }
        }
    }

    /// 直接分配 Entity id\
    /// 分配Entity后你应该立刻修改 EntityLocation\
    /// #wait : 也许不应该暴露这个api,可能进行一次包装,让修改EntityLocation变成必要
    pub(crate) fn alloc(&mut self) -> Entity {
        self.verify_flushed();

        self.len += 1;
        if let Some(entity_id) = self.pending.pop() {
            let new_free_cursor = self.pending.len() as isize;
            *self.free_cursor.get_mut() = new_free_cursor;
            Entity {
                generation: self.meta[entity_id as usize].generation,
                entity_id,
            }
        } else {
            let entity_id =
                u32::try_from(self.meta.len()).expect("too many entities");
            self.meta.push(EntityMeta::EMPTY);
            Entity {
                generation: NonZeroU32::new(1).unwrap(),
                entity_id,
            }
        }
    }

    /// 释放一个Entity
    pub fn free(
        &mut self,
        entity: Entity,
    ) -> Result<EntityLocation, NoSuchEntity> {
        self.verify_flushed();

        let meta = self
            .meta
            .get_mut(entity.entity_id as usize)
            .ok_or(NoSuchEntity)?;
        if meta.generation != entity.generation
            || meta.location == EntityMeta::EMPTY.location
        {
            return Err(NoSuchEntity);
        }

        meta.generation =
            NonZeroU32::new(u32::from(meta.generation).wrapping_add(1))
                .unwrap_or_else(|| NonZeroU32::new(1).unwrap());

        let loc = mem::replace(&mut meta.location, EntityMeta::EMPTY.location);

        self.pending.push(entity.entity_id);

        let new_free_cursor = self.pending.len() as isize;
        *self.free_cursor.get_mut() = new_free_cursor;
        self.len -= 1;

        Ok(loc)
    }

    pub(crate) fn verify_flushed(&mut self) {
        debug_assert!(
            !self.needs_flush(),
            "flush() needs to be called before this operation is legal"
        );
    }

    /// 若 存在 预留的id 则返回 true
    pub(crate) fn needs_flush(&mut self) -> bool {
        *self.free_cursor.get_mut() != self.pending.len() as isize
    }

    /// 将预留但尚未正式分配的 Entity id 进行初始化和分配\
    /// init(entity_id,entity_location) : 用于将指定id初始化
    pub(crate) fn flush(
        &mut self,
        mut init: impl FnMut(/*entity_id:*/ Entity, &mut EntityLocation),
    ) {
        let free_cursor = *self.free_cursor.get_mut();

        let new_free_cursor = if free_cursor >= 0 {
            free_cursor as usize
        } else {
            //分配meta.len 后的id
            let old_meta_len = self.meta.len();
            let new_meta_len = old_meta_len + -free_cursor as usize;
            self.meta.resize(new_meta_len, EntityMeta::EMPTY);

            self.len += -free_cursor as u32;
            for (id, meta) in
                self.meta.iter_mut().enumerate().skip(old_meta_len)
            {
                init(
                    Entity::from_raw_and_generation(id as u32, meta.generation),
                    &mut meta.location,
                );
            }

            *self.free_cursor.get_mut() = 0;
            0
        };

        self.len += (self.pending.len() - new_free_cursor) as u32;
        for id in self.pending.drain(new_free_cursor..) {
            let meta = &mut self.meta[id as usize];
            //分配pending中的id
            init(
                Entity::from_raw_and_generation(id, meta.generation),
                &mut meta.location,
            );
        }
    }

    /// Returns the location of an [`Entity`].
    /// Note: for pending entities, returns `Some(EntityLocation::INVALID)`.
    #[inline]
    pub fn get(&self, entity: Entity) -> Option<EntityLocation> {
        if let Some(meta) = self.meta.get(entity.index() as usize) {
            if meta.generation != entity.generation
                || meta.location.archetype_id == ArchetypeId::INVALID
            {
                return None;
            }
            Some(meta.location)
        } else {
            None
        }
    }

    /// Updates the location of an [`Entity`]. This must be called when moving the components of
    /// the entity around in storage.
    ///
    /// # Safety
    ///  - `index` must be a valid entity index.
    ///  - `location` must be valid for the entity at `index` or immediately made valid afterwards
    ///    before handing control to unknown code.
    #[inline]
    pub(crate) unsafe fn set(&mut self, index: u32, location: EntityLocation) {
        // SAFETY: Caller guarantees that `index` a valid entity index
        let meta = unsafe { self.meta.get_unchecked_mut(index as usize) };
        meta.location = location;
    }

    #[inline]
    pub fn len(&self) -> u32 {
        self.len
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

#[derive(Debug, Copy, Clone)]
pub(crate) struct EntityMeta {
    pub(crate) generation: EntityGeneration,
    pub(crate) location: EntityLocation,
}
impl EntityMeta {
    /// 表示一个未初始化或无效的实体元数据
    /// 为了 待处理实体的meta 而存在
    pub(crate) const EMPTY: EntityMeta = EntityMeta {
        generation: NonZeroU32::MIN,
        location: EntityLocation::INVALID,
    };
}

/// Entity 的位置
///
/// 包括 Archetype和Table的位置,
/// Archetype声明 Entity包含的Component,
/// Table存储Entity这种Component的实际数据
#[derive(Debug, Copy, Clone, PartialEq)]
pub(crate) struct EntityLocation {
    pub archetype_id: ArchetypeId,
    pub archetype_row: ArchetypeRow,
    pub table_id: TableId,
    pub table_row: TableRow,
}
impl EntityLocation {
    /// **待定实体**和**无效实体**的位置
    const INVALID: EntityLocation = EntityLocation {
        archetype_id: ArchetypeId::INVALID,
        archetype_row: ArchetypeRow::INVALID,
        table_id: TableId::INVALID,
        table_row: TableRow::INVALID,
    };
}

/// 当前结构表示 不存在具有特定 id 的 live Entity (往往是因为 generation 不同导致的)
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct NoSuchEntity;
impl fmt::Display for NoSuchEntity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad("no such entity")
    }
}
impl Error for NoSuchEntity {}

/// #wait 类型等待构造,暂时占位
pub(crate) struct EntityBuilder<'w> {
    world: &'w World,
    entity: Entity,
    Component: (),
}
impl<'w> EntityBuilder<'w> {
    pub fn new(world: &'w World) -> EntityBuilder<'w> {
        todo!()
    }

    pub fn with(self, component: ()) -> EntityBuilder<'w> {
        todo!()
    }

    pub fn build(self) {
        todo!()
    }
}

/// EntityManager负责增删改查World中的Entity
/// #wait
struct EntityManager();
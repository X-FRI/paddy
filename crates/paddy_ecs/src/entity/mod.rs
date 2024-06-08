use crate::world::World;

#[derive(Debug, Clone)]
struct EntityId(u32);
#[derive(Debug, Clone)]
struct EntityIndex(usize);
#[derive(Debug)]
pub(crate) struct Entity {
    /// Entity的唯一ID
    entity_id: EntityId,
    /// 在存储中的索引
    entity_index: EntityIndex,
}

impl Entity {
    #[inline]
    pub fn id(&self) -> &EntityId {
        &self.entity_id
    }
    #[inline]
    pub fn index(&self) -> &EntityIndex {
        &self.entity_index
    }
}

/// #wait 类型等待构造,暂时占位
pub(crate) struct EntityBuilder<'w> {
    world: &'w World,
    entity: Entity,
    Component: (),
}
impl<'w> EntityBuilder<'w> {

    pub fn new(world: &'w World)->EntityBuilder<'w>{
        todo!()
    }

    pub fn with(self,component:())->EntityBuilder<'w>{
        todo!()
    }

    pub fn build(self){
        todo!()
    }

}

/// EntityManager负责增删改查World中的Entity
/// #wait
struct EntityManager();


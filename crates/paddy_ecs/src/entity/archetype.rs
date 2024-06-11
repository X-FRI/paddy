

#[derive(Debug)]
struct ArchetypeId(u32);


/// 一种Entity的组件类型集合
/// Archetype 表示一种实体类型
/// #wait
#[derive(Debug)]
struct Archetype{
    archetype_id: ArchetypeId,
}


#[derive(Debug)]
struct Archetypes {
    archetypes: Vec<Archetype>,
    archetype_component_count: u32,
}


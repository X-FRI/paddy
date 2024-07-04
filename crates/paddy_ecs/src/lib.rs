#![allow(dead_code)] //有点吵,给关了

/// #plan : 为了方便而存在,未来会被移除
#[cfg(test)]
mod _tests;
/// #plan : 为了方便而存在,未来会被移除
mod _todo;
mod archetype;
mod borrow;
mod bundle;
mod component;
mod debug;
mod entity;
mod query;
mod storage;
mod system;
mod world;

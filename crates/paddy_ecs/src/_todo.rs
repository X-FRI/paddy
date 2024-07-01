
// 在参考(copy) bevy_ecs 代码时,我们暂时不想去实现某些特性,就需要标注 todo!("...")
// 若未来需要添加特性后,为了方便追踪代码,这里特别的去 包装todo


/// 对于稀疏集的特性 的实现
pub mod for_sparse {
    pub fn _sparse()->!{
        todo!("waiting for the implementation of sparse sets")   
    }
}


/// 对于 Tick 的实现
pub mod for_tick {

}

// 比较难,要花点时间读源码
// 都不知道应该从哪里开始起手

mod access;
mod iter;


pub(crate) trait WorldQuery {
    /// 查询的 返回值的类型
    type Item<'a>;
    /// 用于如何从 World 中提取数据
    #[doc(hidden)]
    type Fetch: Fetch;

    unsafe fn get<'a>(fetch: &Self::Fetch, n: usize) -> Self::Item<'a>;
}

pub(crate) unsafe trait Fetch: Clone + Sized {
    /// 构建 [`Fetch`] 所需的状态
    ///
    /// 该状态被缓存，以减少每次查询时的计算成本
    type State: Copy;
}

use std::mem::ManuallyDrop;

pub mod hash;

pub use paddy_utils_proc_macros::*;


/// 一个在对象被销毁时调用函数的类型\
/// `callback` 将在`drop`中被调用
///
/// 这可以用于确保即使在发生 panic 的情况下，也能运行释放空间
///
/// 注意，这只适用于会 [unwind](https://doc.rust-lang.org/nomicon/unwinding.html) 的 panic
/// 如果 panic 没有展开（如在使用 `abort` 策略时），则 `OnDrop` 中的代码将不会执行
///
/// 在大多数情况下，这个功能将正常工作
///
/// # Examples
/// ```
/// # use paddy_utils::OnDrop;
/// fn test_panic(do_panic: bool, log: impl FnOnce(&str)) {
///     let _catch = OnDrop::new(|| log("Oops, a panic occurred and this function didn't complete!"));
///
///     if do_panic { panic!() }
///
/// // 只有在发生 panic 时，log才会被执行
/// // 如果我们移除这一行，那么log将在无论是否发生 panic 的情况下都被执行 \
/// // (因为我们依靠的是Drop trait,不使用forget,_catch必然指向drop)
/// // 类似于 `try ... finally` 代码块
///     std::mem::forget(_catch);
/// }
/// 
/// test_panic(false, |_| unreachable!());
/// let mut did_log = false;
/// std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
///   test_panic(true, |_| did_log = true);
/// }));
/// assert!(did_log);
pub struct OnDrop<F: FnOnce()> {
    callback: ManuallyDrop<F>,
}

impl<F: FnOnce()> OnDrop<F> {
    /// Returns an object that will invoke the specified callback when dropped.
    pub fn new(callback: F) -> Self {
        Self {
            callback: ManuallyDrop::new(callback),
        }
    }
}

impl<F: FnOnce()> Drop for OnDrop<F> {
    fn drop(&mut self) {
        // SAFETY: We may move out of `self`, since this instance can never be observed after it's dropped.
        let callback = unsafe { ManuallyDrop::take(&mut self.callback) };
        callback();
    }
}

use std::cell::UnsafeCell;

use paddy_ptr::UnsafeCellDeref;


/// The (arbitrarily chosen) minimum number of world tick increments between `check_tick` scans.
///
/// Change ticks can only be scanned when systems aren't running. Thus, if the threshold is `N`,
/// the maximum is `2 * N - 1` (i.e. the world ticks `N - 1` times, then `N` times).
///
/// If no change is older than `u32::MAX - (2 * N - 1)` following a scan, none of their ages can
/// overflow and cause false positiv    es.
// (518,400,000 = 1000 ticks per frame * 144 frames per second * 3600 seconds per hour)
pub const CHECK_TICK_THRESHOLD: u32 = 518_400_000;

/// The maximum change tick difference that won't overflow before the next `check_tick` scan.
///
/// Changes stop being detected once they become this old.
pub const MAX_CHANGE_AGE: u32 = u32::MAX - (2 * CHECK_TICK_THRESHOLD - 1);

/// A value that tracks when a system ran relative to other systems.\
/// 一个值，用于跟踪一个系统相对于其他系统的运行时间
///
/// This is used to power change detection.
///
/// #note : 尚未运行的system的 `Tick` 值为 0
#[derive(Copy, Clone, Default, Debug, Eq, Hash, PartialEq)]
pub struct Tick {
    tick: u32,
}

impl Tick {
    /// The maximum relative age for a change tick.
    /// The value of this is equal to [`MAX_CHANGE_AGE`].
    ///
    /// Since change detection will not work for any ticks older than this,
    /// ticks are periodically scanned to ensure their relative values are below this.
    pub const MAX: Self = Self::new(MAX_CHANGE_AGE);

    /// 创建一个包装给定值的 [`Tick`]
    #[inline]
    pub const fn new(tick: u32) -> Self {
        Self { tick }
    }

    /// 获取这个 tick 的值
    #[inline]
    pub const fn get(self) -> u32 {
        self.tick
    }

    /// 设置这个 tick 的值
    #[inline]
    pub fn set(&mut self, tick: u32) {
        self.tick = tick;
    }

    /// Returns `true` if this `Tick` occurred since the system's `last_run`.\
    /// 如果这个 `Tick` 发生在system的 `last_run` 之后，则返回 `true`
    ///
    /// `this_run` is the current tick of the system, used as a reference to help deal with wraparound.\
    /// `this_run` 是system的当前tick，作为参考用于处理溢出
    #[inline]
    pub fn is_newer_than(self, last_run: Tick, this_run: Tick) -> bool {
        // This works even with wraparound because the world tick (`this_run`) is always "newer" than
        // `last_run` and `self.tick`, and we scan periodically to clamp `ComponentTicks` values
        // so they never get older than `u32::MAX` (the difference would overflow).
        //
        // The clamp here ensures determinism (since scans could differ between app runs).
        let ticks_since_insert =
            this_run.relative_to(self).tick.min(MAX_CHANGE_AGE);
        let ticks_since_system =
            this_run.relative_to(last_run).tick.min(MAX_CHANGE_AGE);

        ticks_since_system > ticks_since_insert
    }

    /// Returns a change tick representing the relationship between `self` and `other`.\
    /// 返回 `self` 与 `other` 之间的相对 tick 差值
    ///
    /// `self - other`
    #[inline]
    pub(crate) fn relative_to(self, other: Self) -> Self {
        let tick = self.tick.wrapping_sub(other.tick);
        Self { tick }
    }

    /// 如果超过了 [`Tick::MAX`]，则封装这个 tick 的值
    ///
    /// 如果执行了封装操作，返回 `true`。否则，返回 `false`。
    #[inline]
    pub(crate) fn check_tick(&mut self, tick: Tick) -> bool {
        let age = tick.relative_to(*self);
        // 这个比较 假设 `age` 之前没有溢出过 `u32::MAX`，只要这个检查总是在这种情况发生之前运行，这就是正确的
        if age.get() > Self::MAX.get() {
            *self = tick.relative_to(Self::MAX);
            true
        } else {
            false
        }
    }
}

/// Interior-mutable access to the [`Tick`]s for a single component or resource.\
/// 对单个组件或资源的 [`Tick`]s 进行内部可变访问
#[derive(Copy, Clone, Debug)]
pub struct TickCells<'a> {
    /// The tick indicating when the value was added to the world.\
    /// 表示值何时被添加到世界的tick
    pub added: &'a UnsafeCell<Tick>,
    /// The tick indicating the last time the value was modified.\
    /// 表示值上次被修改的tick
    pub changed: &'a UnsafeCell<Tick>,
}

impl<'a> TickCells<'a> {
    /// # Safety
    /// All cells contained within must uphold the safety invariants of [`UnsafeCellDeref::read`].\
    /// 包含的所有单元格必须维护 [`UnsafeCellDeref::read`] 的安全性不变量
    #[inline]
    pub(crate) unsafe fn read(&self) -> ComponentTicks {
        ComponentTicks {
            // SAFETY: The callers uphold the invariants for `read`.
            added: unsafe { self.added.read() },
            // SAFETY: The callers uphold the invariants for `read`.
            changed: unsafe { self.changed.read() },
        }
    }
}

/// Records when a component or resource was added and when it was last mutably dereferenced (or added).\
/// 记录一个组件或资源的 添加时间 和 最后一次被 可变引用（或添加）的时间
#[derive(Copy, Clone, Debug)]
pub struct ComponentTicks { 
    pub(crate) added: Tick,
    pub(crate) changed: Tick,
}

impl ComponentTicks {
    /// Returns `true` if the component or resource was added after the system last ran
    /// (or the system is running for the first time).\
    /// 如果组件或资源是在系统上次运行之后被添加的（或者系统是第一次运行），则返回 `true`
    #[inline]
    pub fn is_added(&self, last_run: Tick, this_run: Tick) -> bool {
        self.added.is_newer_than(last_run, this_run)
    }

    /// Returns `true` if the component or resource was added or mutably dereferenced after the system last ran
    /// (or the system is running for the first time).\
    /// 如果组件或资源是在系统上次运行之后被添加或可变引用的（或者系统是第一次运行），则返回 `true`
    #[inline]
    pub fn is_changed(&self, last_run: Tick, this_run: Tick) -> bool {
        self.changed.is_newer_than(last_run, this_run)
    }

    /// Returns the tick recording the time this component or resource was most recently changed.\
    /// 返回 记录这个组件或资源 最近一次被更改的tick
    #[inline]
    pub fn last_changed_tick(&self) -> Tick {
        self.changed
    }

    /// Returns the tick recording the time this component or resource was added.\
    /// 返回 记录这个组件或资源 被添加的tick
    #[inline]
    pub fn added_tick(&self) -> Tick {
        self.added
    }

    /// 创建一个新的 [`ComponentTicks`]，并将其初始化为给定的tick
    pub(crate) fn new(change_tick: Tick) -> Self {
        Self {
            added: change_tick,
            changed: change_tick,
        }
    }

    /// 手动设置 change tick.
    ///
    /// This is normally done automatically via the [`DerefMut`](std::ops::DerefMut) implementation
    /// on [`Mut<T>`](crate::change_detection::Mut), [`ResMut<T>`](crate::change_detection::ResMut), etc.
    /// However, components and resources that make use of interior mutability might require manual updates.
    ///
    /// # Example
    /// ```no_run
    /// # use bevy_ecs::{world::World, component::ComponentTicks};
    /// let world: World = unimplemented!();
    /// let component_ticks: ComponentTicks = unimplemented!();
    ///
    /// component_ticks.set_changed(world.read_change_tick());
    /// ```
    #[inline]
    pub fn set_changed(&mut self, change_tick: Tick) {
        self.changed = change_tick;
    }
}


use std::sync::atomic::{AtomicUsize, Ordering};

const UNIQUE_BIT: usize = !(usize::max_value() >> 1);

const COUNTER_MASK: usize = usize::max_value() >> 1;

// #plan: 我并不想使用这个,未来移除它
struct AtomicBorrow(AtomicUsize);

impl AtomicBorrow {
    pub(crate) const fn new() -> Self {
        Self(AtomicUsize::new(0))
    }

    pub(crate) fn borrow(&self) -> bool {
        let prev_value = self.0.fetch_add(1, Ordering::Acquire);

        if prev_value & COUNTER_MASK == COUNTER_MASK {
            core::panic!("immutable borrow counter overflowed")
        }

        if prev_value & UNIQUE_BIT != 0 {
            self.0.fetch_sub(1, Ordering::Release);
            false
        } else {
            true
        }
    }

    pub(crate) fn borrow_mut(&self) -> bool {
        self.0
            .compare_exchange(
                0,
                UNIQUE_BIT,
                Ordering::Acquire,
                Ordering::Relaxed,
            )
            .is_ok()
    }

    pub(crate) fn release(&self) {
        let value = self.0.fetch_sub(1, Ordering::Release);
        debug_assert!(value != 0, "unbalanced release");
        debug_assert!(
            value & UNIQUE_BIT == 0,
            "shared release of unique borrow"
        );
    }

    pub(crate) fn release_mut(&self) {
        let value = self.0.fetch_and(!UNIQUE_BIT, Ordering::Release);
        debug_assert_ne!(
            value & UNIQUE_BIT,
            0,
            "unique release of shared borrow"
        );
    }
}

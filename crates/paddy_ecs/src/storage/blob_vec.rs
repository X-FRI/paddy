use std::{
    alloc::{handle_alloc_error, Layout}, cell::UnsafeCell, num::NonZeroUsize, ptr::NonNull
};

type DropFn = unsafe fn(NonNull<u8>);

/// 用于密集存储同质(同结构)数据\
/// 存储类似于数组,不过它是动态可变大小\
/// item_layout = Layout::new::\<T\>()\
/// \[T;capacity\]
pub(crate) struct BlobVec {
    /// 元素的内存布局
    item_layout: Layout,
    /// 容量:可容纳的元素 数量
    capacity: usize,
    /// 当前元素数量
    len: usize,
    /// 数组数据
    data: NonNull<u8>,
    // Some(f) ,f 释放元素空间的函数
    drop: Option<DropFn>,
}

impl BlobVec {
    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }
    /// true : is empty
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    #[inline]
    pub fn capacity(&self) -> usize {
        self.capacity
    }
    #[inline]
    pub fn layout(&self) -> Layout {
        self.item_layout
    }

    /// capacity : 初始容量(仅对于非ZST类型),ZST为usize::MAX
    pub unsafe fn new(item_layout: Layout, drop: Option<DropFn>, capacity: usize) -> BlobVec {
        let align = NonZeroUsize::new(item_layout.align()).expect("alignment must be > 0");
        debug_assert!(align.is_power_of_two(), "Alignment must be power of two.");
        // 延迟初始化 (当前给予的是无效地址)
        let data = unsafe { NonNull::new_unchecked(align.get() as *mut u8) };
        if item_layout.size() == 0 {
            BlobVec {
                // 这个无法访问,是无效地址(也不需要被访问,因为是ZST)
                data,
                // ZST(Zero Sized Type, 零大小类型) 的BlobVec 最大容量为 usize::MAX
                capacity: usize::MAX,
                len: 0,
                item_layout,
                drop,
            }
        } else {
            let mut blob_vec = BlobVec {
                data,
                capacity: 0,
                len: 0,
                item_layout,
                drop,
            };
            blob_vec.reserve_exact(capacity);
            blob_vec
        }
    }

    /// 将剩余容量扩展到 additional 大小\
    /// 若 剩余容量>=additional 则 啥也不做
    pub fn reserve_exact(&mut self, additional: usize) {
        // 剩余容量
        let available_space = self.capacity - self.len;
        if available_space < additional {
            // #safety : available_space < additional ==> additional - available_space > 0
            let increment = unsafe { NonZeroUsize::new_unchecked(additional - available_space) };
            self.grow_exact(increment);
        }
    }
    /// 将剩余容量扩展到 max{ additional , capacity + 剩余容量 } 大小\
    /// 若 剩余容量>=additional 则 啥也不做
    ///
    /// 不太理解这个函数的作用... (可能单纯是用于扩充大量容量吧)
    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        // 写一个内部函数 似乎是 bevy_ecs 的一种优化 (看不懂...)
        #[cold]
        fn do_reserve(slf: &mut BlobVec, additional: usize) {
            let increment = slf.capacity.max(additional - (slf.capacity - slf.len));
            let increment = NonZeroUsize::new(increment).unwrap();
            slf.grow_exact(increment);
        }
        if self.capacity - self.len < additional {
            do_reserve(self, additional);
        }
    }
    /// 增加increment多的容量
    fn grow_exact(&mut self, increment: NonZeroUsize) {
        let new_capacity = self
            .capacity
            .checked_add(increment.get())
            .expect("capacity overflow");
        let new_layout =
            array_layout(&self.item_layout, new_capacity).expect("array layout should be valid");
        let new_data = if self.capacity == 0 {
            // 之前容量为0,说明未被初始化,直接进行初始化即可
            // SAFETY:
            // - layout has non-zero size as per safety requirement
            unsafe { std::alloc::alloc(new_layout) }
        } else {
            // SAFETY:
            // - ptr was be allocated via this allocator
            // - the layout of the ptr was `array_layout(self.item_layout, self.capacity)`
            // - `item_layout.size() > 0` and `new_capacity > 0`, so the layout size is non-zero
            // - "new_size, when rounded up to the nearest multiple of layout.align(), must not overflow (i.e., the rounded value must be less than usize::MAX)",
            // since the item size is always a multiple of its alignment, the rounding cannot happen
            // here and the overflow is handled in `array_layout`
            unsafe {
                std::alloc::realloc(
                    self.data.as_ptr(),
                    array_layout(&self.item_layout, self.capacity)
                        .expect("array layout should be valid"),
                    new_layout.size(),
                )
            }
        };

        self.data = NonNull::new(new_data).unwrap_or_else(|| handle_alloc_error(new_layout));
        self.capacity = new_capacity;
    }

    /// 初始化对应下标的值
    /// warn: 注意index应该是 非剩余容量的空间
    #[inline]
    pub unsafe fn initialize_unchecked(&mut self, index: usize, value: NonNull<u8>) {
        debug_assert!(index < self.len());
        let ptr = self.get_unchecked(index);
        std::ptr::copy_nonoverlapping::<u8>(value.as_ptr(), ptr.as_ptr(), self.item_layout.size());
    }

    /// 向尾部添加一个值
    #[inline]
    pub unsafe fn push(&mut self, value: NonNull<u8>) {
        self.reserve(1);
        let index = self.len;
        self.len += 1;
        self.initialize_unchecked(index, value);
    }

    /// 
    #[inline]
    pub unsafe fn set_len(&mut self, len: usize) {
        debug_assert!(len <= self.capacity());
        self.len = len;
    }

    #[inline]
    pub unsafe fn get_unchecked(&self, index: usize) -> NonNull<u8> {
        debug_assert!(index < self.len());
        let size = self.item_layout.size();
        unsafe { self.data.byte_add(index * size) }
    }

    /// 获取 非剩余容量空间 的切片
    pub unsafe fn get_slice<T>(&self) -> &[UnsafeCell<T>] {
        unsafe { std::slice::from_raw_parts(self.data.as_ptr() as *const UnsafeCell<T>, self.len) }
    }

    #[inline]
    pub unsafe fn deref<T>(&self, index: usize) -> &T {
        let ptr = self.get_unchecked(index).as_ptr().cast::<T>();
        unsafe { &*ptr }
    }

    
    pub fn clear(&mut self) {
        let len = self.len;
        self.len = 0;
        if let Some(drop) = self.drop {
            let size = self.item_layout.size();
            for i in 0..len {
                let item = unsafe { self.data.byte_add(i * size) };
                unsafe { drop(item) };
            }
        }
    }

}

impl std::fmt::Debug for BlobVec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BlobVec")
            .field("item_layout", &self.item_layout)
            .field("capacity", &self.capacity)
            .field("len", &self.len)
            .field("data", &self.data)
            .finish()
    }
}

/// layout = Layout::new::\<T\>()\
/// 创建一个布局，描述 `[T; n]` 的记录。
fn array_layout(layout: &Layout, n: usize) -> Option<Layout> {
    let (array_layout, offset) = repeat_layout(layout, n)?;
    debug_assert_eq!(layout.size(), offset);
    Some(array_layout)
}

/// 创建一个布局，以描述 `layout` 的 `n` 实例的记录，并在每个实例之间使用适当的填充量，以确保为每个实例提供其请求的大小和对齐方式。
/// 成功后，返回 `(k, offs)`，其中 `k` 是数组的布局，`offs` 是数组中每个元素的起点之间的距离。
///
fn repeat_layout(layout: &Layout, n: usize) -> Option<(Layout, usize)> {
    // 这不会溢出。引用 Layout 的不变式:
    // > `size`, 当四舍五入到 `align` 的最接近倍数时，
    // > 不得溢出 (即，四舍五入的值必须小于
    // > `usize::MAX`)
    let padded_size = layout.size() + padding_needed_for(layout, layout.align());
    let alloc_size = padded_size.checked_mul(n)?;

    // #safety : 已知 self.align 是有效的，并且 alloc_size 已被填充。
    unsafe {
        Some((
            Layout::from_size_align_unchecked(alloc_size, layout.align()),
            padded_size,
        ))
    }
}

/// 返回必须在 `layout` 之后插入的填充量，以确保以下地址满足 `align` (以字节为单位)。
///
/// 例如，如果 `layout.size()` 为 9，则 `layout.padding_needed_for(4)` 返回 3，因为这是获得 4 对齐地址所需的最小填充字节数 (假设相应的存储块从 4 对齐地址开始)。
///
///
/// 如果 `align` 不是 2 的幂，则此函数的返回值没有意义。
///
/// 注意，返回值的实用程序要求 `align` 小于或等于整个分配的内存块的起始地址的对齐方式。满足此约束的一种方法是确保 `align <= self.align()`。
///
pub const fn padding_needed_for(layout: &Layout, align: usize) -> usize {
    let len = layout.size();

    // 向上取整的值为:
    //   len_rounded_up = (len + align - 1) & !(align - 1);
    // 然后返回填充差异: `len_rounded_up - len`.
    //
    // 我们在整个过程中都使用模块化的算法:
    //
    // 1. align 保证 > 0，因此 align - 1 始终有效。
    //
    // 2.
    // `len + align - 1` &-mask with `!(align - 1)` 最多可以溢出 `align - 1`，因此 &-mask with `!(align - 1)` 将确保 `len_rounded_up` 本身为 0。
    //
    //    因此，当返回的填充添加到 `len` 时，将产生 0，该填充简单地满足了 `align` 的对齐方式。
    //
    // (当然，尝试以上述方式分配其大小和填充溢出的内存块无论如何都会导致分配器产生错误。)
    //

    let len_rounded_up = len.wrapping_add(align).wrapping_sub(1) & !align.wrapping_sub(1);
    len_rounded_up.wrapping_sub(len)
}

mod tests {
    use std::{alloc::Layout, num::NonZeroUsize, ptr::NonNull};

    use super::BlobVec;

    #[test]
    fn test() {
        #[derive(Debug)]
        struct A {
            a: u32,
            b: u64,
        }
        unsafe {
            let mut blob = BlobVec::new(Layout::new::<A>(), None, 5);
            let mut val = A {
                a: 123,
                b: 456,
            };
            blob.set_len(2);
            blob.initialize_unchecked(1,NonNull::new_unchecked(&mut val as *mut A as *mut u8));
            blob.reserve(6);
            println!("{blob:?}");
            let slice = blob.get_slice::<A>();
            println!("{:?}",(*(slice[1].get() as *mut u8 as *mut A)).a);
            val.a = 789;
            println!("{val:?}");
            println!("{:?}",(*(slice[1].get() as *mut u8 as *mut A)).a);
            println!("{:?}",(*(slice[1].get() as *mut u8 as *mut A)).b);


        }
    }
}

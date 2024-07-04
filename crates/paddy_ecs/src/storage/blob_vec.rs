use std::{
    alloc::{handle_alloc_error, Layout},
    cell::UnsafeCell,
    num::NonZeroUsize,
    ptr::NonNull,
};

use paddy_ptr::{OwningPtr, Ptr, PtrMut};
use paddy_utils::OnDrop;

type DropFn = unsafe fn(OwningPtr<'_>);

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
    pub unsafe fn new(
        item_layout: Layout,
        drop: Option<DropFn>,
        capacity: usize,
    ) -> BlobVec {
        let align = NonZeroUsize::new(item_layout.align())
            .expect("alignment must be > 0");
        debug_assert!(
            align.is_power_of_two(),
            "Alignment must be power of two."
        );
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
            let increment = unsafe {
                NonZeroUsize::new_unchecked(additional - available_space)
            };
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
            let increment =
                slf.capacity.max(additional - (slf.capacity - slf.len));
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
        let new_layout = array_layout(&self.item_layout, new_capacity)
            .expect("array layout should be valid");
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

        self.data = NonNull::new(new_data)
            .unwrap_or_else(|| handle_alloc_error(new_layout));
        self.capacity = new_capacity;
    }

    /// 初始化对应下标的值
    ///
    /// # Note
    /// - 注意index应该在 非剩余容量的空间 内
    /// - @`value` 应该指向被擦出类型前的类型
    #[inline]
    pub unsafe fn initialize_unchecked(
        &mut self,
        index: usize,
        value: OwningPtr<'_>,
    ) {
        debug_assert!(index < self.len());
        let ptr = self.get_unchecked(index);
        std::ptr::copy_nonoverlapping::<u8>(
            value.as_ptr(),
            ptr.as_ptr(),
            self.item_layout.size(),
        );
    }

    /// 将 `index` 位置的值替换为 `value`
    ///
    /// # Safety
    /// - `index` 必须在有效范围内
    /// - 从 `index` 开始的 [`BlobVec`] 内存块，且大小与此 [`BlobVec`] 的 `item_layout` 匹配，
    ///   必须已经被初始化为一个与此 [`BlobVec`] 的 `item_layout` 匹配的项
    /// - `*value` 所指向的内存也必须已初始化为一个与此 [`BlobVec`] 的 `item_layout` 匹配的项
    ///
    /// # Note
    /// - 此函数不会进行边界检查
    ///
    pub unsafe fn replace_unchecked(
        &mut self,
        index: usize,
        value: OwningPtr<'_>,
    ) {
        debug_assert!(index < self.len());

        // 获取将被替换的 vec 中的值的指针
        // SAFETY: The caller ensures that `index` fits in this vector.
        let destination =
            NonNull::from(unsafe { self.get_unchecked_mut(index) });
        let source = value.as_ptr();

        if let Some(drop) = self.drop {
            // 临时将长度设置为0，这样如果`drop`发生panic，
            // 调用者不会因为`BlobVec`中有一个已被释放的元素在其初始化范围内而陷入困境
            let old_len = self.len;
            self.len = 0;

            // 从vec中移除旧值的所有权，以便可以将其释放
            // SAFETY:
            // - `destination`是从该vec中的`PtrMut`获取的，这确保它是非空的，
            //   对底层类型对齐，并具有适当的来源
            // - 存储位置稍后将被`value`覆盖，这确保了
            //   元素不会被观察到或重复释放
            // - 如果发生panic，`self.len`将保持为`0`，这确保了不会发生重复释放,
            //   相反，所有元素都将被忘记
            let old_value = unsafe { OwningPtr::new(destination) };

            // 这个闭包将在`drop()`发生panic时运行，
            // 这确保了`value`不会被忘记
            let on_unwind = OnDrop::new(|| drop(value));

            drop(old_value);

            // 如果上面的代码没有panic，确保`value`不会被释放
            core::mem::forget(on_unwind);

            // 由于panic不再可能，使vec的内容重新可见
            self.len = old_len;
        }

        // 将新值复制到vec中，覆盖先前的值
        // SAFETY:
        // - `source`和`destination`是从`OwningPtr`获得的，这确保了它们
        //   对读取和写入都是有效的
        // - The value behind `source` will only be dropped if the above branch panics,
        //   so it must still be initialized and it is safe to transfer ownership into the vector.\
        //   如果上述分支恐慌，`source`后面的值只会被释放，
        //   因此它必须仍然被初始化，并且可以安全地将所有权转移到向量中
        // - `source`和`destination`是从不同的内存位置获得的，
        //   我们对这些位置都有独占访问权，因此它们保证不会重叠
        unsafe {
            std::ptr::copy_nonoverlapping::<u8>(
                source,
                destination.as_ptr(),
                self.item_layout.size(),
            );
        }
    }

    /// 向尾部添加一个值
    #[inline]
    pub unsafe fn push(&mut self, value: OwningPtr<'_>) {
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
    pub unsafe fn get_unchecked(&self, index: usize) -> Ptr<'_> {
        debug_assert!(index < self.len());
        let size = self.item_layout.size();
        unsafe { self.get_ptr().byte_add(index * size) }
    }

    #[inline]
    pub unsafe fn get_unchecked_mut(&mut self, index: usize) -> PtrMut<'_> {
        debug_assert!(index < self.len());
        let size = self.item_layout.size();
        unsafe { self.get_ptr_mut().byte_add(index * size) }
    }

    /// 在给定的 `index` 处执行“交换移除”，移除 `index` 处的项，并将 [`BlobVec`] 中最后一项移动到 `index` 位置（如果 `index` 不是最后一项）
    ///
    /// 如果需要，调用者有责任释放返回的指针
    ///
    /// # Safety
    /// It is the caller's responsibility to ensure that `index` is less than `self.len()`.
    #[inline]
    #[must_use = "The returned pointer should be used to dropped the removed element"]
    pub unsafe fn swap_remove_and_forget_unchecked(
        &mut self,
        index: usize,
    ) -> OwningPtr<'_> {
        debug_assert!(index < self.len());
        // 由于 `index` 必须严格小于 `self.len` 且 `index` 至少为零，
        // 因此 `self.len` 必须至少为一。这样的话，不会下溢。
        let new_len = self.len - 1;
        let size = self.item_layout.size();
        if index != new_len {
            std::ptr::swap_nonoverlapping::<u8>(
                self.get_unchecked_mut(index).as_ptr(),
                self.get_unchecked_mut(new_len).as_ptr(),
                size,
            );
        }
        self.len = new_len;
        // Cannot use get_unchecked here as this is technically out of bounds after changing len.
        // SAFETY:
        // - `new_len` is less than the old len, so it must fit in this vector's allocation.
        // - `size` is a multiple of the erased type's alignment,
        //   so adding a multiple of `size` will preserve alignment.
        // - The removed element lives as long as this vector's mutable reference.
        let p = unsafe { self.get_ptr_mut().byte_add(new_len * size) };
        // SAFETY: The removed element is unreachable by this vector so it's safe to promote the
        // `PtrMut` to an `OwningPtr`.
        unsafe { p.promote() }
    }

    /// 移除 `index` 处的值并将存储的值复制到 `ptr` 中
    ///
    /// 不对 `index` 进行边界检查.
    /// 被移除的元素由 `BlobVec` 的最后一个元素替换
    ///
    /// # Safety
    /// 调用者有责任确保 `index` 小于 `self.len()`
    /// 并且 `self[index]` 已经正确初始化。
    #[inline]
    pub unsafe fn swap_remove_unchecked(
        &mut self,
        index: usize,
        ptr: PtrMut<'_>,
    ) {
        debug_assert!(index < self.len());
        let last = self.get_unchecked_mut(self.len - 1).as_ptr();
        let target = self.get_unchecked_mut(index).as_ptr();
        // 将 index 处的项复制到提供的 ptr 中
        std::ptr::copy_nonoverlapping::<u8>(
            target,
            ptr.as_ptr(),
            self.item_layout.size(),
        );
        // Recompress the storage by moving the previous last element into the
        // now-free row overwriting the previous data. The removed row may be the last
        // one so a non-overlapping copy must not be used here.
        std::ptr::copy::<u8>(last, target, self.item_layout.size());
        // Invalidate the data stored in the last row, as it has been moved
        self.len -= 1;
    }

    /// 移除 `index` 处的值并将其drop。
    /// 不对 `index` 进行边界检查。
    /// 被移除的元素由 `BlobVec` 的最后一个元素替换。
    ///
    /// # Safety
    /// It is the caller's responsibility to ensure that `index` is `< self.len()`.
    #[inline]
    pub unsafe fn swap_remove_and_drop_unchecked(&mut self, index: usize) {
        debug_assert!(index < self.len());
        let drop = self.drop;
        let value = self.swap_remove_and_forget_unchecked(index);
        if let Some(drop) = drop {
            drop(value);
        }
    }

    /// 获取指向 vec 起始位置的 [`Ptr`]
    #[inline]
    pub fn get_ptr(&self) -> Ptr<'_> {
        // SAFETY: the inner data will remain valid for as long as 'self.
        unsafe { Ptr::new(self.data) }
    }

    /// 获取指向 vec 起始位置的 [`PtrMut`]
    #[inline]
    pub fn get_ptr_mut(&mut self) -> PtrMut<'_> {
        // SAFETY: the inner data will remain valid for as long as 'self.
        unsafe { PtrMut::new(self.data) }
    }

    /// 获取 非剩余容量空间 的切片
    pub unsafe fn get_slice<T>(&self) -> &[UnsafeCell<T>] {
        unsafe {
            std::slice::from_raw_parts(
                self.data.as_ptr() as *const UnsafeCell<T>,
                self.len,
            )
        }
    }

    /// #plan : remove the function
    #[inline]
    unsafe fn deref<T>(&self, index: usize) -> &T {
        let ptr = self.get_unchecked(index).as_ptr().cast::<T>();
        unsafe { &*ptr }
    }

    /// 释放所有元素数据,但容量不变
    pub fn clear(&mut self) {
        let len = self.len;
        self.len = 0;
        if let Some(drop) = self.drop {
            let size = self.item_layout.size();
            for i in 0..len {
                let item =
                    unsafe { self.get_ptr_mut().byte_add(i * size).promote() };
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

impl Drop for BlobVec {
    fn drop(&mut self) {
        self.clear();
        let array_layout = array_layout(&self.item_layout, self.capacity)
            .expect("array layout should be valid");
        if array_layout.size() > 0 {
            // SAFETY: data ptr layout is correct, swap_scratch ptr layout is correct
            unsafe {
                std::alloc::dealloc(self.get_ptr_mut().as_ptr(), array_layout);
            }
        }
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
    let padded_size =
        layout.size() + padding_needed_for(layout, layout.align());
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

    let len_rounded_up =
        len.wrapping_add(align).wrapping_sub(1) & !align.wrapping_sub(1);
    len_rounded_up.wrapping_sub(len)
}

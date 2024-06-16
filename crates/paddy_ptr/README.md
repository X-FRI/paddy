

对指针操作有点多,core所提供的指针 约束性 并不高,所以我们需要bevy_ptr (它做的很好,没必要自己在写一份)

这里我们是直接copy的源码,方便未来添加和修改代码

当前crate的目的是:
- 安全的指针操作  ：通过封装原始指针，确保在解引用和操作时的安全性
- 类型擦除      ：允许在编译时不需要知道具体类型的情况下使用指针
- 对齐安全      ：区分对齐和非对齐的指针，防止由于未对齐的内存访问引发的错误
- 生命周期管理   ：通过严格的生命周期规则处理数据的所有权和借用


## Standard Pointers

|Pointer Type       |Lifetime'ed|Mutable|Strongly Typed|Aligned|Not Null|Forbids Aliasing|Forbids Arithmetic|
|-------------------|-----------|-------|--------------|-------|--------|----------------|------------------|
|`Box<T>`           |Owned      |Yes    |Yes           |Yes    |Yes     |Yes             |Yes               |
|`&'a mut T`        |Yes        |Yes    |Yes           |Yes    |Yes     |Yes             |Yes               |
|`&'a T`            |Yes        |No     |Yes           |Yes    |Yes     |No              |Yes               |
|`&'a UnsafeCell<T>`|Yes        |Maybe  |Yes           |Yes    |Yes     |Yes             |Yes               |
|`NonNull<T>`       |No         |Yes    |Yes           |No     |Yes     |No              |No                |
|`*const T`         |No         |No     |Yes           |No     |No      |No              |No                |
|`*mut T`           |No         |Yes    |Yes           |No     |No      |No              |No                |
|`*const ()`        |No         |No     |No            |No     |No      |No              |No                |
|`*mut ()`          |No         |Yes    |No            |No     |No      |No              |No                |

## Available in Nightly

|Pointer Type       |Lifetime'ed|Mutable|Strongly Typed|Aligned|Not Null|Forbids Aliasing|Forbids Arithmetic|
|-------------------|-----------|-------|--------------|-------|--------|----------------|------------------|
|`Unique<T>`        |Owned      |Yes    |Yes           |Yes    |Yes     |Yes             |Yes               |
|`Shared<T>`        |Owned*     |Yes    |Yes           |Yes    |Yes     |No              |Yes               |

## Available in `paddy_ptr`

|Pointer Type         |Lifetime'ed|Mutable|Strongly Typed|Aligned|Not Null|Forbids Aliasing|Forbids Arithmetic|
|---------------------|-----------|-------|--------------|-------|--------|----------------|------------------|
|`ConstNonNull<T>`    |No         |No     |Yes           |No     |Yes     |No              |Yes               |
|`ThinSlicePtr<'a, T>`|Yes        |No     |Yes           |Yes    |Yes     |Yes             |Yes               |
|`OwningPtr<'a>`      |Yes        |Yes    |No            |Maybe  |Yes     |Yes             |No                |
|`Ptr<'a>`            |Yes        |No     |No            |Maybe  |Yes     |No              |No                |
|`PtrMut<'a>`         |Yes        |Yes    |No            |Maybe  |Yes     |Yes             |No                |

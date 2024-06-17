 

# Label 

`#lable: <description>`

A : 若标签存在,那么\<description\> 是必要的

| label| | |
|--|--|--|
|untested| 需要测试的(但还未经过测试的;若你认为这个函数需要被测试,则标记它,测试后需删除标签,并且声明测试函数的位置)| |
|todo |未完成的| |
|wait |等待编写中的(往往占名时用)| |
|safety| 安全的(在使用unsafe代码后,若可以保证安全,则使用这个进行标记)(或在标签后描述 保证安全需要注意的事项,声明只有有限的约定的行为才能保证安全性)| |
|plan |声明未来的计划 |A |
|note |你需要特别注意的注释 | A |


## for fn

```rust
/// 标签永远紧贴在 fn 上,其他内容不可先于标签
/// #label : i am sb
/// #label2 #label3
fn f(){

}
```

# Documentation Specification


用于声明测试函数的位置
```md
# Test Fn
[`test_fn`](crate::tests::test_fn) : description
[`test_fn2`](crate::tests::test_fn2) : description2
```
---

例子,用于教你如何使用这个东西,且必须是可运行的
```md
# Examples
\`\`\`rust
assert_eq!(1,1)
\`\`\`

```

---

特殊的安全性要求(若只有一条,可使用标签(#safety)代替)

```md
# Safety

- 你不能做什么什么
- 你可以做什么什么
- 这是使用它的约定
- 这是使用它的前提
- 使用它后的注意事项
- ...
```

---
若文档过多,或标签过多,需使用这个来声明标签位置(根据需要决定)
```md
# Lable

#lable1 : descritpion
#lable2
#lable3
```

---

描述函数返回值
```md
@return : description

```
---
描述函数参数

```md
@`parameter` : description
```
---
描述字段(注意:是描述字段,若需在其他文档中指定这个字段,去掉@即可)
```md
@[`field`](crate::structure::field) : description
```




# Git Commit Specification

`<type>(<crate>:[<scope>;...];...) : <subject>`

## \<type\>
| type | description|
|--|--|
|feat  |新增 feature  |
|fix  | 修复 bug |
|docs  | 仅仅修改了文档，比如 README, CHANGELOG, CONTRIBUTE等等 |
|style  | 仅仅修改了空格、格式缩进、逗号等等，不改变代码逻辑 |
|refactor  | 代码重构，没有加新功能或者修复 bug |
|perf  |优化相关，比如提升性能、体验  |
|test  | 测试用例，包括单元测试、集成测试等 |
|chore  | 改变构建流程、或者增加依赖库、工具等 |
|revert  | 回滚到上一个版本 |
|chaos |过于混乱,跳过\<crate\>\<scope\>,直接写\<subject\> |
|release |发布一个版本,跳过\<scope\>; \<subject\> 为版本号 |
|include |`include: <type>,<type>,...` , 声明包含的type,其余内容写到描述中 |


## \<crate\>
声明影响的crate\
使用分号进行分割

例如: `(paddy_render:[<scope>];paddy:[<scope>])`


## \<scope\> (可忽略)
声明影响的范围(写重点就可以了) \
使用分号进行分割 \

\* : 表示只包含当前路径下的文件 \
\*\*: 表示包含路径下的文件 \
\#\<name\>/... : #render/shader 表示对render/shader这个功能的影响(一个模糊的范围)

例如:`[/src/*;/src/**;#abc]`

## \<subject\>
标题

`<symbol> <other>`

\<symbol\> : 用于强化标题内容(可忽略)

| symbol|description |
|--|--|
|Add| 添加|
|Update |更新 |
|Remove |移除 |
|Move |移动 |



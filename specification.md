 

# Label 

| label| |
|--|--|
|untested| 为经过测试的|
|todo |未完成的|
|wait |等待编写中的(往往占名时用)|
|safety| 安全的(在使用unsafe代码后,若可以保证安全,则使用这个进行标记)|
|plan |声明未来的计划 |


## for fn

```rust
/// 标签永远紧贴在 fn 上,其他内容不可先于标签
/// #label #label2
fn f(){

}
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



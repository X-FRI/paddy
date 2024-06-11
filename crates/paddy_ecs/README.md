
paddy的ESC架构系统


# 计划
还挺难的...
## 第一步(启动!!!)
先写给雏形运行吧... 毫无经验的我学习那些复杂的ECS(完全看不明白源码,太多东西了)

当前的实现方式是 Archetypes (aka "Dense ECS" or "Table based ECS")


## 第二步(增加特性)
第一步完成后,开始慢慢增加特性,研究其他库的源码,偷些喜欢的特性来用

这个库 [https://github.com/zakarumych/edict/tree/main] 似乎是一个 实验性的ECS库,有挺多特性的,也是Archetypes

## 第三步(性能优化与内存布局)
第二步完成后(确定了目前需要的特性后)
开始进行 性能优化 与 内存布局 (专为那些特性服务)


# my-spider

基于 [`spider`](https://github.com/spider-rs/spider) 的登录态爬虫项目，使用 **Bazel** 构建与运行，抓取结果写入 **SQLite** 数据库。

## 项目简介

本项目用于抓取需要登录状态才能访问的网站内容，并将结果持久化到本地 SQLite 数据库中，便于后续查询、分析与处理。

项目采用 Bazel 进行构建，同时通过 `[patch.crates-io]` 指向 fork 的 `spider` 分支，修复了 `spider` 在 Bazel 环境下因 `aws-lc-sys` 导致的构建失败问题。

## 特别说明

原始 `spider` 依赖链中，内部 `reqwest` 默认会走 `rustls` 路线，在 Bazel 环境下会触发 `aws-lc-sys` 构建失败。

为解决该问题，本项目通过 fork 的 `spider` 分支，将其内部 `reqwest` 调整为 `native-tls`，从而保证项目可以在 Bazel 下正常构建和运行。

## 环境准备

开始前请确认本地已经安装以下工具：

- Rust
- Cargo
- Bazel
- SQLite 客户端（可选，用于查看数据库）

## 项目配置

### 1. 配置环境变量

复制示例配置文件：

```bash
cp .env.example .env

然后根据实际情况修改 .env 中的配置项。

运行方式
1. 赋予脚本执行权限
chmod +x run.sh
2. 执行初始化脚本
./run.sh
3. 使用 Bazel 运行项目
bazel run //:my_spider

程序运行后，会抓取目标数据并写入 SQLite 数据库。

构建方式

如果只需要构建，不立即运行，可以执行：

bazel build //:my_spider

构建成功后，产物位于：

bazel-bin/my_spider
数据存储

抓取结果会写入 SQLite 数据库文件。

你可以使用 SQLite 客户端查看，例如：

sqlite3 data/your_database.db

请将 your_database.db 替换为实际生成的数据库文件名。

项目结构
.
├── BUILD.bazel
├── MODULE.bazel
├── Cargo.toml
├── Cargo.lock
├── README.md
├── run.sh
├── .env.example
├── src/
└── data/
常见问题
1. Bazel 构建失败

如果 Bazel 构建失败，请优先检查以下内容：

.env 是否已正确配置
fork 的 spider patch 是否已生效
Cargo.lock 是否已重新生成
本地 Bazel 缓存是否需要清理

可以尝试执行：

bazel clean --expunge
cargo generate-lockfile
bazel build //:my_spider --verbose_failures
2. 登录态失效

如果目标站点返回登录页而不是预期内容，请检查：

cookie / session 是否有效
账号是否过期或被退出登录
目标站点是否增加了验证码或额外验证流程
3. 数据库中没有结果

请确认：

程序是否成功运行
目标页面是否成功抓取
数据写入逻辑是否正常执行
数据库路径是否正确
开发说明

本项目当前已验证以下链路可正常工作：

Bazel 构建
Bazel 运行
spider 在 Bazel 环境下可用
登录态页面抓取
SQLite 数据落库
后续建议

建议在后续开发中补充以下内容：

登录态校验日志
cookie / session 失效自动检测
抓取结果去重
错误重试机制
更完整的数据库表结构说明
License

仅供学习与研究使用，请遵守目标网站的使用条款与相关法律法规。

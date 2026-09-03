# cprof

一个统一管理 Claude Code 和 Codex 配置 profile 的命令行工具。

## 安装

```bash
git clone https://github.com/haukzero/cprof.git
cd cprof
cargo install --path .
```

或

```bash
cargo install --git https://github.com/haukzero/cprof.git
```

## 命令

目标命令格式为 `cprof <target> <command>`，target 为 `claude` 或 `codex`。

| 命令 | 说明 |
|------|------|
| `cprof <target> dir` | 显示 profile 存储目录 |
| `cprof <target> list` | 列出所有 profile |
| `cprof <target> which` | 当前激活状态 |
| `cprof <target> num` | profile 总数 |
| `cprof <target> switch [name] [-f]` | 切换 profile |
| `cprof <target> create [name]` | 创建并编辑 profile |
| `cprof <target> edit [name] [--filename key]` | 编辑 profile |
| `cprof <target> remove [names...]` | 删除 profile |
| `cprof <target> clean [-f]` | 清空所有 profile |
| `cprof <target> where [name] [--filename key]` | profile 文件实际位置 |
| `cprof <target> pack [--save path]` | 打包指定 target |
| `cprof <target> unpack [--path path] [-f]` | 解包指定 target |
| `cprof pack [--save path]` | 一次打包所有 target |
| `cprof unpack [--path path] [-f]` | 一次解包所有 target |

除 `create` 外，省略 profile 名称会进入模糊搜索选择。`--filename` 使用资源逻辑名称：
Claude 为 `settings`，Codex 为 `config` 或 `auth`。
省略 target 的 `pack` 和 `unpack` 会一次处理所有 target，默认包文件为 `cprof.pkg`。

## 存储位置

真实配置文件集中在：

```text
~/.cprof/profiles/<target>/<profile>/<resource>
```

外部程序使用的固定路径由软链接指向当前 profile。

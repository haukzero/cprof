# cprof

一个统一管理 Claude Code 和 Codex 配置 profile 的命令行工具.

## 安装

```bash
cargo install --git https://github.com/haukzero/cprof.git cprof
```

或从本地源码安装:

```bash
git clone https://github.com/haukzero/cprof.git
cd cprof
cargo install --path ./cprof
```

## 基本用法

每个 target 对应一个工具, 每个 profile 对应它的一套配置. 内置 target 为 `claude` 和 `codex`, 也可以[添加外部 target](docs/extra-targets.md).

已有配置可以直接导入并激活:

```bash
cprof codex adopt personal
```

创建其他配置, 然后按需切换:

```bash
cprof codex create work    # 交互选择默认模板或已有 profile, 然后编辑
cprof codex list
cprof codex switch work
```

真实配置文件集中在 `~/.cprof/profiles/<target>/<profile>/<resource>`, 外部程序使用的固定路径由软链接指向当前 profile.

## 命令

命令分为 root command 和 target subcommand 两级. `cprof <target> <command>` 操作指定 target; `cprof pack`, `cprof unpack`, `cprof clean` 等 root command 操作多个 target.

| 用途 | 文档 |
|------|------|
| 查看 target, profile 和存储位置 | [查看配置](docs/commands.md#查看配置) |
| 创建, 导入, 编辑, 重命名和切换 profile | [管理 profile](docs/commands.md#管理-profile) |
| 指定编辑器和资源文件 | [编辑器与资源](docs/commands.md#编辑器与资源) |
| 删除 profile 或清空配置 | [删除与清理](docs/commands.md#删除与清理) |
| 选择打包内容, 导入或镜像同步配置 | [打包与解包](docs/commands.md#打包与解包) |
| 管理其他工具的配置 | [外部 target](docs/extra-targets.md) |

使用 `cprof --help`, `cprof <target> --help` 或具体命令的 `--help` 查看参数.

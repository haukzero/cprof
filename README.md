# cprof

一个统一管理 Claude Code 和 Codex 配置 profile 的命令行工具. 

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

命令分为 root command 和 target subcommand 两级.

### Root command

| 命令 | 说明 |
|------|------|
| `cprof pack [--save path] [--select [target]]...` | 打包所有或选中的 target |
| `cprof unpack [--path path] [-f]` | 一次解包所有 target |
| `cprof clean [-f] [--extra-toml]` | 清空所有 target 的 profile |
| `cprof edit-extra [--editor program] [--editor-arg arg]...` | 编辑外部 target 配置文件 |
| `cprof targets [--json]` | 列出所有 target 及其相关信息, `--json` 输出 JSON |

- Root command `pack` 默认一次打包所有 target; 重复传入 `--select target` 可只打包指定 target, 例如 `cprof pack --select claude --select codex`. 单独传入 `--select` 时会打开交互式多选. `unpack` 会处理包内所有 target, 默认包文件为 `cprof.pkg`.
- Root command `clean` 默认保留 `extra-target.toml`, 使用 `--extra-toml` 一并删除.
- Root command `edit-extra` 会创建(如果不存在)并打开 `~/.cprof/extra-target.toml`; 使用 `--editor` 可手动指定编辑器. 配置存在语法错误或 target 定义冲突时, 仍可使用此命令编辑修复.

### Target subcommand

目标命令格式为 `cprof <target> <command>`, 内置 target 为 `claude` 和 `codex`, 也可以在 `~/.cprof/extra-target.toml` 中[添加外部 target](#外部-target).

| 命令 | 说明 |
|------|------|
| `cprof <target> dir` | 显示 profile 存储目录 |
| `cprof <target> list` | 列出所有 profile |
| `cprof <target> which` | 当前激活状态 |
| `cprof <target> num` | profile 总数及 incomplete 数量 |
| `cprof <target> switch [name] [-f]` | 切换 profile |
| `cprof <target> create [name] [-c profile] [--editor program] [--editor-arg arg]...` | 创建并编辑 profile, 可从已有 profile 复制 |
| `cprof <target> edit [name] [--filename key] [--editor program] [--editor-arg arg]...` | 编辑 profile |
| `cprof <target> remove [names...] [-f]` | 删除 profile, 名称支持 `*` 和 `?` 通配符; `-f` 允许删除当前激活项 |
| `cprof <target> clean [-f]` | 清空所有 profile |
| `cprof <target> where [name] [--filename key]` | profile 文件实际位置 |
| `cprof <target> pack [--save path]` | 打包指定 target |
| `cprof <target> unpack [--path path] [-f]` | 解包指定 target |

- `create name` 默认从模板创建; 使用 `--copy-from profile` 或 `-c profile` 会先复制指定的已有 profile, 再进入编辑器. 省略 `create` 的名称时, 输入新名称后可在默认模板和已有 profile 间模糊选择来源. 
- 其他省略 profile 名称的命令会进入模糊搜索选择. `--filename` 使用资源逻辑名称:
    - Claude 为 `settings`
    - Codex 为 `config` 或 `auth`. 
- 编辑器按 `--editor`, `VISUAL`, `EDITOR`, 系统默认值的顺序选择. `--editor-arg` 可重复使用以传入参数; `VISUAL` 和 `EDITOR` 也支持带引号的"程序 + 参数"配置.
- 需要确认的操作在非交互环境中会报错退出; 使用对应命令的 `--force` 明确执行删除或覆盖.

### 外部 target

允许在 `~/.cprof/extra-target.toml` 中配置的定义的额外 target. 配置文件使用顶层表名定义 target, 格式参考 [`examples/extra-target.toml`](examples/extra-target.toml). 命令中的 target 默认使用表名; 声明 `id` 后改用该 id: 

```toml
[example]
id = "example-id" # 可选, 默认与表名相同

[[example.resources]]
filename = "settings.conf"
active_path = ".config/example/settings.conf"
# 或使用绝对路径(二者同时配置时必须指向同一路径)
# absolute_active_path = "/opt/example/settings.conf"
# key, template, required 均可省略; key 默认与 filename 相同, template 默认为空文件
```

外部 target 可直接使用上述全部 subcommand, 资源内容不会进行格式校验. cprof 在命令需要读取 target 配置时加载并校验配置, target id 与已有 target, 命令冲突时会报告明确错误. 根帮助会加载配置以展示外部 target, 因此也需要配置有效.

target id 和 filename 必须是非空的单个路径组件. active_path 相对于 HOME, 不允许绝对路径, Windows 前缀或 `..`, 父目录软链接也不能越出 HOME. 如需指定 HOME 之外的位置, 使用 absolute_active_path. 两个字段同时配置时, 指向同一路径会警告, 指向不同路径会报错. 同一 target 内的 filename 和实际激活路径均不能重复.

打包外部 target 时会同时记录 `extra-target.toml` 中的 target 定义. 解包时会与本地配置合并; 新的 target 和资源会自动加入, 定义冲突时交互选择保留本地或使用包内定义. 使用 `-f` 可直接采用包内定义.

本地配置, 包内定义及合并结果使用相同的路径校验, `-f` 不会跳过校验. package manifest 只接受当前版本, 其他版本会报错.

## 存储位置

真实配置文件集中在: 

```text
~/.cprof/profiles/<target>/<profile>/<resource>
```

外部程序使用的固定路径由软链接指向当前 profile. 

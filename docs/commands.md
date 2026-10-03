# 命令

[返回 README](../README.md)

下文的 `<target>` 可以是 `claude`, `codex` 或已配置的[外部 target](extra-targets.md). `[]` 表示可选参数, `...` 表示可以重复传入.

## 查看配置

| 命令 | 说明 |
|------|------|
| `cprof targets [--json]` | 列出所有 target 及其相关信息, `--json` 输出 JSON |
| `cprof <target> list` | 列出所有 profile |
| `cprof <target> which` | 当前激活状态 |
| `cprof <target> num` | profile 总数及 incomplete 数量 |
| `cprof <target> dir` | 显示 profile 存储目录 |
| `cprof <target> where [name] [--filename key]` | 显示 profile 的资源文件路径 |

## 管理 profile

| 命令 | 说明 |
|------|------|
| `cprof <target> create [name] [-c profile]` | 创建并编辑 profile, 可从已有 profile 复制 |
| `cprof <target> adopt [name]` | 将当前未托管配置导入为新 profile 并激活 |
| `cprof <target> edit [name] [--filename key]` | 编辑 profile |
| `cprof <target> rename [old_name] [new_name]` | 重命名 profile |
| `cprof <target> switch [name] [-f]` | 切换 profile |

- `create` 省略名称时交互输入新名称; 使用 `--copy-from profile` 或 `-c profile` 指定来源, 否则在默认模板和已有 profile 间模糊选择.
- `adopt` 支持普通文件和外部软链接; 省略名称时交互输入新名称.
- `edit`, `switch`, `where`, `remove` 省略名称时进入模糊搜索选择. `rename` 可以同时传入旧名称和新名称, 或不传名称进入交互流程.
- `switch -f` 允许替换目标路径上未托管的文件或外部软链接; 如需保留现有配置, 先使用 `adopt` 导入.

## 编辑器与资源

`create`, `edit`, `edit-extra` 均支持 `--editor program` 和可重复传入的 `--editor-arg arg`. `--editor-arg` 需要同时指定 `--editor`.

编辑器按 `--editor`, `VISUAL`, `EDITOR`, 系统默认值的顺序选择. `VISUAL` 和 `EDITOR` 支持带引号的"程序 + 参数"配置.

`edit` 和 `where` 的 `--filename` 使用资源逻辑名称, Claude 为 `settings`, Codex 为 `config` 或 `auth`:

```bash
cprof codex edit work --filename auth
cprof codex edit work --editor code --editor-arg --wait
```

## 删除与清理

| 命令 | 说明 |
|------|------|
| `cprof <target> remove [names...] [-f]` | 删除指定 profile, 名称支持 `*` 和 `?` 通配符 |
| `cprof <target> clean [-f]` | 清空当前 target 的 profile 和托管软链接 |
| `cprof clean [-f] [--extra-toml]` | 清空所有 target, `--extra-toml` 一并删除外部 target 配置 |

`remove` 删除当前激活项时需要确认, `-f` 允许直接删除. `clean` 默认需要确认, `-f` 跳过确认; root command `clean` 默认保留 `extra-target.toml`.

通配符建议加引号, 如 `cprof codex remove "work-*"`. 需要交互输入或确认时, 非交互环境会报错退出; 可显式传入名称, 并在支持的命令中使用 `--force` (`-f`) 确认操作.

## 打包与解包

| 命令 | 说明 |
|------|------|
| `cprof pack [--save path] [--select [target[/profile]]]... [--ascii]` | 打包所有或选中的 target/profile |
| `cprof <target> pack [--save path] [--select [profile]]... [--ascii]` | 打包当前 target 的所有或选中的 profile |
| `cprof unpack [--path path] [-f] [--dry-run] [--mirror]` | 一次解包所有 target |
| `cprof <target> unpack [--path path] [-f] [--dry-run] [--mirror]` | 解包指定 target |

包文件默认使用当前目录的 `cprof.pkg`, `--save` 和 `--path` 分别指定打包输出和解包输入.

### 选择打包内容

不传 `--select` 时打包当前作用域的全部 profile. root command 中 `--select target` 选择整个 target, `--select target/profile` 选择单个 profile; target subcommand 中直接传 profile 名称. 可以重复传入, 重复项自动去重:

```bash
cprof pack --select claude --select codex/work
cprof claude pack --select work --select personal
```

两个 `pack` 命令单独传入 `--select` 都会打开树状选择界面. 方向键或 `j/k` 移动, 空格选择/取消当前节点及其全部子节点, 也可单独取消某个 profile; `a` 全选/取消全选, Enter 确认, `q` 或 Esc 取消.

树形默认使用 Unicode; 显示异常时可加 `--ascii`, 如 `cprof pack --select --ascii`, 或设置环境变量 `CPROF_ASCII=1`.

### 导入配置

`unpack` 默认合并配置, 保留本地额外的 profile; 同名内容不同时询问是否覆盖, `--force` (`-f`) 直接覆盖. `--dry-run` (`-n`) 只预览变更.

`--mirror` (`-m`) 将 profile, 激活状态及外部 target 定义同步为包内状态, 会删除作用域内包中不存在的配置. root command 作用于所有 target, target subcommand 只作用于指定 target:

```bash
cprof codex unpack --path backup.pkg --mirror --dry-run
```

## 外部 target 管理

`cprof edit-extra` 创建(如果不存在)并打开 `~/.cprof/extra-target.toml`. 配置有语法错误或 target 定义冲突时, 仍可使用此命令编辑修复; 也支持[指定编辑器](#编辑器与资源).

配置格式见[外部 target](extra-targets.md).

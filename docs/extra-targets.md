# 外部 target

[返回 README](../README.md) | [命令](commands.md)

除 `claude` 和 `codex` 外, 可以在 `~/.cprof/extra-target.toml` 中添加其他工具. 使用 `cprof edit-extra` 创建或编辑该文件, 外部 target 可直接使用全部 target subcommand.

## 配置格式

使用顶层表名定义 target. 命令中的 target 默认使用表名, 声明 `id` 后改用该 id:

```toml
[example]
id = "example-id" # 可选, 默认与表名相同

[[example.resources]]
filename = "settings.conf"
active_path = ".config/example/settings.conf"
```

配置完成后, 即可使用 `cprof example-id create work`, `cprof example-id switch work` 等命令. 多资源示例见 [`extra-target.toml`](../cprof/examples/extra-target.toml).

每个 `[[example.resources]]` 定义一个资源文件:

| 字段 | 说明 |
|------|------|
| `filename` | profile 内的文件名, 必填 |
| `active_path` | 外部程序使用的路径, 相对于 HOME |
| `absolute_active_path` | 外部程序使用的绝对路径, 可用于 HOME 之外的位置 |
| `key` | 供 `--filename` 使用的逻辑名称, 默认与 `filename` 相同 |
| `template` | 新建 profile 的默认内容, 默认为空文件 |
| `required` | 该资源是否必需, 默认为 `true` |

`active_path` 和 `absolute_active_path` 至少填写一个. 外部 target 的资源内容不会进行格式校验.

## 名称与路径

- target 表名, id 和 filename 必须是非空的单个路径组件. target id 不能与已有 target 或 root command 重名.
- `active_path` 不允许绝对路径, Windows 前缀或 `..`, 父目录软链接也不能越出 HOME. 如需使用 HOME 之外的路径, 填写 `absolute_active_path`.
- 两个路径字段同时配置时必须指向同一路径, 此时会提示重复配置; 指向不同路径会报错.
- 同一 target 内的 key, filename 和实际激活路径均不能重复.

根帮助会加载配置以展示外部 target. 配置无效时, 可使用 `cprof edit-extra` 修复.

## 随包迁移

打包外部 target 时会同时记录其定义. 默认解包会与本地配置合并, 新的 target 和资源自动加入; 定义冲突时交互选择保留本地或使用包内定义, `-f` 直接采用包内定义.

`--mirror` 会按包内定义同步, 行为见[导入配置](commands.md#导入配置). `-f` 不会跳过名称和路径校验.

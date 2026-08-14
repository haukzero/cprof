# cprof

一个简单的 Claude Code 配置文件切换工具. 

## 安装

```bash
cargo install --path .
```

## 命令

| 命令 | 说明 |
|------|------|
| `cprof list` | 列出所有配置 |
| `cprof which` | 当前用的哪个 |
| `cprof num` | 配置总数 |
| `cprof switch [name]` | 切换配置 |
| `cprof create [name]` | 新建配置 |
| `cprof edit [name]` | 编辑配置 |
| `cprof remove [names...]` | 删除配置(支持多个以空格间隔) |
| `cprof clean [-f]` | 清空所有配置 |
| `cprof pack` | 打包以快速同步到其他机器 |
| `cprof unpack` | 解包以快速同步其他机器传来的配置 |

不带参数会弹交互选择, 支持模糊搜索. 

## 存储位置

```
~/.claude-profiles/<name>/settings.json
```

`~/.claude/settings.json` 软链接指向当前激活的配置. 

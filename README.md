# mem - CPU 和内存占用工具

一个用于控制 CPU 和内存资源占用的命令行工具。

## 安装

从 [Releases](https://github.com/sharkAndshark/mem/releases) 下载对应平台的二进制文件：

**Linux:**
```bash
wget https://github.com/sharkAndshark/mem/releases/download/v0.1.0/mem-linux-x86_64.tar.gz
tar -xzf mem-linux-x86_64.tar.gz
chmod +x mem
```

**Windows:**
```powershell
# 下载 mem-windows-x86_64.exe.zip 并解压
```

## 使用方法

### 基本用法

```bash
# 占用 50% CPU 和 50% 内存
mem -c 50% -m 50%

# 占用 100% CPU (1核) 和 2GB 内存
mem -c 100 -m 2G

# 运行 60 秒后自动退出
mem -c 50% -m 1G -d 60
```

### 参数说明

| 参数 | 说明 | 示例 |
|------|------|------|
| `-c, --cpu` | CPU 占用 | `-c 50%` 或 `-c 100` |
| `-m, --memory` | 内存占用 | `-m 50%` 或 `-m 2G` |
| `-d, --duration` | 运行时长(秒) | `-d 60` |

### CPU 参数

```bash
-c 50%    # 占用总 CPU 的 50% (8核系统 = 4核)
-c 100    # 占用 100% CPU (1个核心满载)
-c 200    # 占用 200% CPU (2个核心满载)
```

### 内存参数

```bash
-m 50%    # 占用总内存的 50% (16GB系统 = 8GB)
-m 2G     # 占用 2GB 内存
-m 512M   # 占用 512MB 内存
```

## 使用场景

### 场景 1: 服务器资源指标

```bash
# 保持 CPU 使用率 50%，内存使用率 60%
mem -c 50% -m 60%
```

### 场景 2: 压力测试

```bash
# 模拟高负载：4核 CPU + 4GB 内存，持续 5 分钟
mem -c 400 -m 4G -d 300
```

### 场景 3: 后台运行

```bash
# Linux 后台运行
nohup mem -c 50% -m 50% &

# 使用 systemd 管理 (推荐)
```

## 退出方式

- `Ctrl-C` - 手动退出
- `-d` 参数 - 定时退出

## 特性

- ✅ 支持百分比和固定值两种模式
- ✅ 多核 CPU 支持
- ✅ 自动内存调整
- ✅ 内存不足时自动释放
- ✅ 跨平台支持 (Linux/Windows)

## 注意事项

1. 百分比模式会根据系统资源动态调整
2. 内存紧张时会自动释放部分占用
3. 程序需要足够权限读取系统信息

## 构建

```bash
cargo build --release
```

## License

MIT

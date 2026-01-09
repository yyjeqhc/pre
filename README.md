# Pre - Cargo.toml Linux平台清理工具

一个用于清理 `Cargo.toml` 文件的 Rust 库和命令行工具，可删除非 Linux 平台相关的配置，包括 target 配置、平台特定依赖和 features。

## 功能

- **智能依赖识别**：使用 `cargo_toml` 解析 Cargo.toml，自动识别所有 target-specific 依赖
- **删除非 Linux 平台配置**：移除 Windows、macOS、Android、iOS、WASM、BSD 等平台的 `[target.'cfg(...)']` 配置
- **保留 Linux/Unix 配置**：智能保留 `unix` 和 `linux` 相关配置
- **Features 清理**：自动清理引用了被删除依赖的 features
- **保留格式和注释**：使用 `toml_edit` 进行编辑，保留原始格式、注释和空白
- **双重模式**：可以作为库使用或作为命令行工具

## 工作原理

本工具结合了两个强大的 crate：

1. **cargo_toml 0.20**：用于解析和理解 Cargo.toml 的语义结构
   - 自动识别所有 target-specific 依赖
   - 支持 workspace 继承
   - 提供完整的依赖关系视图

2. **toml_edit 0.22**：用于保留格式的 TOML 编辑
   - 保留注释、空白和格式
   - 精确删除配置
   - 保持文档可读性

### 处理流程

```
输入 Cargo.toml
    ↓
[cargo_toml 解析]
    ├─ 识别所有 target-specific 依赖
    ├─ 判断平台配置是否为非 Linux
    └─ 收集需要删除的依赖列表
    ↓
[toml_edit 编辑]
    ├─ 删除非 Linux 的 target 配置
    ├─ 清理引用了被删除依赖的 features
    └─ 保留格式和注释
    ↓
输出清理后的 Cargo.toml
```

## 安装

### 作为库使用

在你的 `Cargo.toml` 中添加：

```toml
[dependencies]
pre = { path = "/path/to/pre" }
```

### 作为命令行工具

```bash
cargo build --release
# 二进制文件在 target/release/pre
```

## 使用方法

### 命令行

```bash
# 覆盖原文件
pre input.toml

# 输出到新文件
pre input.toml output.toml
```

### 作为库使用

```rust
use pre::{process_toml_file, process_toml_string_with_path};

fn main() -> Result<(), pre::ProcessError> {
    // 方式1: 直接处理文件（推荐）
    // cargo_toml 会自动解析并识别所有平台特定依赖
    process_toml_file("Cargo.toml", "Cargo.toml.clean")?;
    
    // 方式2: 处理字符串（需要提供路径用于解析）
    let toml_content = std::fs::read_to_string("Cargo.toml")?;
    let cleaned = process_toml_string_with_path(&toml_content, "Cargo.toml")?;
    println!("{}", cleaned);
    
    Ok(())
}
```

### 查看解析结果

使用内置的分析工具查看 `cargo_toml` 识别的依赖：

```bash
cargo run --example analyze path/to/Cargo.toml
```

这将显示：
- 所有常规依赖
- 所有平台特定依赖（按 target 分组）
- 所有 features 定义

## 实际示例

### 处理前（glutin Cargo.toml）

```toml
[dependencies]
bitflags = "2.2.1"
libloading = { version = "0.8.0", optional = true }
once_cell = "1.13"
raw-window-handle = "0.6"

# 4 个 target 配置，包含 Windows、macOS、Android
[target.'cfg(any(target_os = "linux", ...))'.dependencies]
glutin_egl_sys = { version = "0.7.0", path = "../glutin_egl_sys" }
glutin_glx_sys = { version = "0.6.0", path = "../glutin_glx_sys" }
# ...

[target.'cfg(any(target_os = "macos"))'.dependencies]
cgl = "0.3.2"
objc2 = { version = "0.5.2", features = ["apple"] }
# ...

[target.'cfg(windows)'.dependencies]
glutin_wgl_sys = { version = "0.6.0", path = "../glutin_wgl_sys" }
windows-sys = { version = "0.59", features = ["Win32_Graphics_Gdi"] }

[features]
default = ["egl", "glx", "x11", "wayland", "wgl"]
wgl = ["glutin_wgl_sys", "windows-sys"]  # Windows feature
```

### 处理后

```toml
[dependencies]
bitflags = "2.2.1"
libloading = { version = "0.8.0", optional = true }
once_cell = "1.13"
raw-window-handle = "0.6"

# 只保留 Linux/BSD 配置
[target.'cfg(any(target_os = "linux", ...))'.dependencies]
glutin_egl_sys = { version = "0.7.0", path = "../glutin_egl_sys" }
glutin_glx_sys = { version = "0.6.0", path = "../glutin_glx_sys" }
# ...

[features]
default = ["egl", "glx", "x11", "wayland"]  # wgl 被移除
# wgl feature 被删除
```

****cargo_toml 0.20** - 解析 Cargo.toml 并理解其语义结构
  - 自动识别 target-specific 依赖
  - 支持 workspace 继承
  - 提供完整的依赖关系图
- **toml_edit 0.22** - 保留格式的 TOML 编辑
  - 保留注释、空白和缩进
  - 精确的配置删除
  - 保持文档可读性

## 技术亮点

1. **智能解析**：使用 `cargo_toml` 理解 Cargo.toml 的语义，而不是简单的字符串匹配
2. **格式保留**：使用 `toml_edit` 保留原始格式，适合提交到版本控制
3. **完整性**：自动识别并清理所有相关的依赖引用（包括 features）
4. **安全性**：保留 `not()` 等复杂条件配置，避免误删
5. **可扩展**：库设计允许集成到其他工具中
- ✅ 保留了 1 个 Linux/BSD target 配置
- ✅ 从 default feature 中移除了 wgl
- ✅ 删除了整个 wgl feature（因为它依赖 Windows 专属的 crate）
- ✅ 保留了所有格式和注释

### Target 配置

删除所有非 Linux/Unix 平台的 target 配置，例如：

```toml
# 会被删除
[target.'cfg(windows)'.dependencies]
windows-sys = "0.52"

[target.'cfg(target_os = "macos")'.dependencies]
cocoa = "0.25"

# 会保留
[target.'cfg(unix)'.dependencies]
libc = "0.2"

[target.'cfg(target_os = "linux")'.dependencies]
nix = "0.27"
```

### 平台特定依赖

自动识别并记录以下平台特定依赖：

- **Windows**: `windows-sys`, `winapi`, `anstyle-wincon` 等
- **macOS/iOS**: `cocoa`, `core-foundation`, `objc` 等
- **Android**: `android-activity`, `ndk` 等
- **WASM**: `wasm-bindgen`, `js-sys`, `web-sys` 等
- **BSD/其他**: `freebsd`, `openbsd`, `redox_syscall` 等

### Features

删除或清理与非 Linux 平台相关的 features：

```toml
[features]
# 会被删除
wincon = ["dep:anstyle-wincon"]
wasm = ["dep:wasm-bindgen"]

# 会被清理（移除 wincon 引用）
default = ["auto", "wincon"]  # -> default = ["auto"]

# 会保留
auto = ["dep:anstyle-query"]
```

## 依赖

- `toml_edit` 0.22 - 用于保留格式的 TOML 编辑
- `cargo_toml` 0.20 - 用于解析和理解 Cargo.toml 结构（预留用于未来增强功能）

## 许可证

MIT OR Apache-2.0

use cargo_toml::Manifest;
use std::collections::HashSet;
use std::fs;
use toml_edit::{Array, DocumentMut, Item, Value};

#[derive(Debug)]
pub enum ProcessError {
    IoError(std::io::Error),
    ParseError(toml_edit::TomlError),
    CargoTomlError(cargo_toml::Error),
}

impl std::fmt::Display for ProcessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProcessError::IoError(e) => write!(f, "IO错误: {}", e),
            ProcessError::ParseError(e) => write!(f, "TOML解析错误: {}", e),
            ProcessError::CargoTomlError(e) => write!(f, "Cargo.toml解析错误: {}", e),
        }
    }
}

impl std::error::Error for ProcessError {}

impl From<std::io::Error> for ProcessError {
    fn from(err: std::io::Error) -> Self {
        ProcessError::IoError(err)
    }
}

impl From<toml_edit::TomlError> for ProcessError {
    fn from(err: toml_edit::TomlError) -> Self {
        ProcessError::ParseError(err)
    }
}

impl From<cargo_toml::Error> for ProcessError {
    fn from(err: cargo_toml::Error) -> Self {
        ProcessError::CargoTomlError(err)
    }
}

/// 处理TOML文件，删除非Linux平台相关的配置
///
/// 使用 cargo_toml 解析 Cargo.toml 来智能识别平台特定依赖
pub fn process_toml_file(input_path: &str, output_path: &str) -> Result<(), ProcessError> {
    // 读取文件内容
    let content = fs::read_to_string(input_path)?;

    // 使用 cargo_toml 从字节解析（避免工作区查找问题）
    let manifest = Manifest::from_slice(content.as_bytes())?;

    // 使用 toml_edit 编辑保留格式
    let mut doc = content.parse::<DocumentMut>()?;

    process_toml_doc(&mut doc, &manifest);

    fs::write(output_path, doc.to_string())?;
    Ok(())
}

/// 处理TOML文档字符串（需要提供路径用于解析继承）
pub fn process_toml_string_with_path(
    content: &str,
    manifest_path: &str,
) -> Result<String, ProcessError> {
    let manifest = Manifest::from_path(manifest_path)?;
    let mut doc = content.parse::<DocumentMut>()?;
    process_toml_doc(&mut doc, &manifest);
    Ok(doc.to_string())
}

/// 处理TOML文档字符串（不解析依赖，仅使用硬编码列表）
pub fn process_toml_string(content: &str) -> Result<String, ProcessError> {
    let mut doc = content.parse::<DocumentMut>()?;
    // 创建一个 manifest 用于基本处理
    let manifest = Manifest::from_slice(content.as_bytes())?;
    process_toml_doc(&mut doc, &manifest);
    Ok(doc.to_string())
}

/// 处理TOML文档
///
/// 核心处理函数，使用 cargo_toml 解析的 Manifest 来智能识别需要删除的配置
pub fn process_toml_doc(doc: &mut DocumentMut, manifest: &Manifest) {
    // 1. 使用 cargo_toml 解析并删除非Linux的target配置
    let removed_deps = remove_non_linux_targets(doc, manifest);

    // 2. 清理features中对已删除依赖的引用
    clean_features(doc, &removed_deps);
}

/// 使用 cargo_toml 提取平台特定依赖，然后用 toml_edit 删除配置
fn remove_non_linux_targets(doc: &mut DocumentMut, manifest: &Manifest) -> HashSet<String> {
    let mut removed_deps = HashSet::new();
    let mut kept_deps = HashSet::new();

    // 使用 cargo_toml 提取所有 target-specific 依赖
    for (target_spec, target_dep) in &manifest.target {
        // 检查这个 target 是否应该被删除
        if should_remove_target_config(target_spec) {
            // 收集此 target 下的所有依赖（候选删除列表）
            for (dep_name, _) in &target_dep.dependencies {
                removed_deps.insert(dep_name.clone());
            }
            for (dep_name, _) in &target_dep.dev_dependencies {
                removed_deps.insert(dep_name.clone());
            }
            for (dep_name, _) in &target_dep.build_dependencies {
                removed_deps.insert(dep_name.clone());
            }
        } else {
            // 保留的 target，记录其依赖
            for (dep_name, _) in &target_dep.dependencies {
                kept_deps.insert(dep_name.clone());
            }
            for (dep_name, _) in &target_dep.dev_dependencies {
                kept_deps.insert(dep_name.clone());
            }
            for (dep_name, _) in &target_dep.build_dependencies {
                kept_deps.insert(dep_name.clone());
            }
        }
    }

    // 从 removed_deps 中移除那些在 kept_deps 中也存在的依赖
    // 只有完全被删除的依赖才应该被标记
    removed_deps.retain(|dep| !kept_deps.contains(dep));

    // 添加已知的平台特定依赖（仅那些不在kept_deps中的）
    for dep in get_known_platform_deps() {
        if !kept_deps.contains(&dep) {
            removed_deps.insert(dep);
        }
    }

    // 使用 toml_edit 删除配置（保留格式和注释）

    // 处理 [target] 表下的子项
    if let Some(target_table) = doc.get_mut("target").and_then(|t| t.as_table_mut()) {
        let keys_to_remove: Vec<String> = target_table
            .iter()
            .filter_map(|(key, _)| {
                if should_remove_target_config(key) {
                    Some(key.to_string())
                } else {
                    None
                }
            })
            .collect();

        for key in &keys_to_remove {
            target_table.remove(key);
        }

        // 如果 target 表为空，删除整个表
        if target_table.is_empty() {
            doc.remove("target");
        }
    }

    // 处理直接的 [target.'cfg(...)'.xxx] 表
    let keys_to_remove: Vec<String> = doc
        .iter()
        .filter_map(|(key, _)| {
            if key.starts_with("target.") {
                if let Some(cfg_part) = extract_cfg_from_target_key(key) {
                    if should_remove_target_config(&cfg_part) {
                        return Some(key.to_string());
                    }
                }
            }
            None
        })
        .collect();

    for key in keys_to_remove {
        doc.remove(&key);
    }

    removed_deps
}

fn extract_cfg_from_target_key(key: &str) -> Option<String> {
    if let Some(rest) = key.strip_prefix("target.") {
        // 匹配引号形式: 'cfg(...)' 或 "cfg(...)"
        if rest.starts_with('\'') || rest.starts_with('"') {
            let quote = rest.chars().next().unwrap();
            if let Some(end_pos) = rest[1..].find(quote) {
                return Some(rest[1..end_pos + 1].to_string());
            }
        }
        // 匹配无引号形式: cfg(...).xxx
        if let Some(dot_pos) = rest.find('.') {
            return Some(rest[..dot_pos].to_string());
        }
        return Some(rest.to_string());
    }
    None
}

fn should_remove_target_config(key: &str) -> bool {
    // 如果配置包含 "not("，通常是排除某些特定平台的通用配置，保留
    if key.contains("not(") {
        return false;
    }

    // 保留unix和linux
    if key.contains("unix") || key.contains("linux") {
        return false;
    }

    // 定义需要删除的操作系统列表（非Linux系统）
    const NON_LINUX_OS: &[&str] = &[
        // Windows
        "windows",
        // macOS/Apple
        "macos",
        "darwin",
        "ios",
        "tvos",
        "watchos",
        "visionos",
        // Android
        "android",
        // WASM
        "wasm",
        "emscripten",
        // Embedded/Specialized
        "hermit",
        "wasi",
        "redox",
        "uefi",    // UEFI firmware
        "vxworks", // VxWorks RTOS
        "horizon", // Nintendo 3DS
        "vita",    // PlayStation Vita
        "nto",     // QNX Neutrino
        "aix",     // IBM AIX
        // BSD variants
        "freebsd",
        "openbsd",
        "netbsd",
        "dragonfly",
        "dragonflybsd",
        // Other Unix-like
        "solaris",
        "illumos",
        "haiku",
        "hurd",   // GNU Hurd
        "cygwin", // Cygwin on Windows
        // Other
        "fuchsia", // Google Fuchsia
        "unknown",
        "none",
    ];

    // 定义需要删除的平台家族
    const NON_LINUX_FAMILY: &[&str] = &["wasm"];

    // 定义需要删除的vendor
    const NON_LINUX_VENDOR: &[&str] = &["apple"];

    // 定义需要删除的 target_env（Windows/非Linux特有的编译环境）
    const NON_LINUX_ENV: &[&str] = &[
        "msvc",   // Microsoft Visual C++, Windows only
        "mingw",  // MinGW, Windows only
        "cygwin", // Cygwin, Windows only
        "sgx",    // Intel SGX, not Linux-specific
    ];

    // 定义需要删除的target triple模式
    const NON_LINUX_TRIPLE_PATTERNS: &[&str] = &[
        "-msvc",
        "wasm32-",
        "wasm64-",
        "x86_64-pc-windows",
        "i686-pc-windows",
        "x86_64-apple-",
        "aarch64-apple-",
    ];

    // 检查 cfg(os_name) 简写形式（等价于 target_os = "os_name"）
    for os in NON_LINUX_OS {
        // cfg(windows), cfg( windows ), cfg(windows )
        if key.contains(&format!("cfg({})", os))
            || key.contains(&format!("cfg( {})", os))
            || key.contains(&format!("cfg({} )", os))
            || key.contains(&format!("cfg( {} )", os))
        {
            return true;
        }
    }

    // 检查 target_os = "os_name" 形式
    let has_non_linux_os =
        key.contains("target_os") && NON_LINUX_OS.iter().any(|os| key.contains(os));

    // 检查 target_family = "family" 形式
    let has_non_linux_family =
        key.contains("target_family") && NON_LINUX_FAMILY.iter().any(|family| key.contains(family));

    // 检查 target_vendor = "vendor" 形式
    let has_non_linux_vendor =
        key.contains("target_vendor") && NON_LINUX_VENDOR.iter().any(|vendor| key.contains(vendor));

    // 检查 target_env = "env" 形式（Windows特有的编译环境）
    let has_non_linux_env =
        key.contains("target_env") && NON_LINUX_ENV.iter().any(|env| key.contains(env));

    // 检查是否是特定的target triple
    let has_non_linux_triple = NON_LINUX_TRIPLE_PATTERNS
        .iter()
        .any(|pattern| key.contains(pattern));

    // 只有明确指定了非Linux平台的才删除
    // 只指定 target_arch 的配置保留（适用于所有OS）
    // gnu/musl 等 Linux 特有的 target_env 会被保留
    has_non_linux_os
        || has_non_linux_family
        || has_non_linux_vendor
        || has_non_linux_env
        || has_non_linux_triple
}

/// 获取已知的平台特定依赖列表
/// 这个列表用于补充 cargo_toml 解析的结果
fn get_known_platform_deps() -> HashSet<String> {
    let platform_specific = vec![
        // Windows
        "windows-sys",
        "winapi",
        "anstyle-wincon",
        "windows",
        "windows-core",
        "windows-targets",
        "windows-implement",
        "windows-interface",
        "windows-result",
        "winreg",
        "wio",
        "winapi-util",
        "ntapi",
        // macOS/iOS
        "cocoa",
        "core-foundation",
        "core-foundation-sys",
        "core-graphics",
        "core-graphics-types",
        "objc",
        "objc2",
        "objc2-foundation",
        "objc2-app-kit",
        "objc2-ui-kit",
        "objc2-core-image",
        "objc-foundation",
        "fsevent-sys",
        "fsevents-sys",
        "block",
        "block2",
        "dispatch",
        "icrate",
        "metal",
        "core-video",
        "mach",
        "mach2",
        // Android
        "android-activity",
        "android-properties",
        "android_log-sys",
        "android_logger",
        "ndk",
        "ndk-sys",
        "ndk-context",
        "ndk-glue",
        "jni",
        "jni-sys",
        // WASM
        "wasm-bindgen",
        "wasm-bindgen-futures",
        "wasm-bindgen-macro",
        "js-sys",
        "web-sys",
        "web-time",
        "console_error_panic_hook",
        "tracing-web",
        "gloo",
        "gloo-utils",
        "gloo-timers",
        // BSD和其他Unix系统
        "orbclient",
        "redox_syscall",
        "redox_users",
        "wasip2",
        "wasi",
        "r-efi",
        "r-efi-alloc",
        // 其他平台特定
        "hermit-abi",
        "sgx_tstd",
    ];

    platform_specific.iter().map(|s| s.to_string()).collect()
}

fn clean_features(doc: &mut DocumentMut, removed_deps: &HashSet<String>) {
    if let Some(features) = doc.get_mut("features").and_then(|f| f.as_table_like_mut()) {
        let mut features_to_update: Vec<(String, Vec<String>)> = Vec::new();

        // 遍历每个feature
        for (name, value) in features.iter() {
            let name_str = name.to_string();

            if let Some(array) = value.as_array() {
                let mut new_items = Vec::new();
                let mut modified = false;

                for item in array.iter() {
                    if let Some(s) = item.as_str() {
                        if !should_remove_feature_item(s, removed_deps) {
                            new_items.push(s.to_string());
                        } else {
                            modified = true;
                        }
                    }
                }

                if modified {
                    // 保留 feature，即使变成空数组
                    // 例如 wincon = [] 或 std = []
                    features_to_update.push((name_str, new_items));
                }
            }
        }

        // 更新修改后的features（包括空的）
        for (name, items) in features_to_update {
            let mut new_array = Array::new();
            for item in items {
                new_array.push(item);
            }
            features.insert(&name, Item::Value(Value::Array(new_array)));
        }
    }
}

fn should_remove_feature_item(item: &str, removed_deps: &HashSet<String>) -> bool {
    let lower = item.to_lowercase();

    // 检查是否是 dep:xxx 形式
    if let Some(dep_name) = item.strip_prefix("dep:") {
        return removed_deps.contains(&dep_name.to_lowercase()) || removed_deps.contains(dep_name);
    }

    // 检查是否是 crate/feature 或 crate?/feature 形式（可选依赖）
    if let Some(crate_part) = item.split('/').next() {
        // 去除可选依赖标记 '?'
        let crate_name = crate_part.trim_end_matches('?');
        if removed_deps.contains(&crate_name.to_lowercase()) || removed_deps.contains(crate_name) {
            return true;
        }
    }

    // 如果不包含 'dep:' 或 '/'，说明是对其他 feature 的引用，不删除
    // 例如 default = ["auto", "wincon"]，这里的 "wincon" 是引用 feature，不是依赖
    if !item.contains("dep:") && !item.contains('/') {
        return false;
    }

    // 其他情况检查是否包含非Linux平台的关键词
    if lower.contains("windows") || lower.contains("win32") {
        return true;
    }

    // 检查是否引用了被删除的依赖包
    removed_deps.contains(&lower) || removed_deps.contains(item)
}

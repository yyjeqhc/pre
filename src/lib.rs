use std::collections::HashSet;
use std::fs;
use toml_edit::{DocumentMut, Item, Value, Array};

#[derive(Debug)]
pub enum ProcessError {
    IoError(std::io::Error),
    ParseError(toml_edit::TomlError),
}

impl std::fmt::Display for ProcessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProcessError::IoError(e) => write!(f, "IO错误: {}", e),
            ProcessError::ParseError(e) => write!(f, "TOML解析错误: {}", e),
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

/// 处理TOML文件，删除非Linux平台相关的配置
pub fn process_toml_file(input_path: &str, output_path: &str) -> Result<(), ProcessError> {
    let content = fs::read_to_string(input_path)?;
    let mut doc = content.parse::<DocumentMut>()?;
    
    process_toml_doc(&mut doc);
    
    fs::write(output_path, doc.to_string())?;
    Ok(())
}

/// 处理TOML文档字符串
pub fn process_toml_string(content: &str) -> Result<String, ProcessError> {
    let mut doc = content.parse::<DocumentMut>()?;
    process_toml_doc(&mut doc);
    Ok(doc.to_string())
}

/// 处理TOML文档
pub fn process_toml_doc(doc: &mut DocumentMut) {
    // 1. 删除非Linux的target配置
    remove_non_linux_targets(doc);
    
    // 2. 收集所有被删除的依赖名称
    let removed_deps = collect_non_linux_dependencies();
    
    // 3. 清理features中对已删除依赖的引用
    clean_features(doc, &removed_deps);
}

fn remove_non_linux_targets(doc: &mut DocumentMut) {
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
}

fn extract_cfg_from_target_key(key: &str) -> Option<String> {
    if let Some(rest) = key.strip_prefix("target.") {
        // 匹配引号形式: 'cfg(...)' 或 "cfg(...)"
        if rest.starts_with('\'') || rest.starts_with('"') {
            let quote = rest.chars().next().unwrap();
            if let Some(end_pos) = rest[1..].find(quote) {
                return Some(rest[1..end_pos+1].to_string());
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
    
    // 检查是否包含平台关键词
    let has_platform_keyword = 
        key.contains("target_os") ||
        key.contains("target_family") ||
        key.contains("target_arch") ||
        key.contains("target_env") ||
        key.contains("windows") ||
        key.contains("macos") ||
        key.contains("darwin") ||
        key.contains("android") ||
        key.contains("ios") ||
        key.contains("wasm") ||
        key.contains("hermit") ||
        key.contains("wasi") ||
        key.contains("redox") ||
        key.contains("freebsd") ||
        key.contains("openbsd") ||
        key.contains("netbsd") ||
        key.contains("dragonfly") ||
        key.contains("solaris") ||
        key.contains("illumos");
    
    // 如果没有平台关键词，保留（可能是编译标志）
    if !has_platform_keyword {
        return false;
    }
    
    // 有平台关键词，但不是unix/linux，则删除
    true
}

fn collect_non_linux_dependencies() -> HashSet<String> {
    let platform_specific = vec![
        // Windows
        "windows-sys",
        "winapi",
        "anstyle-wincon",
        "windows",
        "unicode-segmentation",
        // macOS/iOS
        "cocoa",
        "core-foundation",
        "core-graphics",
        "objc",
        "objc2",
        "objc2-foundation",
        "objc2-app-kit",
        "objc2-ui-kit",
        "fsevent-sys",
        "block2",
        // Android
        "android-activity",
        "ndk",
        // WASM
        "wasm-bindgen",
        "wasm-bindgen-futures",
        "js-sys",
        "web-sys",
        "web-time",
        "pin-project",
        "atomic-waker",
        "concurrent-queue",
        "console_error_panic_hook",
        "tracing-web",
        // BSD和其他Unix系统
        "orbclient",
        "redox_syscall",
        "wasip2",
        "r-efi",
    ];
    
    platform_specific.iter().map(|s| s.to_string()).collect()
}

fn clean_features(doc: &mut DocumentMut, removed_deps: &HashSet<String>) {
    if let Some(features) = doc.get_mut("features").and_then(|f| f.as_table_like_mut()) {
        let mut features_to_update: Vec<(String, Vec<String>)> = Vec::new();
        let mut features_to_remove: Vec<String> = Vec::new();
        
        // 遍历每个feature
        for (name, value) in features.iter() {
            let name_str = name.to_string();
            
            // 检查feature名称本身是否应该被删除
            if should_remove_feature_name(&name_str) {
                features_to_remove.push(name_str);
                continue;
            }
            
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
                    features_to_update.push((name_str, new_items));
                }
            }
        }
        
        // 删除Windows相关的feature
        for name in features_to_remove {
            features.remove(&name);
        }
        
        // 更新修改后的features
        for (name, items) in features_to_update {
            let mut new_array = Array::new();
            for item in items {
                new_array.push(item);
            }
            features.insert(&name, Item::Value(Value::Array(new_array)));
        }
    }
}

fn should_remove_feature_name(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower == "wincon" || 
    lower == "wgl" ||
    lower.contains("windows") ||
    lower.starts_with("win32") ||
    lower.starts_with("macos") ||
    lower.starts_with("android") ||
    lower.starts_with("ios") ||
    lower.contains("wasm") ||
    lower.starts_with("bsd")
}

fn should_remove_feature_item(item: &str, removed_deps: &HashSet<String>) -> bool {
    let lower = item.to_lowercase();
    
    // 删除对非Linux平台特性的引用
    if lower == "wincon" || 
       lower == "wgl" ||
       lower.starts_with("macos") ||
       lower.starts_with("android") ||
       lower.starts_with("ios") ||
       lower.contains("wasm") {
        return true;
    }
    
    // 检查是否是 dep:xxx 形式
    if let Some(dep_name) = item.strip_prefix("dep:") {
        return removed_deps.contains(&dep_name.to_lowercase());
    }
    
    // 检查是否包含非Linux平台的关键词
    if lower.contains("windows") || lower.contains("win32") {
        return true;
    }
    
    // 检查是否引用了被删除的依赖包
    for dep in removed_deps {
        if lower.contains(&dep.to_lowercase()) {
            return true;
        }
    }
    
    false
}

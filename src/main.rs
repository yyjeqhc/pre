use std::fs;
use std::collections::HashSet;
use toml_edit::{DocumentMut, Item, Value, Array};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    
    if args.len() < 2 {
        eprintln!("用法: {} <input.toml> [output.toml]", args[0]);
        eprintln!("  如果未指定output.toml，将覆盖原文件");
        std::process::exit(1);
    }
    
    let input_path = &args[1];
    let output_path = if args.len() >= 3 {
        &args[2]
    } else {
        input_path
    };
    
    match process_toml_file(input_path, output_path) {
        Ok(()) => println!("✓ 处理完成: {}", output_path),
        Err(e) => {
            eprintln!("✗ 错误: {}", e);
            std::process::exit(1);
        }
    }
}

fn process_toml_file(input_path: &str, output_path: &str) -> Result<(), String> {
    // 读取文件
    let content = fs::read_to_string(input_path)
        .map_err(|e| format!("无法读取文件 {}: {}", input_path, e))?;
    
    // 解析TOML文档用于编辑
    let mut doc = content.parse::<DocumentMut>()
        .map_err(|e| format!("无法解析TOML文件: {}", e))?;
    
    // 处理文档
    process_toml_doc(&mut doc);
    
    // 写入文件
    fs::write(output_path, doc.to_string())
        .map_err(|e| format!("无法写入文件 {}: {}", output_path, e))?;
    
    Ok(())
}

fn process_toml_doc(doc: &mut DocumentMut) {
    // 1. 删除非Linux的target配置
    remove_non_linux_targets(doc);
    
    // 2. 收集所有被删除的依赖名称
    let removed_deps = collect_non_linux_dependencies(doc);
    
    // 3. 清理features中对已删除依赖的引用
    clean_features(doc, &removed_deps);
}

fn remove_non_linux_targets(doc: &mut DocumentMut) {
    // target section的结构是 [target] 下有多个子表
    if let Some(target_table) = doc.get_mut("target").and_then(|t| t.as_table_mut()) {
        let keys_to_remove: Vec<String> = target_table
            .iter()
            .filter_map(|(key, _)| {
                let key_str = key;
                if should_remove_target_config(key_str) {
                    Some(key_str.to_string())
                } else {
                    None
                }
            })
            .collect();
        
        for key in &keys_to_remove {
            target_table.remove(key);
        }
    }
}

fn should_remove_target_config(key: &str) -> bool {
    // key 格式类似: 'cfg(target_os = "wasi")' 或 "cfg(windows)"
    
    // 保留unix和linux相关的
    if key.contains("unix") || 
       key.contains("linux") ||
       key.contains("hermit") ||
       key.contains("wasi") {
        return false;
    }
    
    // 删除windows, macos, ios, android相关的
    key.contains("windows") ||
    key.contains("macos") ||
    key.contains("darwin") ||
    key.contains("android") ||
    key.contains("ios")
}

fn collect_non_linux_dependencies(_doc: &DocumentMut) -> HashSet<String> {
    let mut removed = HashSet::new();
    
    // 常见的Windows/macOS/Android/iOS专属依赖
    let platform_specific = vec![
        "windows-sys",
        "winapi",
        "anstyle-wincon",
        "windows",
        "cocoa",
        "core-foundation",
        "core-graphics",
        "objc",
        "objc2",
        "objc2-foundation",
        "objc2-app-kit",
        "objc2-ui-kit",
        "fsevent-sys",
        "android-activity",
        "ndk",
        "block2",
    ];
    
    for dep in platform_specific {
        removed.insert(dep.to_string());
    }
    
    removed
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
                        // 检查是否引用了被删除的依赖或feature
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
    // 删除Windows/macOS/Android/iOS相关的feature名称
    lower == "wincon" || 
    lower == "wgl" ||
    lower.contains("windows") ||
    lower.starts_with("win32") ||
    lower.starts_with("macos_") ||
    lower.starts_with("macos-") ||
    lower.starts_with("android") ||
    lower.starts_with("ios")
}

fn should_remove_feature_item(item: &str, removed_deps: &HashSet<String>) -> bool {
    let lower = item.to_lowercase();
    
    // 删除对Windows/macOS/Android/iOS特性的引用
    if lower == "wincon" || 
       lower == "wgl" ||
       lower.starts_with("macos_") ||
       lower.starts_with("macos-") ||
       lower.starts_with("android") ||
       lower.starts_with("ios") {
        return true;
    }
    
    // 检查是否是 dep:xxx 形式
    if let Some(dep_name) = item.strip_prefix("dep:") {
        return removed_deps.contains(&dep_name.to_lowercase());
    }
    
    // 检查是否包含Windows相关的关键词
    if lower.contains("windows") ||
       lower.contains("win32") {
        return true;
    }
    
    // 检查是否引用了被删除的依赖包（如android-activity, ndk等）
    for dep in removed_deps {
        if lower.contains(&dep.to_lowercase()) {
            return true;
        }
    }
    
    false
}

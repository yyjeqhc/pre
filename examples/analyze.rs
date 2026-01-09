use cargo_toml::Manifest;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        eprintln!("用法: {} <input.toml>", args[0]);
        std::process::exit(1);
    }

    let input_path = &args[1];

    // 先使用 cargo_toml 分析依赖
    match Manifest::from_path(input_path) {
        Ok(manifest) => {
            println!("=== 使用 cargo_toml 解析的依赖信息 ===\n");

            // 显示常规依赖
            if !manifest.dependencies.is_empty() {
                println!("常规依赖 ({} 个):", manifest.dependencies.len());
                for (name, _) in &manifest.dependencies {
                    println!("  - {}", name);
                }
                println!();
            }

            // 显示 target-specific 依赖
            if !manifest.target.is_empty() {
                println!("平台特定依赖 ({} 个target配置):", manifest.target.len());
                for (target, deps) in &manifest.target {
                    let total_deps = deps.dependencies.len()
                        + deps.dev_dependencies.len()
                        + deps.build_dependencies.len();

                    if total_deps > 0 {
                        println!("\n  Target: {}", target);

                        if !deps.dependencies.is_empty() {
                            println!("    dependencies:");
                            for (name, _) in &deps.dependencies {
                                println!("      - {}", name);
                            }
                        }

                        if !deps.dev_dependencies.is_empty() {
                            println!("    dev-dependencies:");
                            for (name, _) in &deps.dev_dependencies {
                                println!("      - {}", name);
                            }
                        }

                        if !deps.build_dependencies.is_empty() {
                            println!("    build-dependencies:");
                            for (name, _) in &deps.build_dependencies {
                                println!("      - {}", name);
                            }
                        }
                    }
                }
                println!();
            }

            // 显示 features
            if !manifest.features.is_empty() {
                println!("Features ({} 个):", manifest.features.len());
                for (name, items) in &manifest.features {
                    println!("  {} = {:?}", name, items);
                }
                println!();
            }
        }
        Err(e) => {
            eprintln!("✗ 无法解析 Cargo.toml: {}", e);
            std::process::exit(1);
        }
    }
}

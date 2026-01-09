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

    match pre::process_toml_file(input_path, output_path) {
        Ok(()) => println!("✓ 处理完成: {}", output_path),
        Err(e) => {
            eprintln!("✗ 错误: {}", e);
            std::process::exit(1);
        }
    }
}

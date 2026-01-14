#!/bin/bash

# 确保输出目录存在
mkdir -p out

# 遍历 test 目录下所有的 .toml 文件
for input_file in test/*.toml; do
    # 获取文件名（不包括路径）
    filename=$(basename "$input_file")
    
    # 使用 cargo run 处理每个 .toml 文件，并将结果输出到 out/ 目录中，文件名不变
    cargo run -- "$input_file" "now/$filename"
done

#!/bin/bash

# Git 提交准备脚本
# 用于清理临时文件、验证代码质量并提交到 Git 仓库

echo "🚀 Bilibili缓存转换器 - Git 提交准备脚本"
echo "=========================================="
echo ""

# 1. 清理临时文件
echo "🧹 清理临时文件..."
rm -f test_path_*.exe
rm -f test_path_*.pdb
rm -f test_path_*.rs
rm -f *.problems.txt
echo "✅ 清理完成"
echo ""

# 2. 检查 Node.js 依赖
echo "📦 检查 Node.js 依赖..."
if [ ! -d "node_modules" ]; then
    echo "⚠️  node_modules 不存在，正在安装依赖..."
    npm install
else
    echo "✅ node_modules 已安装"
fi
echo ""

# 3. 运行测试
echo "🧪 运行测试..."
npm run test:run
if [ $? -eq 0 ]; then
    echo "✅ 测试通过"
else
    echo "❌ 测试失败，请修复后重新提交"
    exit 1
fi
echo ""

# 4. 代码检查
echo "🔍 代码检查..."
npm run lint
if [ $? -eq 0 ]; then
    echo "✅ ESLint 检查通过"
else
    echo "⚠️  ESLint 发现一些问题，请手动修复"
fi
echo ""

# 5. 构建前端
echo "🏗️  构建前端..."
npm run build
if [ $? -eq 0 ]; then
    echo "✅ 前端构建成功"
else
    echo "❌ 前端构建失败"
    exit 1
fi
echo ""

# 6. Rust 代码检查
echo "🦀 检查 Rust 代码..."
cd src-tauri
cargo check
if [ $? -eq 0 ]; then
    echo "✅ Rust 代码检查通过"
else
    echo "❌ Rust 代码检查失败"
    exit 1
fi
cd ..
echo ""

# 7. Git 状态检查
echo "📊 Git 状态:"
git status
echo ""

# 8. 提示用户确认
echo "=========================================="
echo "✅ 所有检查已完成!"
echo ""
echo "下一步操作:"
echo "1. 查看变更：git diff"
echo "2. 添加文件：git add ."
echo "3. 提交代码：git commit -m 'feat: 初始版本发布'"
echo "4. 推送代码：git push origin main"
echo ""
echo "建议的提交信息:"
echo "----------------------------------------"
echo "feat: 初始版本发布 - Bilibili缓存转换器 v1.0.0"
echo ""
echo "🎉 项目特性:"
echo "- 智能扫描 Bilibili缓存文件"
echo "- 支持视频格式转换 (MP4/MKV/AVI)"
echo "- 支持音频格式转换 (MP3/AAC/FLAC)"
echo "- GPU 硬件加速 (NVIDIA/AMD/Intel)"
echo "- 并发转换支持 (1/2/4/8)"
echo "- 实时进度监控和性能指标"
echo "- 暂停/恢复/取消功能"
echo "- 虚拟滚动优化 (>100 文件)"
echo "- 智能文件命名和目录优化"
echo "- 完善的安全防护"
echo ""
echo "🛠️ 技术栈:"
echo "- 前端：React 18 + TypeScript + Vite + TailwindCSS"
echo "- 后端：Rust + Tauri + Tokio + FFmpeg"
echo "- UI: shadcn/ui + Radix UI"
echo ""
echo "📚 文档:"
echo "- README.md - 项目说明"
echo "- API_DOCUMENTATION.md - 接口文档"
echo "- TECHNICAL_ARCHITECTURE.md - 架构设计"
echo "- DEPLOYMENT_GUIDE.md - 部署指南"
echo "- CHANGELOG.md - 变更日志"
echo ""
echo "Closes #1"
echo "----------------------------------------"
echo ""

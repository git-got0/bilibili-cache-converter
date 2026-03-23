# Git 提交准备脚本 (PowerShell 版本)
# 用于清理临时文件、验证代码质量并提交到 Git 仓库

Write-Host "🚀 Bilibili缓存转换器 - Git 提交准备脚本" -ForegroundColor Cyan
Write-Host "==========================================" -ForegroundColor Cyan
Write-Host ""

# 1. 清理临时文件
Write-Host "🧹 清理临时文件..." -ForegroundColor Yellow
Get-ChildItem -Path . -Filter "test_path_*.exe" -ErrorAction SilentlyContinue | Remove-Item -Force
Get-ChildItem -Path . -Filter "test_path_*.pdb" -ErrorAction SilentlyContinue | Remove-Item -Force
Get-ChildItem -Path . -Filter "test_path_*.rs" -ErrorAction SilentlyContinue | Remove-Item -Force
Get-ChildItem -Path . -Filter "*.problems.txt" -ErrorAction SilentlyContinue | Remove-Item -Force
Write-Host "✅ 清理完成" -ForegroundColor Green
Write-Host ""

# 2. 检查 Node.js 依赖
Write-Host "📦 检查 Node.js 依赖..." -ForegroundColor Yellow
if (-not (Test-Path "node_modules")) {
    Write-Host "⚠️  node_modules 不存在，正在安装依赖..." -ForegroundColor Yellow
    npm install
} else {
    Write-Host "✅ node_modules 已安装" -ForegroundColor Green
}
Write-Host ""

# 3. 运行测试
Write-Host "🧪 运行测试..." -ForegroundColor Yellow
npm run test:run
if ($LASTEXITCODE -eq 0) {
    Write-Host "✅ 测试通过" -ForegroundColor Green
} else {
    Write-Host "❌ 测试失败，请修复后重新提交" -ForegroundColor Red
    exit 1
}
Write-Host ""

# 4. 代码检查
Write-Host "🔍 代码检查..." -ForegroundColor Yellow
npm run lint
if ($LASTEXITCODE -eq 0) {
    Write-Host "✅ ESLint 检查通过" -ForegroundColor Green
} else {
    Write-Host "⚠️  ESLint 发现一些问题，请手动修复" -ForegroundColor Yellow
}
Write-Host ""

# 5. 构建前端
Write-Host "🏗️  构建前端..." -ForegroundColor Yellow
npm run build
if ($LASTEXITCODE -eq 0) {
    Write-Host "✅ 前端构建成功" -ForegroundColor Green
} else {
    Write-Host "❌ 前端构建失败" -ForegroundColor Red
    exit 1
}
Write-Host ""

# 6. Rust 代码检查
Write-Host "🦀 检查 Rust 代码..." -ForegroundColor Yellow
Set-Location src-tauri
cargo check
if ($LASTEXITCODE -eq 0) {
    Write-Host "✅ Rust 代码检查通过" -ForegroundColor Green
} else {
    Write-Host "❌ Rust 代码检查失败" -ForegroundColor Red
    exit 1
}
Set-Location ..
Write-Host ""

# 7. Git 状态检查
Write-Host "📊 Git 状态:" -ForegroundColor Yellow
git status
Write-Host ""

# 8. 提示用户确认
Write-Host "==========================================" -ForegroundColor Cyan
Write-Host "✅ 所有检查已完成!" -ForegroundColor Green
Write-Host ""
Write-Host "下一步操作:" -ForegroundColor Yellow
Write-Host "1. 查看变更：git diff"
Write-Host "2. 添加文件：git add ."
Write-Host "3. 提交代码：git commit -m 'feat: 初始版本发布'"
Write-Host "4. 推送代码：git push origin main"
Write-Host ""
Write-Host "建议的提交信息:" -ForegroundColor Yellow
Write-Host "----------------------------------------" -ForegroundColor Gray
Write-Host "feat: 初始版本发布 - Bilibili缓存转换器 v1.0.0" -ForegroundColor White
Write-Host ""
Write-Host "🎉 项目特性:" -ForegroundColor Green
Write-Host "- 智能扫描 Bilibili缓存文件"
Write-Host "- 支持视频格式转换 (MP4/MKV/AVI)"
Write-Host "- 支持音频格式转换 (MP3/AAC/FLAC)"
Write-Host "- GPU 硬件加速 (NVIDIA/AMD/Intel)"
Write-Host "- 并发转换支持 (1/2/4/8)"
Write-Host "- 实时进度监控和性能指标"
Write-Host "- 暂停/恢复/取消功能"
Write-Host "- 虚拟滚动优化 (>100 文件)"
Write-Host "- 智能文件命名和目录优化"
Write-Host "- 完善的安全防护"
Write-Host ""
Write-Host "🛠️ 技术栈:" -ForegroundColor Blue
Write-Host "- 前端：React 18 + TypeScript + Vite + TailwindCSS"
Write-Host "- 后端：Rust + Tauri + Tokio + FFmpeg"
Write-Host "- UI: shadcn/ui + Radix UI"
Write-Host ""
Write-Host "📚 文档:" -ForegroundColor Magenta
Write-Host "- README.md - 项目说明"
Write-Host "- API_DOCUMENTATION.md - 接口文档"
Write-Host "- TECHNICAL_ARCHITECTURE.md - 架构设计"
Write-Host "- DEPLOYMENT_GUIDE.md - 部署指南"
Write-Host "- CHANGELOG.md - 变更日志"
Write-Host ""
Write-Host "Closes #1" -ForegroundColor Gray
Write-Host "----------------------------------------" -ForegroundColor Gray
Write-Host ""

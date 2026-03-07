# Bilibili缓存转换器 - 部署与维护手册

## 目录
- [环境准备](#环境准备)
- [开发环境搭建](#开发环境搭建)
- [生产构建](#生产构建)
- [发布流程](#发布流程)
- [日常维护](#日常维护)
- [故障排查](#故障排查)
- [版本更新](#版本更新)

---

## 环境准备

### 系统要求

#### 开发环境
- **操作系统**: Windows 10/11 (64位)
- **Node.js**: 18.x 或更高版本
- **npm**: 9.x 或更高版本
- **Rust**: 1.77 或更高版本
- **Cargo**: 随Rust一起安装
- **Git**: 2.x 或更高版本 (用于版本控制)
- **FFmpeg**: 4.x 或更高版本 (运行时依赖)

#### 生产环境
- **操作系统**: Windows 10/11 (64位)
- **FFmpeg**: 4.x 或更高版本 (必须添加到系统PATH)
- **VC++ Redistributable**: 2015-2022 (Tauri依赖)

---

## 开发环境搭建

### 1. 安装 Node.js 和 npm

#### Windows 安装
1. 访问 https://nodejs.org/
2. 下载 LTS 版本安装程序
3. 运行安装程序,按提示完成安装
4. 验证安装:
```powershell
node -v
npm -v
```

#### 配置 npm 镜像(可选)
```powershell
npm config set registry https://registry.npmmirror.com
```

### 2. 安装 Rust

#### Windows 安装
```powershell
# 下载并运行安装程序
Invoke-WebRequest -Uri https://win.rustup.rs -OutFile rustup-init.exe
.\rustup-init.exe -y

# 刷新环境变量
$env:Path = [System.Environment]::GetEnvironmentVariable("Path","Machine") + ";" + [System.Environment]::GetEnvironmentVariable("Path","User")

# 验证安装
rustc --version
cargo --version
```

#### Rust 工具链配置
```powershell
# 安装稳定版工具链
rustup default stable

# 添加目标平台(如需要)
rustup target add x86_64-pc-windows-msvc
```

### 3. 安装 FFmpeg

#### 方式一: 使用包管理器(推荐)

**Chocolatey**
```powershell
# 如果没有 Chocolatey,先安装
Set-ExecutionPolicy Bypass -Scope Process -Force; [System.Net.ServicePointManager]::SecurityProtocol = [System.Net.ServicePointManager]::SecurityProtocol -bor 3072; iex ((New-Object System.Net.WebClient).DownloadString('https://community.chocolatey.org/install.ps1'))

# 安装 FFmpeg
choco install ffmpeg -y
```

**Scoop**
```powershell
# 安装 Scoop
Set-ExecutionPolicy RemoteSigned -Scope CurrentUser
irm get.scoop.sh | iex

# 安装 FFmpeg
scoop install ffmpeg
```

#### 方式二: 手动安装

1. 访问 https://ffmpeg.org/download.html#build-windows
2. 下载 FFmpeg 共享库版本(Shared)
3. 解压到 `C:\ffmpeg` (或其他路径)
4. 将 `C:\ffmpeg\bin` 添加到系统 PATH
5. 重启命令行窗口
6. 验证安装:
```powershell
ffmpeg -version
```

### 4. 克隆项目代码

```bash
# 使用 Git 克隆
git clone <repository-url>
cd bilibili-converter

# 或直接解压项目压缩包
```

### 5. 安装项目依赖

```bash
# 安装 Node.js 依赖
npm install

# 验证安装
npm list --depth=0
```

### 6. 验证开发环境

```bash
# 启动开发服务器
npm run tauri:dev

# 如果一切正常,应该会自动打开应用程序窗口
```

---

## 生产构建

### 1. 前端构建

```bash
# 构建前端资源
npm run build

# 输出目录: dist/
```

**构建产物**:
```
dist/
├── index.html
├── assets/
│   ├── index-xxxxxx.css
│   └── index-xxxxxx.js
```

### 2. 完整应用构建

```bash
# 构建完整的 Tauri 应用
npm run tauri:build

# 构建过程包括:
# 1. 前端构建 (npm run build)
# 2. Rust 编译 (cargo build --release)
# 3. 打包生成安装包
```

### 3. 构建产物

#### 可执行文件
```
src-tauri/target/release/
└── bilibili-converter.exe        # 主可执行文件
```

#### 安装包
```
src-tauri/target/release/bundle/nsis/
└── Bilibili缓存转换器_1.0.0_x64-setup.exe    # NSIS 安装程序
```

### 4. 构建优化

#### 减小体积
```toml
# src-tauri/Cargo.toml

[profile.release]
opt-level = "z"        # 优化体积
lto = true             # 链接时优化
codegen-units = 1      # 更好的优化
strip = true           # 去除调试符号
```

#### 加快构建
```powershell
# 使用缓存
set CARGO_INCREMENTAL=1

# 使用 sccache(可选)
cargo install sccache
set RUSTC_WRAPPER=sccache
```

---

## 发布流程

### 1. 版本号管理

#### 更新版本号
```bash
# 更新 package.json
npm version patch   # 1.0.0 -> 1.0.1
npm version minor   # 1.0.0 -> 1.1.0
npm version major   # 1.0.0 -> 2.0.0
```

#### 手动更新版本号
```json
// package.json
{
  "version": "1.0.1"
}

// src-tauri/Cargo.toml
[package]
version = "1.0.1"
```

#### 更新 tauri.conf.json
```json
{
  "package": {
    "productName": "Bilibili缓存转换器",
    "version": "1.0.1"
  }
}
```

### 2. 代码审查清单

发布前检查:
- [ ] 所有功能正常工作
- [ ] 无编译错误和警告
- [ ] 通过 ESLint 检查
- [ ] TypeScript 类型检查通过
- [ ] 更新了 CHANGELOG.md
- [ ] 更新了文档
- [ ] 测试了不同场景
- [ ] 性能测试通过

### 3. 发布前测试

#### 功能测试
```bash
# 测试文件扫描
# 测试格式转换(视频/音频)
# 测试取消功能
# 测试设置功能
# 测试窗口调整
```

#### 兼容性测试
- Windows 10 (1903及以上)
- Windows 11
- 不同分辨率
- 不同FFmpeg版本

### 4. 创建发布包

```bash
# 构建发布版本
npm run tauri:build

# 打包发布文件
# 1. bilibili-converter.exe
# 2. Bilibili缓存转换器_1.0.1_x64-setup.exe
# 3. README.md
# 4. 用户手册(可选)
```

#### 压缩发布包
```powershell
# 创建发布目录
mkdir release-v1.0.1

# 复制文件
copy src-tauri\target\release\bilibili-converter.exe release-v1.0.1\
copy src-tauri\target\release\bundle\nsis\Bilibili缓存转换器_1.0.1_x64-setup.exe release-v1.0.1\
copy README.md release-v1.0.1\

# 压缩
Compress-Archive -Path release-v1.0.1 -DestinationPath bilibili-converter-v1.0.1.zip
```

### 5. 发布说明模板

```markdown
## Bilibili缓存转换器 v1.0.1 发布说明

### 新增功能
- [描述新增功能]

### 改进
- [描述改进内容]

### 修复
- [修复的问题]

### 系统要求
- Windows 10/11 (64位)
- FFmpeg 4.x 或更高版本

### 下载
- 安装包: [链接]
- 便携版: [链接]
- 源代码: [链接]
```

---

## 日常维护

### 1. 依赖更新

#### 检查过期依赖
```bash
# 检查 Node.js 依赖
npm outdated

# 检查 Rust 依赖
cd src-tauri
cargo outdated  # 需要先安装 cargo-outdated
```

#### 更新依赖
```bash
# 更新 Node.js 依赖
npm update

# 更新特定依赖
npm install package-name@latest

# 更新 Rust 依赖
cd src-tauri
cargo update
```

#### 更新 Tauri CLI
```bash
# 更新 Tauri CLI
cargo install tauri-cli --version "^2.0.0" --force
```

### 2. 代码质量

#### 运行 Lint
```bash
# ESLint 检查
npm run lint

# 修复 Lint 错误
npm run lint -- --fix
```

#### 代码格式化
```bash
# Prettier 格式化
npx prettier --write "src/**/*.{ts,tsx,js,jsx}"

# Rust 代码格式化
cd src-tauri
cargo fmt
```

#### Rust 代码检查
```bash
cd src-tauri
cargo clippy

# 修复 Clippy 建议
cargo clippy --fix
```

### 3. 日志管理

#### 查看日志
```bash
# Windows 日志位置
%APPDATA%\Bilibili缓存转换器\logs\
```

#### 日志级别调整
```rust
// src-tauri/src/lib.rs
tauri_plugin_log::Builder::default()
    .level(log::LevelFilter::Debug)  // 改为 Debug 获取详细日志
    .build(),
```

### 4. 性能监控

#### 监控指标
- 转换速度(MB/s)
- CPU 使用率
- 内存使用量
- 磁盘 I/O

#### 性能测试
```bash
# 使用不同并发数测试
# 1. 并发数=1
# 2. 并发数=4
# 3. 并发数=8

# 记录每次转换的时间和资源使用
```

### 5. 数据备份

#### 配置备份
```powershell
# 备份用户设置
Copy-Item "$env:APPDATA\Bilibili缓存转换er\settings.json" -Destination "backup\"
```

#### 源代码备份
```bash
# 使用 Git
git push origin main

# 或创建压缩包
tar -czf backup-$(date +%Y%m%d).tar.gz src/ src-tauri/
```

---

## 故障排查

### 1. 常见问题

#### 问题1: FFmpeg 未找到

**症状**:
```
Error: FFmpeg not found in PATH
```

**解决方案**:
```powershell
# 1. 检查 FFmpeg 是否安装
ffmpeg -version

# 2. 如果未安装,安装 FFmpeg
choco install ffmpeg -y

# 3. 手动添加到 PATH
# 控制面板 -> 系统 -> 高级系统设置 -> 环境变量
# 添加: C:\ffmpeg\bin

# 4. 重启应用程序
```

#### 问题2: 转换失败,无错误信息

**症状**: 转换进度卡住,无错误提示

**解决方案**:
```bash
# 1. 检查日志文件
%APPDATA%\Bilibili缓存转换器\logs\

# 2. 手动测试 FFmpeg
ffmpeg -i input.blv -c:v libx264 -c:a aac output.mp4

# 3. 检查源文件是否损坏
ffmpeg -v error -i input.blv -f null -

# 4. 降低并发数重新尝试
```

#### 问题3: 应用启动失败

**症状**: 双击exe文件无响应

**解决方案**:
```powershell
# 1. 以管理员身份运行

# 2. 检查 VC++ 运行库
# 下载并安装: https://aka.ms/vs/17/release/vc_redist.x64.exe

# 3. 检查防火墙/杀毒软件拦截

# 4. 查看事件查看器
eventvwr.msc
```

#### 问题4: 构建错误

**症状**: npm run tauri:build 失败

**解决方案**:
```bash
# 1. 清理缓存
npm cache clean --force
cd src-tauri
cargo clean

# 2. 重新安装依赖
rm -rf node_modules package-lock.json
npm install

# 3. 更新 Rust
rustup update

# 4. 检查环境变量
echo %PATH%
```

#### 问题5: 窗口显示异常

**症状**: 界面元素错位或被截断

**解决方案**:
```json
// src-tauri/tauri.conf.json
{
  "windows": [{
    "title": "Bilibili缓存转换器",
    "width": 900,
    "height": 700,
    "minWidth": 600,
    "minHeight": 500
  }]
}
```

### 2. 调试技巧

#### 前端调试
```bash
# 1. 启动开发模式
npm run tauri:dev

# 2. 打开开发者工具
# 在窗口中右键 -> 检查元素

# 3. 查看控制台输出
# Console 标签页

# 4. 查看网络请求
# Network 标签页
```

#### 后端调试
```rust
// 使用 println! 调试
println!("Debug: {:?}", variable);

// 使用 log 宏
log::info!("Info: {}", message);
log::error!("Error: {}", error);

// 使用 dbg! 宏(开发模式)
dbg!(&variable);
```

#### 日志级别配置
```rust
// 开发环境 - Debug 级别
tauri_plugin_log::Builder::default()
    .level(log::LevelFilter::Debug)
    .build(),

// 生产环境 - Info 级别
taurai_plugin_log::Builder::default()
    .level(log::LevelFilter::Info)
    .build(),
```

---

## 版本更新

### 1. 主版本更新 (Major)

**触发条件**:
- 架构重大改变
- 不兼容的API变更
- 依赖框架大版本升级

**更新步骤**:
1. 创建新分支
2. 修改版本号 (1.0.0 -> 2.0.0)
3. 实施重大变更
4. 更新所有文档
5. 完整测试
6. 发布新版本

### 2. 次版本更新 (Minor)

**触发条件**:
- 新增功能
- 功能性改进
- 向后兼容的API变更

**更新步骤**:
1. 创建功能分支
2. 修改版本号 (1.0.0 -> 1.1.0)
3. 实现新功能
4. 更新文档
5. 测试
6. 发布新版本

### 3. 修订版更新 (Patch)

**触发条件**:
- Bug修复
- 小改进
- 文档更新

**更新步骤**:
1. 创建修复分支
2. 修改版本号 (1.0.0 -> 1.0.1)
3. 修复问题
4. 更新CHANGELOG
5. 测试
6. 发布新版本

### 4. 热更新策略

#### 自动更新检查
```typescript
// src/App.tsx
useEffect(() => {
  const checkUpdate = async () => {
    try {
      const currentVersion = await invoke<string>("get_app_version");
      const latestVersion = await fetchLatestVersion();
      if (compareVersions(currentVersion, latestVersion) < 0) {
        showUpdateNotification(latestVersion);
      }
    } catch (err) {
      console.error("检查更新失败:", err);
    }
  };
  checkUpdate();
}, []);
```

---

## 监控与报警

### 1. 错误监控

#### 前端错误捕获
```typescript
window.addEventListener('error', (event) => {
  console.error('全局错误:', event.error);
  // 发送到错误监控服务
});

window.addEventListener('unhandledrejection', (event) => {
  console.error('未处理的Promise拒绝:', event.reason);
});
```

#### 后端错误日志
```rust
log::error!("Conversion failed: {}", error);

// 发送到日志服务
```

### 2. 性能监控

#### 转换性能统计
```rust
let start_time = std::time::Instant::now();
// ... 执行转换
let duration = start_time.elapsed();
log::info!("转换耗时: {:?}", duration);
```

---

## 备份与恢复

### 1. 配置备份

#### 自动备份脚本
```powershell
# backup_config.ps1
$source = "$env:APPDATA\Bilibili缓存转换器\settings.json"
$dest = "backup\settings_$(Get-Date -Format 'yyyyMMdd_HHmmss').json"
Copy-Item $source -Destination $dest
```

### 2. 恢复配置
```powershell
# restore_config.ps1
$backup = "backup\settings_20260306_120000.json"
$dest = "$env:APPDATA\Bilibili缓存转换er\settings.json"
Copy-Item $backup -Destination $dest -Force
```

---

## 最佳实践

### 1. 代码提交规范

#### Commit Message 格式
```
<type>(<scope>): <subject>

<body>

<footer>
```

**类型 (type)**:
- feat: 新功能
- fix: Bug修复
- docs: 文档更新
- style: 代码格式
- refactor: 重构
- test: 测试
- chore: 构建/工具

**示例**:
```
feat(converter): 添加FLAC音频格式支持

- 支持FLAC无损编码
- 更新UI格式选择
- 更新文档

Closes #123
```

### 2. 代码审查清单

- [ ] 代码符合项目规范
- [ ] 无明显的性能问题
- [ ] 错误处理完善
- [ ] 添加必要的注释
- [ ] 更新相关文档
- [ ] 通过所有测试

### 3. 安全建议

- 不要硬编码敏感信息
- 定期更新依赖以修复安全漏洞
- 使用环境变量管理配置
- 实施最小权限原则

---

**文档版本**: 1.0.0
**最后更新**: 2026年3月7日
**维护者**: [待补充]

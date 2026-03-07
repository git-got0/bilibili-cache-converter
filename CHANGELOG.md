# 变更日志 (Changelog)

本文档记录 Bilibili缓存转换器 的所有重要变更。

格式基于 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.0.0/),
版本号遵循 [语义化版本](https://semver.org/lang/zh-CN/)。

---

## [1.0.2] - 2026-03-06

### 新增 (Added)
- ✨ 暂停/恢复转换功能 - 支持在转换过程中暂停和恢复任务
- ✨ 虚拟滚动支持 - 大量文件(>100)时自动启用虚拟滚动提升性能
- ✨ 性能监控指标 - 显示实时转换速度、平均速度、预计剩余时间等
- ✨ 节流优化 - 进度更新使用150ms节流机制减少UI重渲染

### 安全性改进 (Security)

#### 后端安全增强 🔒
- **路径遍历防护**: 在 `open_output_folder` 和 `ensure_output_directory` 命令中添加绝对路径验证
- **输出路径验证**: 在 `convert_files` 中验证输出路径在预期目录内，防止路径遍历攻击
- **文件名清理**: 增强 `sanitize_filename` 函数，防止路径遍历和命令注入
  - 替换连续点号为下划线
  - 过滤更多危险字符 (`\0`, `\n`, `\r`, `\t`)
  - 确保清理后的文件名不为空，默认返回 "output"
- **扫描深度限制**: 添加 `MAX_DEPTH` 常量（7层）防止深度递归攻击
- **符号链接防护**: 禁止跟随符号链接防止无限循环

#### 前端安全增强 🔒
- **路径输入验证**: 在 `selectFolder` 和 `selectOutputFolder` 中添加空字符串和空白字符检查
- **并发数验证**: 在 `updateSettings` 中验证并发数的合法性（1/2/4/8）
- **类型安全改进**: 使用 `(selected as { path?: string })` 替代 `any` 类型

### 代码质量改进 (Code Quality)

#### Rust 修复 🐛
- 修复日志格式化错误 - 使用 `out.finish()` 替代 `writeln!` 宏修复 tauri_plugin_log 兼容性
- 删除未使用的变量 `file_size_mb`
- 移除不必要的类型转换 `f64 as f64`
- 修复 MSRV 警告 - 使用 `to_string_lossy()` 替代 `display().to_string()`
- 修复 `converter.rs` 第706行的编译错误：`components().is_empty()` 改为 `as_os_str().is_empty()`
- 修复借用检查错误：提前克隆 `first_component.as_os_str()` 避免借用冲突
- 添加更详细的错误日志和返回信息

#### 错误处理改进 📝
- 路径验证失败时返回明确的错误信息（"必须是绝对路径"、"目录不存在"等）
- 文件转换失败时记录详细错误到日志
- 输出目录创建失败时提前返回错误，避免继续处理

### 构建验证 ✅
- 通过 `cargo check` 验证所有代码修复
- 无编译错误和警告

---

## [1.0.1] - 2026-03-06

### 新增 (Added)
- ✨ 智能文件命名 - 从 entry.json 中读取 part 和 title 字段作为文件名
- ✨ 文件名长度控制 - 自动截断过长的文件名（限制50字符）
- ✨ 详细日志输出 - 添加文件名生成过程的调试日志

### 改进 (Improved)
- 🎯 优化音频文件处理 - 合并后的音频不再单独转换输出
- 🎯 修复输出路径逻辑 - result 目录始终位于用户选择的根目录下
- 🎯 改进 base_dir 计算 - 从前端传入正确的用户选择路径，而非硬编码上溯层级
- 🎯 修复 JSON 解析 - 使用 `pointer()` 方法正确读取嵌套路径的 title 和 part 字段
- 🎯 优化构建脚本 - 更新 build.bat 使用正确的 Tauri 构建命令

### 修复 (Fixed)
- 🐛 修复 extract_title_from_json 对嵌套路径的解析错误（如 `page_data.title`）
- 🐛 修复输出目录生成位置错误（之前可能在子文件夹下生成 result）
- 🐛 修复 Clippy 警告 - 合并嵌套 if 语句，移除不必要的借用，删除未使用函数
- 🐛 修复构建配置 - build.bat 现在正确构建前端和后端并打包成可执行文件

### 技术细节 (Technical Details)

#### 文件命名逻辑优化
- **优先级**: entry.json 中的 `part` → `title` → 回退到目录标题
- **命名格式**:
  - 有 part: `{title}_P{part}.{ext}` (例: `视频标题_P1.mp4`)
  - 无 part: `{json_title}.{ext}` (例: `视频标题.mp4`)
  - 回退: `{truncated_title}.{ext}` (限制50字符)
- **支持路径**:
  - part: `part`, `page_data.part`, `video_info.part`, `data.part`
  - title: `title`, `page_data/title`, `video_info/title`, `data/title`

#### 输出路径优化
- **默认位置**: `{用户选择文件夹}/result/`
- **目录结构**: 保持原始相对路径结构
- **示例**:
  - 输入: `download/v/video.blv`
  - 输出: `download/result/v/video.mp4`

#### 构建产物
- **可执行文件**: `src-tauri/target/release/bilibili-converter.exe` (7.0 MB)
- **安装程序**: `src-tauri/target/release/bundle/nsis/Bilibili缓存转换器_1.0.0_x64-setup.exe` (67.9 MB)

---

## [1.0.0] - 2026-03-06

### 新增 (Added)
- ✨ 初始版本发布
- ✨ 文件夹选择功能 - 支持GUI选择Bilibili缓存目录
- ✨ 智能文件扫描 - 自动识别.blv、.m4s等Bilibili缓存格式
- ✨ 多格式支持:
  - 视频: MP4、MKV、AVI
  - 音频: MP3、AAC、FLAC
- ✨ 实时进度显示:
  - 动态显示整体任务进度(当前文件/总文件数)
  - 实时显示单个文件转换进度
  - 整体进度计算公式: `(current_index * 100 + progress) / total_count`
- ✨ 任务取消功能:
  - 运行中途可取消转换任务
  - 取消后弹出进度提示框
  - 显示已完成数量/总数量
  - 提供"查看"按钮打开输出文件夹
- ✨ 输出文件夹管理:
  - 支持自定义输出文件夹
  - 支持默认输出路径(源文件夹/result)
  - 未设置输出时,弹出询问是否打开默认路径
  - 自动创建不存在的输出目录
- ✨ 系统托盘支持:
  - 最小化到系统托盘
  - 右键菜单(显示窗口、退出)
  - 托盘图标点击显示窗口
- ✨ 任务完成通知:
  - 右下角弹出通知
  - 显示转换成功数量
  - 支持提示音开关
- ✨ 设置管理:
  - 提示音开关
  - 并发数配置(1/2/4/8)
  - 输出格式选择
  - 自定义输出路径
- ✨ 响应式设计:
  - 支持窗口大小自由调整
  - 最小窗口尺寸: 600x500
  - 自适应不同分辨率屏幕
  - 优化文件列表滚动,防止内容被挤出可视区域

### 改进 (Improved)
- 🎨 优化UI布局,采用深色主题设计
- 🎨 使用 shadcn/ui 组件库,界面现代化
- 🎨 实现响应式布局,支持flex-wrap和grid响应式类
- ⚡ 支持多线程并发转换,提升处理效率
- ⚡ 使用 Tokio 异步运行时,提高I/O性能
- ⚡ 优化文件列表显示,限制最大高度为150px
- ⚡ 减小UI组件尺寸,提高空间利用率

### 技术细节 (Technical Details)

#### 技术栈
- **前端**: React 18.3.1 + TypeScript 5.6.2 + Vite 5.4.10
- **UI库**: shadcn/ui + Tailwind CSS 3.4.17
- **后端**: Tauri 2.10.0 + Rust 1.77+
- **音视频**: FFmpeg 4.x+ (外部依赖)
- **并发**: Tokio + Arc<Mutex> 状态管理

#### 架构特点
- 前后端分离架构
- 基于事件的异步通信
- 多线程并发处理
- 类型安全的TypeScript + Rust

#### 核心模块
- `src/App.tsx` - 主应用组件
- `src-tauri/src/scanner.rs` - 文件扫描模块
- `src-tauri/src/converter.rs` - 格式转换模块
- `src-tauri/src/lib.rs` - 状态管理和命令注册

#### API设计
- Tauri Commands: 前端调用后端命令
- Tauri Events: 后端向前端推送事件
- 类型安全的接口定义

### 已知问题 (Known Issues)
- 需要用户手动安装FFmpeg并添加到PATH
- 仅支持Windows系统
- 暂不支持拖放文件夹

### 系统要求
- Windows 10/11 (64位)
- Node.js 18+ (开发环境)
- Rust 1.77+ (开发环境)
- FFmpeg 4.x+ (运行时依赖)

### 文档
- ✅ 项目总览文档 (PROJECT_README.md)
- ✅ API接口文档 (API_DOCUMENTATION.md)
- ✅ 技术架构文档 (TECHNICAL_ARCHITECTURE.md)
- ✅ 部署维护手册 (DEPLOYMENT_GUIDE.md)

---

## 版本说明

### 版本号规则
- **主版本号 (Major)**: 架构重大改变,不兼容的API变更
- **次版本号 (Minor)**: 新增功能,向后兼容的API变更
- **修订号 (Patch)**: Bug修复,小改进

### 变更类型
- **新增 (Added)**: 新功能
- **改进 (Improved)**: 现有功能的改进
- **修复 (Fixed)**: Bug修复
- **移除 (Removed)**: 删除的功能
- **弃用 (Deprecated)**: 即将删除的功能
- **安全 (Security)**: 安全相关的变更

---

**文档维护**: 本文档应随每次版本更新而更新

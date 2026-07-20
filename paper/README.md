# Besure AI Context 论文 — Overleaf 使用指南

## 快速开始

### 方法一：上传到 Overleaf（推荐）

1. 登录 https://www.overleaf.com （用 elttilz@gmail.com）
2. 点击 **New Project** → **Upload Project**
3. 把 `paper/` 目录下的所有文件打包成 zip 上传：
   ```bash
   cd besureAI && zip -r paper.zip paper/ && # 上传 paper.zip
   ```
4. Overleaf 会自动识别 `main.tex` 并编译

### 方法二：从 GitHub 克隆

```bash
git clone https://github.com/joevise/besureAI.git
cd besureAI/paper/
# 用 Overleaf 的 GitHub 集成或手动上传
```

## 文件结构

```
paper/
├── main.tex           # 主文件（含全部正文 + TikZ 图代码）
├── references.bib     # BibTeX 参考文献（19 篇）
├── README.md          # 本文件
└── closed-loop-memory-system.md  # Markdown 初稿（供参考）
```

## 编译设置

Overleaf 里：
1. 左上角 **Menu** → **Compiler** 选择 **XeLaTeX**（因为用了 xeCJK 中文包）
2. **Main document** 设为 `main.tex`
3. 点击 **Recompile**

> ⚠️ 必须用 XeLaTeX，不能用 pdfLaTeX（中文需要）

## 包含的 5 张图

全部用 TikZ 绘制（矢量图，不需要外部图片文件）：

| 图 | 内容 | TikZ 在文中 |
|----|------|------------|
| 图 1 | 三层强制记忆注入架构（纵深防御） | `\begin{tikzpicture}` 在 Section 3.2 |
| 图 2 | 四态记忆生命周期状态机 | `\begin{tikzpicture}` 在 Section 3.3 |
| 图 3 | 记忆关联图谱（有向图示例） | `\begin{tikzpicture}` 在 Section 3.4 |
| 图 4 | 八维度评估雷达图 | `\begin{tikzpicture}` 在 Section 3.6 |
| 图 5 | 系统架构图（多 Agent + Vault） | `\begin{tikzpicture}` 在 Section 4 |

## 注意事项

1. **字体**：`main.tex` 里用了 Noto CJK 字体。如果 Overleaf 报字体错误，改成：
   ```latex
   \setCJKmainfont{Noto Serif CJK SC}
   ```
   或在 Overleaf Menu → 里确认 `Noto` 字体已安装（默认应该有）

2. **arXiv 提交**：编译成功后，Overleaf → **Submit** → **arXiv**（需要先在 arXiv 注册账号并完成邮箱验证）

3. **英文版**：如果要发 arXiv，建议翻成英文。可以在 main.tex 基础上直接替换正文内容，保留所有 TikZ 图和 bibliography

## 下一步

- [ ] 在 Overleaf 上编译成功
- [ ] 检查 5 张图渲染效果
- [ ] 翻译英文版
- [ ] arXiv 提交

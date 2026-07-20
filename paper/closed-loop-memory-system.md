# 从记录到记忆：大语言模型 Agent 的闭环记忆系统

**From Recording to Remembering: A Closed-Loop Memory System for LLM Agents**

---

## 摘要

大语言模型（LLM）Agent 在软件开发、办公自动化和知识工作领域正获得广泛应用，但它们普遍存在一个尚未被充分认识的根本性缺陷：**元认知缺失**（metacognitive deficit）——即 Agent 缺乏判断"何时应将当前交互信息写入持久记忆"的内建能力。这一缺陷导致 Agent 在每次会话（session）启动时都处于"失忆"状态，大量的决策进展、经验教训和上下文信息在会话间隙被永久丢失。

现有的记忆增强方案（如 RAG、MemGPT、向量数据库等）主要关注记忆的**检索**（retrieval）和**存储**（storage），却忽视了记忆的**写入触发**（write triggering）问题。正如本文所论证的，问题的核心不在于"如何检索记忆"，而在于"什么时候应该写入记忆"——这是一个被现有研究几乎完全忽略的维度。

本文提出一个闭环记忆系统框架，包含三项核心贡献：（1）**三层强制记忆注入机制**（Three-Layer Mandatory Memory Injection），通过行为规则层、程序知识层和工具提示层的递进式注入，将记忆写入行为嵌入 Agent 的执行流程；（2）**四态记忆生命周期模型**（Four-State Memory Lifecycle），引入过期、替代和归档等状态转换，配合记忆关联图谱和主动召回机制，实现对记忆的动态维护；（3）**八维度评估框架**（Eight-Dimension Evaluation Framework），为 Agent 记忆系统提供全面的评估基准。我们在 Besure AI Context 系统中实现了上述框架（100% Rust，单二进制部署，23 个 MCP 工具），并通过实际多 Agent 协作场景验证了其有效性。本文的意义在于提出从 Agent-centric 到 Memory-centric 的范式转变——记忆不应是 Agent 的附属品，而应成为独立的、持久的基础设施。

**关键词**：大语言模型 Agent；记忆系统；元认知；记忆生命周期；MCP Protocol

---

## 1. 引言

### 1.1 背景

随着大语言模型能力的飞速提升，以 LLM 为核心的自主 Agent 系统在软件开发、数据分析、办公自动化等领域展现出前所未有的应用潜力。Claude Code、Cursor、OpenClaw、GitHub Copilot 等工具已经深度融入开发者的日常工作流程，能够执行代码编写、文件操作、工具调用等复杂任务。然而，这些 Agent 系统在功能性上日益完善的同时，却普遍存在一个被忽视的根本性限制：**记忆缺失**。

LLM Agent 的"记忆"本质上依赖于上下文窗口（context window）内的注意力机制。一旦会话结束、上下文被清空，Agent 便回到完全"失忆"的状态。这意味着：跨会话的决策连贯性、经验积累和知识传承——这些人类工作者习以为常的认知能力——对当前的主流 Agent 而言完全缺失。

### 1.2 问题的发现

本文第一作者在开发和维护多个 AI Agent（代号 Joey、Joevise、William）的实践中，深刻体验到了这一问题的严重性。三个 Agent 同时工作数小时，完成了从 V0.4 到 V0.56 的多个版本迭代，涉及查询管理、多 Vault 支持、Dashboard 多 Agent 视角等数十项功能开发和架构决策。然而，检查记忆 Vault 时发现：记录几乎为空。

关键观察是：Agent 并非不会使用记忆工具（`besure add`、`memory_search` 等），而是在执行任务时**不知道"当前这一时刻应该记录"**。它们可以流畅地完成代码编写、测试、部署，但在完成一个里程碑之后，自然地继续执行下一个任务，而不是先停下来记录刚才的进展。

### 1.3 核心论点

我们将这一现象称为**记忆写入触发问题**（The Recording Trigger Problem）：在 LLM Agent 系统中，不存在内建的机制来决定"什么时候应该将当前交互信息写入持久记忆"。

现有方案的共同盲区在于：无论是 RAG [Lewis et al., 2020]、MemGPT [Packer et al., 2023] 还是向量数据库，都假设记忆会被"自然产生"——由人类开发者或某种隐式机制负责写入。但 LLM Agent 缺乏人类开发者所具备的"元认知"能力，即反思"这段信息是否值得记住"的自我意识。问题不在"怎么检索"，而在**"什么时候写入"**。

### 1.4 贡献概述

本文的主要贡献如下：

1. **识别并形式化定义了记忆写入触发问题**，揭示现有记忆增强方案的根本盲区；
2. **提出三层强制记忆注入机制**，通过配置文件注入、技能文档嵌入和工具描述提示三个层次，在不修改底层模型的前提下将记忆写入行为嵌入 Agent 的执行流程；
3. **设计四态记忆生命周期模型**，配合记忆关联图谱和主动召回机制，实现记忆的动态维护和演化；
4. **提出八维度评估框架**，为 Agent 记忆系统提供系统化的评估基准；
5. **实现并开源 Besure AI Context 系统**，验证所提框架的可行性和有效性。

---

## 2. 相关工作

### 2.1 MemGPT

MemGPT [Packer et al., 2023] 是最具影响力的 LLM Agent 记忆管理方案之一。它借鉴操作系统的虚拟内存管理思想，将 Agent 的记忆分为核心记忆（core memory）和归档记忆（archival memory）两层。核心记忆驻留在上下文窗口中，归档记忆则存储在外部，通过 LLM 自主决策的函数调用进行读写。MemGPT 的关键创新在于让 LLM 自己管理记忆的分页（paging），通过编辑核心记忆和检索归档记忆来维持长期对话的连贯性。

然而，MemGPT 的设计关注的是**记忆管理**（memory management）而非**写入触发**（write triggering）。它假设 LLM 会自主地在合适的时候将信息写入记忆，但没有解决"LLM 如何知道现在是写入的好时机"这一元认知问题。实践中，LLM 往往在处理用户请求时全神贯注于任务本身，而不会自发地停下来记录。

### 2.2 检索增强生成（RAG）

检索增强生成 [Lewis et al., 2020] 通过在生成时检索外部知识库来增强 LLM 的输出质量，已被广泛应用于问答系统和知识密集型任务。RAG 框架包含检索器（retriever）和生成器（generator）两个组件，通过不同的融合策略（naive RAG、advanced RAG、modular RAG [Gao et al., 2024]）实现知识注入。

RAG 本质上是一个**只读**（read-only）系统：它从预先存在的知识库中检索信息，但不负责向知识库中写入新信息。在实际应用中，知识库的更新（即记忆的写入）通常由人工或独立的管道完成。RAG 优化了信息的检索效率，但完全回避了"何时写入、写入什么"的问题。

### 2.3 长上下文窗口

近年来，LLM 的上下文窗口规模急剧增长，从早期的 4K tokens 扩展到 Gemini 1.5 Pro 的 2M tokens [Google, 2024]、Claude 的 200K tokens 等。长上下文窗口在单次会话中缓解了信息丢失的问题，但存在两个根本局限：（1）上下文窗口不是记忆——会话结束后信息即丢失；（2）过长的上下文会导致"中间遗忘"（lost in the middle）[Liu et al., 2024] 现象，位于上下文中部的信息被忽略的概率显著高于首尾两端。

扩大上下文窗口本质上是在增加短期记忆的容量，而非建立真正的长期记忆。正如人类的工作记忆（working memory）容量有限但可以通过编码过程转化为长期记忆，Agent 也需要类似的编码和存储机制。

### 2.4 向量数据库

Pinecone、Weaviate、Chroma 等向量数据库为 LLM Agent 提供了高效的语义检索能力。这些系统通过将文本编码为高维向量并使用近似最近邻（ANN）搜索，实现了大规模记忆库的快速检索。

然而，向量数据库是通用的存储和检索基础设施，它们不包含任何关于"应该存储什么"或"什么时候存储"的语义。将向量数据库用于 Agent 记忆，仍然需要一个上游系统来决定写入触发——这正是现有方案的共同盲区。

### 2.5 Model Context Protocol（MCP）

MCP [Anthropic, 2024] 是一个标准化 LLM Agent 与外部工具/数据源交互的开放协议。它定义了工具（tools）、资源（resources）和提示（prompts）三类原语，使得不同平台上的 Agent 能够以统一的方式访问外部能力。

MCP 的贡献在于标准化了工具交互的**协议层**，但未定义记忆管理的**语义层**。一个记忆系统可以通过 MCP 暴露为一组工具（如 `add_entry`、`search`、`recall`），但 MCP 本身不规定这些工具应该何时被调用、调用频率应该是多少、以及在什么场景下调用是强制性的。我们的工作在 MCP 协议之上补充了这一语义层。

### 2.6 认知科学中的记忆理论

人类记忆的研究有着深厚的学术传统。Atkinson-Shiffrin 模型 [Atkinson & Shiffrin, 1968] 提出了感觉记忆、短期记忆和长期记忆的三级结构。Ebbinghaus 遗忘曲线 [Ebbinghaus, 1885] 揭示了记忆随时间自然衰减的规律。联想记忆理论 [Anderson & Bower, 1973] 强调记忆之间的关联网络在回忆中的关键作用。Tulving [1972] 区分了情景记忆（episodic memory）和语义记忆（semantic memory）。

这些理论为 Agent 记忆系统设计提供了重要启示：（1）记忆需要分层管理；（2）记忆具有时效性且会自然衰减；（3）记忆之间应建立关联以支持推理；（4）不同类型的记忆具有不同的特性。然而，人类记忆的编码和检索过程是自主的、自动的，而 LLM Agent 没有这种自动机制——这是人工系统与生物系统的根本差异，也是记忆写入触发问题的根源。

### 2.7 现有方案的共同盲区

表 1 总结了现有方案在记忆生命周期各环节的覆盖情况：

**表 1：现有记忆增强方案的覆盖维度对比**

| 方案 | 写入触发 | 存储管理 | 检索机制 | 状态管理 | 关联维护 |
|------|---------|---------|---------|---------|---------|
| RAG | ✗（外部写入） | ✓ | ✓ | ✗ | ✗ |
| MemGPT | △（自主决策，但不可靠） | ✓ | ✓ | △（core/archival 分层） | ✗ |
| 向量数据库 | ✗（外部写入） | ✓ | ✓ | ✗ | ✗ |
| 长上下文窗口 | ✗ | ✗（仅短期） | ✓（注意力） | ✗ | ✗ |
| MCP | ✗（协议不定义语义） | —（通过工具实现） | —（通过工具实现） | ✗ | ✗ |
| **本文方案** | **✓（三层强制注入）** | **✓** | **✓** | **✓（四态模型）** | **✓（关联图谱）** |

如表 1 所示，**所有现有方案都没有解决写入触发问题**。这一盲区是本文的核心切入点。

---

## 3. 理论框架

### 3.1 记忆写入触发问题

**定义 1（记忆写入触发问题）**。在 LLM Agent 系统中，设 Agent 在时刻 $t$ 的上下文为 $C(t)$，累积交互历史为 $H(t) = \{h_1, h_2, \ldots, h_t\}$。当 Agent 从时刻 $t$ 过渡到时刻 $t + \Delta t$（即开始新的会话）时，上下文差量 $\Delta C = C(t + \Delta t) \setminus C(t)$ 中的信息将丢失，除非在 $t$ 时刻执行了显式写入操作 $W(h_i) \to M$，其中 $M$ 为持久记忆存储。

LLM Agent 系统缺乏一个内建函数 $f: (C(t), H(t)) \to \{0, 1\}$ 来判断当前时刻是否应触发记忆写入。我们将这一缺失称为**记忆写入触发问题**。

**核心论点**。LLM 的"记忆"本质是上下文窗口内的注意力分配，不是真正的记忆（memory in the cognitive sense）。真正的记忆需要满足四个条件：

$$\text{Memory} = \langle \text{Encode}, \text{Store}, \text{Retrieve}, \text{Maintain} \rangle$$

其中：
- **编码（Encode）**：将交互信息转化为结构化记忆条目；
- **存储（Store）**：将记忆条目持久化到外部存储；
- **检索（Retrieve）**：在需要时检索相关记忆；
- **维护（Maintain）**：管理记忆的生命周期（过期、替代、关联）。

现有方案主要解决了 Store 和 Retrieve 两个环节，我们的工作补齐了 Encode（通过三层注入机制触发编码行为）和 Maintain（通过四态生命周期管理）。

### 3.2 三层强制记忆注入机制

我们提出三层递进式注入机制，在不修改底层 LLM 的前提下，将记忆写入行为嵌入 Agent 的执行流程。

#### 3.2.1 Layer 1：行为规则层（Behavioral Rules）

**载体**：Agent 配置文件（如 OpenClaw 的 `AGENTS.md`、Claude Code 的 `CLAUDE.md`、Cursor 的 `.cursorrules`）。

**机制**：Agent 配置文件在每次会话启动时被强制加载到系统提示词（system prompt）中。我们在这个文件中注入一段"记忆铁律"（Mandatory Recording Rules），明确列出必须触发记忆写入的场景：

1. **完成任何任务/功能/修复** → 记录为 `milestone` 类型；
2. **做了决策或达成结论** → 记录为 `decision` 类型；
3. **发现问题/踩坑** → 记录为 `lesson` 类型；
4. **会话即将结束** → 记录为 `progress` 类型（总结本次所有进展）；
5. **用户明确要求** → 立即执行。

注入的规则还包含一个简明的判断标准：*"如果这个信息在下次会话中可能有用，就必须记。宁可多记，不可漏记。"*

**关键设计决策**：这不是"建议"（suggestion）或"最佳实践"（best practice），而是与"读取用户身份""检查系统配置"同级别的**强制步骤**。这一设计选择基于对元认知缺失的根本判断——如果将记忆写入降级为可选行为，LLM 的注意力分配机制会自然地将其忽略。

**技术实现**：采用幂等注入（idempotent injection）策略。注入的内容用特殊标记（如 `<!-- BESURE-AUTO-START -->`）包裹，重复执行时自动检测并替换，避免重复注入。多平台检测通过扫描已知配置文件路径实现：

| 平台 | 配置文件 | 检测路径 |
|------|---------|---------|
| OpenClaw / Codex / Hermes / WorkBuddy | `AGENTS.md` | 工作目录根 |
| Claude Code | `CLAUDE.md` | 工作目录根 |
| Cursor | `.cursorrules` | 工作目录根 |
| CodeBuddy | `.codebuddy/rules.md` | 工作目录根 |

#### 3.2.2 Layer 2：程序知识层（Procedural Knowledge）

**载体**：技能文档（Skill Document，如 MCP 工具的 `SKILL.md`）。

**机制**：Agent 在调用特定工具或技能时，通常会先读取相应的技能文档以了解使用方法。我们在技能文档的顶部加入一段 **MANDATORY RECORDING RULES**，详细说明记忆写入的格式、类型分类和最佳实践。

**与 Layer 1 的关系**：Layer 1 解决"什么时候记"（when），Layer 2 解决"怎么记"（how）和"记什么格式"（what format）。当 Agent 按照 Layer 1 的规则触发记忆写入时，Layer 2 提供操作指南。例如，Layer 1 规定"完成任何任务后必须记录"，Layer 2 则说明"使用 `besure add --type milestone` 命令，内容应包含：任务描述、实现方案、关键技术决策、遇到的困难和解决方法"。

**强化效应**：Agent 在一次会话中可能调用多个技能，每次调用都会重新读取技能文档。这意味着 Layer 2 的规则在会话过程中被反复强化，弥补了 Layer 1 仅在启动时加载一次的局限。

#### 3.2.3 Layer 3：工具提示层（Tool-Level Reminders）

**载体**：MCP 工具描述（tool description）。

**机制**：在 MCP 协议中，每个工具都有一个 `description` 字段，用于向 Agent 说明工具的功能和用法。Agent 在扫描可用工具列表时会读取这些描述。我们在记忆工具的描述中嵌入强制性提示，例如：

> `besure_add_entry`：⚠️ MANDATORY: Call this tool after completing tasks, making decisions, or encountering issues. Do NOT skip recording—session memory loss is irreversible.

**关键设计**：工具描述是 Agent 决策"使用哪个工具"时的直接依据。通过在描述中嵌入强制提示，我们利用了 Agent 的工具选择机制来触发记忆行为。即使 Layer 1 和 Layer 2 在某些情况下失效（如配置文件被覆盖或技能文档未被读取），只要 Agent 使用了记忆工具，Layer 3 的提示就会在工具选择时发挥作用。

#### 3.2.4 三层协同分析

三层的协同关系如表 2 所示：

**表 2：三层强制记忆注入的协同关系**

| 层级 | 载体 | 加载时机 | 解决问题 | 强制力 | 冗余性 |
|------|------|---------|---------|--------|--------|
| Layer 1 | AGENTS.md / CLAUDE.md | 会话启动时 | When（什么时候记） | 最强（启动必读） | 无冗余 |
| Layer 2 | SKILL.md | 技能调用时 | How + What（怎么记、记什么） | 中等（调用时读） | Layer 1 的强化 |
| Layer 3 | MCP tool description | 工具选择时 | Reminder（调用时提醒） | 最弱但最频繁 | Layer 1+2 的兜底 |

三层机制形成**纵深防御**（defense in depth）架构：任何一层失效，其他两层仍能工作。这种冗余设计是应对 LLM 指令遵循不确定性的关键策略。

### 3.3 四态记忆生命周期模型

#### 3.3.1 状态定义

现有记忆系统的记忆条目只有两个状态：**存在**或**删除**。这种二态模型无法表达记忆的时效性、版本演化和重要性变化。我们提出四态记忆生命周期模型：

**定义 2（四态记忆生命周期）**。记忆条目 $m$ 的状态 $S(m) \in \{\text{Active}, \text{Expired}, \text{Superseded}, \text{Archived}\}$，状态转换由显式操作或时间条件触发：

```
Active ──expire──→ Expired ──archive──→ Archived
    ↘ supersede ↗          ↑
      Superseded ──────────┘
```

- **Active（活跃）**：当前有效的记忆，参与所有常规查询和召回；
- **Expired（过期）**：时效性到期，通过 `valid_until` 字段自动到期或手动标记。不参与常规查询，但通过 `recall` 主动召回；
- **Superseded（被替代）**：被更新的记忆条目覆盖。旧记忆不删除（保留历史），但在常规查询中降权或隐藏。每条被替代的记忆通过 `superseded_by` 字段指向替代者；
- **Archived（归档）**：手动 sidelined，不删除但不参与任何常规查询。需要显式的归档查询才能检索。

#### 3.3.2 与认知科学理论的对应

四态模型与认知科学的记忆理论存在明确的对应关系：

| 状态转换 | 认知科学对应 | 理论来源 |
|---------|------------|---------|
| Active → Expired | 自然衰减（decay theory） | [Ebbinghaus, 1885] |
| Active → Superseded | 记忆更新（memory updating） | [Naveh-Benjamin & Jonides, 1984] |
| Active → Archived | 长期记忆深处（deep storage） | [Atkinson & Shiffrin, 1968] |
| Recall（主动召回） | 默认模式网络（default mode network） | [Raichle et al., 2001] |

**关键区别**：人类的记忆衰减是**自然且不可控**的，而我们的系统采用**显式标记**——记忆不会自动消失，而是被显式地管理。这一设计选择使得记忆系统可以保留完整的历史记录，同时通过状态标记来控制查询时的信息过滤。

#### 3.3.3 与 MemGPT 的对比

MemGPT 在 core memory 和 archival memory 之间做**空间分层**（spatial tiering）——信息根据重要性在两个存储层之间移动。我们的四态模型是**状态管理**（state management）——同一条记忆可以在任何存储位置上处于不同的生命周期状态。两种模型是正交的，可以组合使用。

### 3.4 记忆关联图谱

#### 3.4.1 关系类型

每条记忆条目可以有向链接到其他记忆条目或外部资源，形成有向图结构：

**表 3：记忆关联类型**

| 关系类型 | 语义 | 方向 | 学术对应 |
|---------|------|------|---------|
| `caused_by` | A 导致了 B | A→B（因果） | Causal reasoning [Pearl, 2009] |
| `supersedes` | 新 A 替代旧 B | A→B（替代） | Memory updating |
| `related_to` | A 与 B 相关 | A↔B（联想） | Associative memory [Anderson & Bower, 1973] |
| `ref_file` | A 引用文件 F | A→F（外部） | Episodic memory [Tulving, 1972] |
| `ref_commit` | A 引用代码提交 | A→C（外部） | Source monitoring [Johnson et al., 1993] |
| `ref_url` | A 引用网页 | A→U（外部） | Episodic memory |

#### 3.4.2 推理链路追踪

关联图谱支持多种推理模式：

- **因果链路**：从一条记忆出发，沿着 `caused_by` 链可以追溯到根因。例如，`bug_fix_D → caused_by → bug_report_C → caused_by → feature_release_A`，形成完整的因果链。
- **决策演进**：沿着 `supersedes` 链可以查看决策的演进历史。例如，`decision_v3 → supersedes → decision_v2 → supersedes → decision_v1`，展示决策从初始到最终的完整演化过程。
- **联想检索**：通过 `related_to` 关系可以发现相关记忆，支持联想式推理。

### 3.5 主动召回机制

#### 3.5.1 设计理念

现有方案的记忆检索主要是**被动的**（reactive）——Agent 在需要时通过关键词搜索或语义检索查找记忆。我们引入**主动召回**（proactive recall）机制：Agent 在会话启动时主动获取需要注意的记忆，而非等待需要时才搜索。

主动召回的内容包括：
- 即将过期的记忆（`valid_until` 临近当前时间）；
- 已过期但未归档的记忆；
- 被替代但可能需要回顾的记忆；
- 最近时间段内的重要记忆（如上一个会话的进展记录）。

#### 3.5.2 理论对应

主动召回对应人脑的**默认模式网络**（Default Mode Network, DMN）[Raichle et al., 2001]。DMN 在大脑处于休息状态或任务切换间隙时活跃，主动回放和整理重要记忆。Agent 的 `recall` 命令模拟了这一机制——在会话启动时（相当于"休息结束、开始工作"的时刻），主动整理和提示需要注意的记忆。

### 3.6 八维度评估框架

基于上述理论框架，我们提出 Agent 记忆系统的八维度评估框架：

**表 4：八维度评估框架**

| 维度 | 定义 | 评估指标 | 现有方案覆盖 |
|------|------|---------|------------|
| 主体（Identity） | 这是谁的记忆？ | Agent 身份隔离度 | 部分方案支持 |
| 编码（Encoding） | 信息如何被编码为记忆？ | 记录格式规范性、颗粒度 | 很少被讨论 |
| 完整性（Completeness） | 记忆是否完整？ | 关键信息丢失率 | 很少被评估 |
| 上下文（Context） | 在什么场景下产生？ | 上下文绑定深度 | 部分方案支持 |
| 检索（Retrieval） | 能否被有效检索？ | 检索准确率、召回率 | 大量方案关注 |
| 关联（Association） | 与其他记忆的关系？ | 关联图谱密度、连通性 | 极少被讨论 |
| 时效（Temporality） | 什么时候有效？ | 时间有效性管理 | 很少被讨论 |
| 失效（Decay） | 如何标记过时？ | 生命周期管理能力 | 极少被讨论 |

**关键观察**：现有研究和方案主要关注**检索**维度，对其余七个维度缺乏系统性关注。我们的八维度框架揭示了大量被忽视的设计空间。

---

## 4. 实现

### 4.1 系统架构

我们在 Besure AI Context 系统中实现了上述理论框架。系统采用 100% Rust 实现，编译为单一二进制文件，零外部运行时依赖。

**表 5：系统实现概要**

| 组件 | 技术选型 | 说明 |
|------|---------|------|
| 加密引擎 | AES-256-GCM + Argon2 | 端到端加密，密钥不从二进制中提取 |
| 数据库 | SQLite（编译进二进制） | 零配置，单文件存储 |
| CLI | clap v4 | 全功能命令行界面 |
| MCP Server | stdio JSON-RPC | 23 个工具，覆盖全部记忆操作 |
| REST API | axum + tokio | Dashboard 后端 |
| Web Dashboard | 单文件 HTML/CSS/JS | 零前端依赖，多 Agent 视角 |
| 向量检索 | 内嵌 embedding | 支持语义搜索（可选） |

### 4.2 多上下文隔离

系统通过环境变量 `BESURE_VAULT` 实现物理隔离，每个 Agent 拥有独立的 vault 目录：

```
~/.besure/
├── joey/          # Agent "Joey" 的 vault
│   ├── .besure.config   # 加密配置 + agent_name + agent_type
│   └── besure.db        # SQLite 数据库
├── joevise/       # Agent "Joevise" 的 vault
├── william/       # Agent "William" 的 vault
└── shared/        # 跨 Agent 共享 vault
```

每个 vault 独立加密，Agent 只能访问自己的 vault（通过环境变量路径限制）。全局视角通过 `BESURE_VAULTS_ALL=true` 环境变量开启，仅授予人类使用的 Dashboard。

### 4.3 `besure setup` 命令

`besure setup` 实现一键配置流程：

1. **初始化 vault**：创建加密的存储目录和配置文件，写入 `agent_name` 和 `agent_type` 元数据；
2. **多平台检测**：扫描当前工作目录和上级目录，查找已知的 Agent 配置文件（`AGENTS.md`、`.hermes.md`、`CLAUDE.md`、`.cursorrules`、`.codebuddy/rules.md`）；
3. **幂等注入**：将记忆铁律段落用 `<!-- BESURE-AUTO-START/END -->` 标记包裹后写入检测到的配置文件。重复执行时自动替换标记之间的内容，保证幂等性；
4. **服务安装**：安装 Dashboard 后台服务（Linux: systemd, macOS: launchd, Windows: 启动项 + VBS）。

### 4.4 MCP 工具设计

系统通过 MCP 协议暴露 23 个工具，覆盖记忆的全生命周期操作：

**表 6：MCP 工具分类**

| 类别 | 工具数量 | 代表工具 |
|------|---------|---------|
| 记忆写入 | 3 | `besure_add_entry`, `besure_resolve`, `besure_append` |
| 检索查询 | 4 | `besure_search`, `besure_query`, `besure_recall`, `besure_stats` |
| 生命周期 | 3 | `besure_expire`, `besure_supersede`, `besure_link` |
| 上下文管理 | 4 | `besure_switch_context`, `besure_list_contexts`, `besure_create_context`, `besure_status` |
| 配置 | 3 | `besure_config_set`, `besure_config_get`, `besure_config_list` |
| 多 Vault | 3 | `besure_share_entry`, `besure_share_context`, `besure_shared` |
| 其他 | 3 | `besure_unlock`, `besure_lock`, `besure_export` |

### 4.5 多平台支持

当前支持的平台和对应的配置文件：

| 平台 | 配置文件 | 注入方式 |
|------|---------|---------|
| OpenClaw | `AGENTS.md` | `besure setup` 自动检测 |
| Hermes Agent | `.hermes.md` / `AGENTS.md` | 自动检测（fallback） |
| Claude Code | `CLAUDE.md` | 自动检测 |
| Cursor | `.cursorrules` | 自动检测 |
| Codex | `AGENTS.md` | 自动检测 |
| 腾讯 CodeBuddy | `.codebuddy/rules.md` | 自动检测 |
| 腾讯 WorkBuddy | `AGENTS.md` | 自动检测 |

---

## 5. 案例研究

### 5.1 场景描述

我们在 Besure AI Context 系统自身的开发过程中部署了三个 AI Agent（Joey、Joevise、William），用于验证所提框架的有效性。三个 Agent 分别运行在 OpenClaw 平台上，各自拥有独立的 vault。

在部署三层强制记忆注入机制之前，三个 Agent 在数小时的协作开发中（完成 V0.4 到 V0.5.1 的多个版本迭代），vault 中的记忆条目极少。Agent 能够正常使用记忆工具，但不会主动触发写入。

### 5.2 问题诊断

通过对 Agent 行为的观察，我们发现了以下模式：

1. **任务完成后跳过记录**：Agent 完成一个功能后，直接进入下一个任务，不停下来记录刚才的进展；
2. **一句话摘要代替详细记录**：即使 Agent 执行了记忆写入，内容通常只有一句话摘要，缺乏决策背景、技术细节等关键上下文；
3. **跨会话信息丢失**：会话结束后，大量上下文信息（讨论过程、被否决的方案、踩过的坑）永久丢失。

这些观察验证了记忆写入触发问题的存在：Agent 不是不会用工具，而是缺乏"现在该记了"的元认知。

### 5.3 部署三层注入

我们通过 `besure setup` 命令为三个 Agent 的配置文件注入记忆铁律，同时更新了 SKILL.md 和 MCP 工具描述。部署后，Agent 的行为发生了显著变化：

- **记录频率提升**：Agent 在完成任务后会主动执行 `besure add`；
- **记录颗粒度改善**：通过 SKILL.md 中的格式指导，记录内容包含了更多上下文；
- **召回利用率提升**：会话启动时的 `recall` 命令使得 Agent 能够主动恢复上一会话的关键信息。

### 5.4 经验教训

在实际部署中，我们也发现了一些需要改进的方面：

1. **颗粒度控制**：强制规则导致部分 Agent 过度记录（记录过于琐碎的信息），需要更精细的颗粒度指导；
2. **格式规范**：不同 Agent 的记录格式不一致，需要更强的结构化约束；
3. **记忆质量评估**：目前缺乏自动化的记忆质量评估机制，需要人工审查。

---

## 6. 讨论

### 6.1 "强制写入" vs "智能写入"

本文选择**强制**（mandatory）而非**智能**（intelligent）的记忆写入策略。这一选择基于以下论据：

1. **元认知缺失的根本性**：LLM 的"智能判断"本身就受到元认知缺失的影响。让一个不知道"什么时候该记"的系统去"智能判断"是否该记，是一个循环论证。
2. **成本不对称性**：强制写入的成本是"多记一些冗余信息"（存储成本极低），而遗漏记忆的代价是"永久丢失关键上下文"（不可逆）。在成本严重不对称的情况下，宁可过度记录。
3. **延迟和计算成本**：智能判断需要额外的 LLM 推理，增加延迟和 token 消耗。强制规则是确定性的，零额外计算成本。
4. **类比 git**：版本控制的最佳实践是"频繁提交"（commit often），而非"智能判断什么时候该提交"。记忆写入同理。

**未来方向**：随着 LLM 元认知能力的提升，可以探索"强制为主、智能为辅"的混合策略——强制规则确保底线覆盖，智能判断优化记录的颗粒度和格式。

### 6.2 从 Agent-centric 到 Memory-centric

现有 AI Agent 架构以 Agent 为中心，记忆是 Agent 的附属品——Agent 创建记忆、使用记忆，但记忆随 Agent 的消亡而消失。本文提出 **Memory-centric** 范式：

- **记忆作为基础设施**：记忆是独立的、持久的，不依赖于特定的 Agent 实例；
- **Agent 作为使用者**：Agent 是记忆的使用者和贡献者，但不是记忆的所有者；
- **跨 Agent 共享**：记忆可以跨 Agent 共享，支持团队协作；
- **记忆生命周期独立于 Agent 生命周期**：Agent 可以被重建、替换、升级，但记忆永久存在。

这一范式转变对 Agent 系统的架构设计有深远影响：记忆系统不再是 Agent 内部的一个模块，而是独立的基础设施层。

### 6.3 局限性

本文的工作存在以下局限：

1. **平台依赖**：三层注入机制依赖 Agent 平台支持配置文件加载。对于不使用配置文件的 Agent 平台（如纯 API 调用的 Agent），Layer 1 无法直接应用；
2. **指令遵循不确定性**：强制规则的执行依赖 LLM 的指令遵循（instruction following）能力。虽然现代 LLM 的指令遵循能力已经相当强，但仍存在跳过规则的可能；
3. **评估的初步性**：目前的评估基于实际使用经验的案例研究，缺乏大规模的定量实验；
4. **单机限制**：当前实现是单机版，多 Agent 协作需要通过共享 vault 或网络同步。

---

## 7. 结论与未来工作

### 7.1 结论

本文识别并形式化定义了 LLM Agent 记忆系统中的"记忆写入触发问题"，揭示现有方案的重检索轻写入的根本盲区。我们提出三层强制记忆注入机制，通过行为规则层、程序知识层和工具提示层的递进式注入，将记忆写入行为嵌入 Agent 的执行流程。在此基础上，我们设计了四态记忆生命周期模型，配合记忆关联图谱和主动召回机制，实现记忆的动态维护。我们还提出八维度评估框架，揭示了 Agent 记忆系统设计中被忽视的维度。

我们在 Besure AI Context 系统中实现了上述框架，并通过多 Agent 协作开发场景验证了其有效性。系统已开源：https://github.com/joevise/besureAI

### 7.2 未来工作

1. **定量评估**：设计对照实验，量化三层注入机制对记忆覆盖率的影响；
2. **团队协作记忆**：扩展多 Agent 共享机制，支持团队级的记忆协作；
3. **记忆质量自动评估**：基于八维度框架设计自动化的记忆质量评分；
4. **跨 Agent 记忆推理**：在关联图谱上实现跨 Agent 的推理链路；
5. **记忆压缩与摘要**：定期将高频访问的记忆压缩为高层摘要；
6. **arXiv 发布**：将本文扩展为英文全文，提交至 arXiv（cs.AI / cs.CL）。

---

## 参考文献

[1] Packer, C., Wooders, S., Lin, K., Fang, S., Gupta, S., & González, O. (2023). "MemGPT: Towards LLMs as Operating Systems." arXiv preprint arXiv:2310.08560.

[2] Lewis, P., Perez, E., Piktus, A., Petroni, F., Karpukhin, V., Goyal, N., et al. (2020). "Retrieval-Augmented Generation for Knowledge-Intensive NLP Tasks." NeurIPS 2020.

[3] Gao, Y., Xiong, Y., Gao, X., Jia, K., Pan, J., Bi, Y., et al. (2024). "Retrieval-Augmented Generation for Large Language Models: A Survey." arXiv preprint arXiv:2312.10997.

[4] Anthropic (2024). "Model Context Protocol Specification." Available at: https://modelcontextprotocol.io

[5] Google (2024). "Gemini 1.5 Pro: Unlocking Long-Context Reasoning." Google AI Blog.

[6] Liu, N.F., Lin, K., Hewitt, J., Paranjape, A., Bevilacqua, M., Petroni, F., & Liang, P. (2024). "Lost in the Middle: How Language Models Use Long Contexts." TACL.

[7] Atkinson, R.C., & Shiffrin, R.M. (1968). "Human Memory: A Proposed System and its Control Processes." In Psychology of Learning and Motivation (Vol. 2, pp. 89-195).

[8] Ebbinghaus, H. (1885). "Über das Gedächtnis: Untersuchungen zur experimentellen Psychologie." Leipzig: Duncker & Humblot.

[9] Anderson, J.R., & Bower, G.H. (1973). "Human Associative Memory." Washington, DC: Winston.

[10] Tulving, E. (1972). "Episodic and Semantic Memory." In Organization of Memory (pp. 381-403). Academic Press.

[11] Raichle, M.E., MacLeod, A.M., Snyder, A.Z., Powers, W.J., Gusnard, D.A., & Shulman, G.L. (2001). "A Default Mode of Brain Function." PNAS, 98(2), 676-682.

[12] Naveh-Benjamin, M., & Jonides, J. (1984). "Maintenance Rehearsal: A Second Look." Memory & Cognition, 12(2), 175-184.

[13] Johnson, M.K., Hashtroudi, S., & Lindsay, D.S. (1993). "Source Monitoring." Psychological Bulletin, 114(1), 3-28.

[14] Pearl, J. (2009). "Causality: Models, Reasoning and Inference." Cambridge University Press.

[15] Wang, L., Ma, C., Feng, X., et al. (2024). "A Survey on Large Language Model based Autonomous Agents." Frontiers of Computer Science, 18, 186345.

[16] Yao, S., Zhao, J., Yu, D., et al. (2022). "ReAct: Synergizing Reasoning and Acting in Language Models." arXiv preprint arXiv:2210.03629.

[17] Park, J.S., O'Brien, J.C., Cai, C.J., et al. (2023). "Generative Agents: Interactive Simulacra of Human Behavior." UIST 2023.

[18] Laird, J.E., Lebiere, C., & Rosenbloom, P.S. (2017). "A Standard Model of the Mind." AI Magazine, 38(4), 13-26.

[19] Wei, J., Wang, X., Schuurmans, D., et al. (2022). "Chain-of-Thought Prompting Elicits Reasoning in Large Language Models." NeurIPS 2022.

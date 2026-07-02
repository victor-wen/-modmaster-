# Modbus TCP/RTU 上位机平台 — 设计规格书

> 基于 Rust + Tauri 2.x + React/TypeScript 的 Modbus TCP/RTU 上位机，面向中小规模工业现场，
> 以"接口契约先行、能力逐子项目兑现"方式保证可扩展性与二次开发能力。

| 项目 | 值 |
|---|---|
| 文档版本 | 1.0 |
| 创建日期 | 2026-07-02 |
| 目标 OS | Windows（WebView2 Runtime 依赖） |
| 部署形态 | Tauri 桌面应用，单一 .exe |
| 开发语言 | Rust（后端）+ TypeScript/React（前端） |

---

## 1. 总体路线图与子项目划分

整个平台型上位机按"承诺写在接口、能力逐子项目兑现"展开，拆为 4 个子项目：

| 子项目 | 目标 | 预计复杂度 |
|---|---|---|
| **SP1（当前设计）** | Modbus 采集核心 + 仪表盘实时监控 + 轻量历史 | 中 |
| SP2 | 组态画面（SVG 拖拽编辑器 + 运行时渲染） | 高 |
| SP3 | 报警 / 长期归档 / 报表 / 趋势增强 | 高 |
| SP4 | 扩展兑现层：协议动态加载（dylib）/ 脚本引擎（Lua 或 JS）/ 外部 widget 包加载 | 高 |

**SP1 对"可扩展性"的承诺方式**（不空谈）：
- 协议层：`Source` trait 既用于内置 Modbus 适配器，也是 SP4 动态插件的 ABI 基础，在 SP1 即稳定。
- UI 层：组件走"widget 注册表 + manifest"，SP1 内置组件是注册表项；SP4 外部 widget 沿用同 manifest。
- 逻辑层：`event-bus` 预留 `before-publish` / `after-poll` 钩子点位，SP3 的报警和 SP4 的脚本挂载在这些点位。

**长期形态**：SP1 是单一 .exe 无运行时依赖（除自带 WebView2）。SP4 的动态加载是可选增强，不破坏单二进制默认形态。

---

## 2. MVP（SP1）功能清单与边界

### 包含（In Scope）

1. **设备配置** — 添加/编辑 Modbus TCP 与 RTU 设备（地址、端口/串口参数、slave id、轮询周期、超时）。
2. **数据点定义** — 按设备绑定寄存器（Holding/Input/Coil/Discrete），含数据类型（u16/i16/u32/f32/bool）、字节序、缩放系数、单位、读写属性。
3. **采集引擎** — 用 `tokio-modbus` 并发轮询多设备，按各设备自身周期独立轮询（默认 1s）。TCP 与 RTU 共用 `Source` trait，RTU 底层 `serialport` crate。
4. **实时监控画面（仪表盘）** — React 仪表盘页，预置组件：数值表、实时曲线、仪表、状态灯、柱状图。用户选组件、拖布局、绑数据点、调参数、保存布局。
5. **轻量历史** — SQLite 本地库，所有数据点默认 1s 采样入库（可配周期），提供最近 N 小时趋势查询。
6. **工程管理** — 新建/打开/保存工程。工程目录包含 `project.toml` + `devices.toml` + `tags.toml` + `dashboards/*.toml` + `history.db`。可整体复制迁移、可手编、可版本控制。
7. **连接状态与日志** — 主界面显示各设备在线/离线/通信错误计数；操作日志窗口。
8. **Modbus 模拟器验收** — 随项目附带 `pymodbus.server` 验收剧本，端到端跑通。

### 不包含（Out of Scope，留给后续子项目）

- 组态 SVG 画面（SP2）
- 报警事件、长期归档压缩、报表导出（SP3）
- 第三方协议动态插件、脚本引擎、外部 widget 包加载（SP4）
- OPC UA / MQTT / 自定义串口协议（SP4 通过新 `Source` 适配器）
- 用户权限/登录、多机冗余/分布式、设备发现

### 边界澄清

- **"可配置组件" ≠ "外部插件"**：SP1 预置组件是注册表内置项，不是外部包。视觉和配置上与未来外部包一致（同 manifest 格式），加载机制留待 SP4。
- **写操作范围**：SP1 支持"手动写一个数据点"（控制按钮），不批量写/脚本联动（SP3/SP4）。
- **历史库与工程文件分离**：history.db 放工程目录内但独立文件，便于备份/清理时不碰配置。

---

## 3. Rust 核心架构分层与 crate 划分

### Crate 拓扑

```
host-computer/
├── crates/
│   ├── hc-core/          # 协议无关：trait 契约、数据模型、event-bus、错误类型
│   ├── hc-modbus/        # 内置 Modbus 适配器（impl Source）
│   ├── hc-storage/       # SQLite 历史层 + 工程文件（TOML）读写
│   ├── hc-runtime/       # 采集运行时：调度器、连接池、数据通路编排
│   ├── hc-ipc/           # Tauri 命令/事件桥（throttle、序列化、前端 API）
│   └── hc-app/           # 应用粘合：DI 装配各 crate、Tauri 入口、生命周期
└── frontend/             # React + TS + Vite
```

### 各 crate 详细职责

#### hc-core — 契约层

零依赖（仅 serde/log）。其它 crate 依赖它，它不依赖其它 crate。

```rust
// Source trait — 协议无关采集接口
#[async_trait]
pub trait Source: Send {
    async fn open(spec: &DeviceSpec) -> Result<Self, SourceError> where Self: Sized;
    async fn poll(&mut self, tags: &[TagSpec]) -> PollOutcome;
    async fn write(&mut self, tag: &TagSpec, value: Value) -> Result<(), SourceError>;
    async fn health(&mut self) -> SourceHealth;
}

// Hook trait — event-bus 扩展点
pub trait Hook: Send + Sync {
    fn before_publish(&self, dev: DeviceId, samples: &mut Vec<(TagId, Value)>) {}
    fn after_poll(&self, dev: DeviceId, status: &PollStatus) {}
    fn on_alarm(&self, evt: &AlarmEvent) {}       // SP3
    fn on_user_action(&self, action: &UserAction) {} // SP4
}

// 核心值类型
pub enum Value { U16(u16), I16(i16), U32(u32), I32(i32), F32(f32), Bool(bool) }
pub enum Quality { Good, Bad, Stale }

pub struct Sample {
    pub tag_id: TagId,
    pub ts: DateTime<Utc>,
    pub value: Value,
    pub quality: Quality,
}
```

`Event` 枚举：`PollSucceeded(DeviceId, Vec<(TagId, Value)>)` / `PollFailed(...)` / `ConnectionStateChanged(...)` 走 `event-bus`（`tokio::sync::broadcast`）。

#### hc-modbus — 协议适配器

`Source` 的内置实现。TCP 走 `tokio-modbus::tcp::client`，RTU 走 `rtu::client`（底层 `serialport`）。把 Modbus 寄存器读返回值按 `Tag` 元数据（类型/字节序/缩放）解码为 `Value`。解码测试矩阵覆盖 6 种 data_type × 4 种 byte_order × scale/offset — 32 用例。

- `PollRequest` 中 `protocol_params: serde_json::Value`（含 function_code / address / quantity / byte_order）。
- **零协议下穿**：前端永不接收 Modbus 寄存器号/功能码。

#### hc-storage — 存储层

两件事：
- **工程文件读写**：`Project` 树（project.toml + devices.toml + tags.toml + dashboards/*）用 `toml` + `serde` 反序列化校验。`load_project(path) -> Project` / `save_project(Project, path)`。
- **历史库**：`rusqlite` 打开工程目录内 history.db。订阅 event-bus 异步入库（背压队列），趋势查询 `query_trend(tag_ids, t0, t1, max_points) -> Vec<Series>`。

#### hc-runtime — 编排层

`DeviceRunner` 每个 `tokio::task` 跑一个设备轮询循环，按设备配置周期 `poll`。所有 Runner 由 `Runtime` 注册，`Runtime::start()` / `stop()`。

- event-bus（`tokio::sync::broadcast`）发 `PollSucceeded` 给三类订阅者：`hc-storage` 入库、`hc-ipc` 推前端、（SP3/SP4 预留位）。
- 钩子点位（SP1 内置空直通实现）：`before-publish(DeviceId, &mut Vec<(TagId, Value)>)` / `after-poll(DeviceId, &Status)`。
- 背压：入库慢时优先丢最旧未入库样本而非阻塞轮询。
- 失败重连：指数退避（最大 30s），实时上报 event-bus → ipc → 前端。

#### hc-ipc — Tauri 桥

`#[tauri::command]` 命令群：load_project / save_project / list_devices / upsert_device / remove_device / list_tags / upsert_tag / remove_tag / save_dashboard / list_dashboards / start_runtime / stop_runtime / runtime_status / write_tag / query_trend / read_logs。签名只依赖 `hc-core` / `hc-storage` / `hc-runtime` 暴露的领域类型。

Rust → 前端 events：
- `tag-update`：throttle 合并后 ~10 Hz 批量 emit
- `device-state`：事件驱动（去抖 500ms）
- `log`：节流后 ~5 Hz

**实时节流两层策略：**
- Rust 侧：订阅位过滤 → 100ms 批合并 → 差分过滤（值/quality 未变跳过）→ channel 满时丢最旧批。
- 前端侧：store `Map<TagId, CurrentValue>` O(1) 覆盖 → 仪表盘按 rAF 粒度消费 → 不单事件触 React re-render。

**类型同步**：Rust 型上 `#[derive(ts_rs::TS)]` 构建脚本生成 `frontend/src/ipc/bindings.ts`，前端 import；禁止手写同款。

#### hc-app — 装配

`main()`：logger / DI / 装配 Runtime + Storage + IpcState / 初始化 Tauri / 注册命令 / 加载最近工程 / 启动 runtime。

---

## 4. 前端模块图与 widget 注册表机制

### 模块拓扑

```
frontend/src/
├── app/
│   ├── ipc/           # 封装 invoke + 事件订阅；类型由 ts-rs 生成
│   ├── store/          # Zustand store：工程状态、运行态、连接状态
│   └── routes/         # 设备配置页 / 数据点页 / 仪表盘页 / 历史趋势页
├── config/             # 设备 / 数据点 / 工程管理 UI
├── runtime/            # 连接状态指示 / 日志窗口 / 运行控制
├── dashboard/          # 仪表盘运行时 + 编辑器
│   ├── grid/           # 网格布局引擎（react-grid-layout）
│   ├── renderer/       # 运行时：按布局渲染 widget、订阅更新、批量刷新
│   └── editor/         # 编辑时：组件选择、属性面板、数据点绑定、保存
├── widgets/            # widget 注册表 + 内置组件
│   ├── registry.ts     # 注册 API + WidgetManifest 类型
│   ├── built-in/       # MVP 5 个内置组件
│   │   ├── numeric-table/
│   │   ├── realtime-chart/     # lightweight-charts
│   │   ├── gauge/
│   │   ├── status-light/
│   │   └── bar-chart/
│   └── types.ts        # WidgetManifest / WidgetProps / WidgetConfig
├── history/            # 历史趋势查询 UI
└── ui-kit/            # 基础 UI 库（shadcn/ui）
```

### Widget 注册表机制

```ts
interface WidgetManifest {
  id: string;               // "hcs.builtin.gauge"
  name: string;
  category: "indicator" | "chart" | "table" | "control";
  configSchema: JSONSchema;  // 属性面板自动生成配置表单
  dataBinding: {
    minSources: number;
    maxSources: number;
    sourceRoleNames: string[];
  };
  defaultProps: { w: number; h: number };
  runtime: React.ComponentType<WidgetRuntimeProps>;
  editor?: React.ComponentType<WidgetEditorProps>;
}

class WidgetRegistry {
  register(manifest: WidgetManifest): void;
  get(id: string): WidgetManifest;
  list(category?: string): WidgetManifest[];
  unregister(id: string): void;  // SP4 外部包卸载
}
```

### 仪表盘布局格式（存 dashboards/*.toml）

```toml
[[widgets]]
id = "inst_1"
widget_id = "hcs.builtin.gauge"
layout = { x = 0, y = 0, w = 4, h = 3 }
[widgets.config]
title = "反应釜温度"
unit = "℃"
range = { min = 0, max = 100 }
[[widgets.bindings]]
role = 0
tag_id = "dev_1/tag_temp"
```

---

## 5. 数据模型与工程文件 schema

### 工程文件树

```
myplant.hcproj/
├── project.toml
├── devices.toml
├── tags.toml
├── dashboards/
│   ├── main.toml
│   └── ...
└── history.db
```

### project.toml

```toml
name = "反应釜监控"
version = 1
created = "2026-07-02T10:00:00"
[runtime]
default_poll_interval_ms = 1000
[storage]
history_sampling_ms = 1000
trend_max_points = 2000
```

### devices.toml

```toml
[[device]]
id = "dev_1"
name = "1号温控仪"
enabled = true
protocol = "modbus"
[device.transport]
type = "tcp"
host = "127.0.0.1"
port = 502
# type = "rtu" 时
# serial_port = "COM3"
# baud_rate = 9600
# data_bits = 8
# parity = "none"
# stop_bits = 1
[device.modbus]
slave_id = 1
timeout_ms = 1000
poll_interval_ms = 1000
byte_order = "abcd"
```

### tags.toml

```toml
[[tag]]
id = "dev_1/tag_temp"
device_id = "dev_1"
name = "反应釜温度"
enabled = true
[tag.modbus]
function = "read_holding"
address = 100
quantity = 2
writable = true
write_function = "write_single"
[tag.scaling]
data_type = "f32"
scale = 0.1
offset = 0.0
unit = "℃"
```

### SQLite schema

```sql
CREATE TABLE samples (
  tag_id TEXT NOT NULL,
  ts INTEGER NOT NULL,           -- Unix ms
  value BLOB NOT NULL,           -- serde_json 字符串
  quality INTEGER NOT NULL       -- 0=good,1=bad,2=stale
);
CREATE INDEX idx_tag_ts ON samples(tag_id, ts DESC);
-- 预建报警表（SP3 使用，MVP 不写入但 schema 预先建立）
CREATE TABLE IF NOT EXISTS alarms (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  tag_id TEXT NOT NULL,
  ts INTEGER NOT NULL,
  level INTEGER,
  condition TEXT,
  value BLOB,
  ack INTEGER DEFAULT 0
);
```

### 关键设计点

- **协议边界隔离**：Device / Tag 持 `protocol_params: serde_json::Value` 透传块，hc-modbus 自行解。SP4 新协议不动 hc-core。
- **类型履行**：前端 IPC 只收 `<tag_id, scaled_value, unit, quality>`，不感知 Modbus 细节。
- **schema 版本号**：`version = 1` 锁定 SP1；未来升级走迁移函数。
- **value 编码**：SP1 存 serde_json 字符串（千级点位毫秒级写入无压力）；SP3 长期归档可换二进制。

---

## 6. IPC 详表与实时节流策略

### Tauri commands（前端 → Rust）

| 命令 | 入参 | 返回 |
|---|---|---|
| `new_project(name)` | 工程名 | Project |
| `open_project(path)` | 目录路径 | Project |
| `save_project(project)` | Project | () |
| `list_devices()` | — | Vec<Device> |
| `upsert_device(device)` | Device | () |
| `remove_device(id)` | DeviceId | () |
| `list_tags(device_id?)` | 可选过滤 | Vec<Tag> |
| `upsert_tag(tag)` | Tag | () |
| `remove_tag(id)` | TagId | () |
| `save_dashboard(name, layout)` | name + 布局 | () |
| `list_dashboards()` | — | Vec<(name, layout)> |
| `start_runtime(device_ids?)` | 可选子集 | () |
| `stop_runtime()` | — | () |
| `runtime_status()` | — | RuntimeStatus |
| `write_tag(tag_id, value)` | tag_id + value | Result<(), Error> |
| `query_trend(tag_ids, t0, t1, max_points)` | 多 tag + 时间窗 | Vec<Series> |
| `read_logs(since_ts)` | 时间戳 | Vec<LogEntry> |

### Tauri events（Rust → 前端）

| 事件 | Payload | 触发 | 频率 |
|---|---|---|---|
| `tag-update` | `Vec<TagUpdate>` | 采集后入 bus | ~10 Hz |
| `device-state` | `DeviceState` | 连接变化 | 去抖 500ms |
| `log` | `LogEntry` | 日志产生 | ~5 Hz |

### 实时节流策略

**Rust 侧（主防线）：**
1. 前端发 `subscribe_updates(tag_ids)` → ipc 只处理关注集
2. 100ms 批合并 → 差分过滤（值/quality 未变跳过）→ channel 满时（>64 条）丢最旧批
3. 计数 ipc_dropped 上报 log 事件

**前端侧：**
- store 用 `Map<TagId, CurrentValue>` O(1) 覆盖
- 仪表盘按 rAF 粒度消费 store；lightweight-charts 用内置 `update` 推 tick

### 类型同步

Rust 型上加 `#[derive(Serialize, Deserialize, ts_rs::TS)]`，构建脚本生成 `frontend/src/ipc/bindings.ts`，**禁止**前端手写同款。

### 错误传播

Rust command 返回 `Result<T, IpcError>`(`{code, message, context}`)，前端 ipc 统一捕获 toast + log。采集异常走 `device-state` / `log` event 而非 command 返回值（异步发生）。

---

## 7. 扩展点契约详表与 SP4 接轨方式

SP1 不实装动态加载，但每个扩展点的接口契约在 SP1 就固化，SP4 只"兑现实现"不"改契约"。

### 扩展点 1：协议层（Source trait）

SP1：`hc-modbus::ModbusSource` 静态编译进 hc-app。SP4：`hc-plugin-host` 用 `libloading` 加载第三方 dylib，在 host 层把 C ABI 翻译成 `dyn Source`。**trait 字段从 SP1 起冻结**，新增字段走默认实现/版本枚举。

### 扩展点 2：UI 层（widget manifest）

§4 已详述。SP1 内置 5 组件以注册表项存在；SP4 引入 `widget-package/` 目录约定 + 动态 import + 沙箱。`WidgetManifest` 已预留 `version` / `permissions` / `runtimeEntry` 字段。

### 扩展点 3：逻辑层（Hook trait）

`Runtime::hooks: Vec<Box<dyn Hook>>` SP1 默认空 vec，内置 NoOpHook。SP3/SP4 push 真实实现——不改 Runtime 主体。

### 扩展点 4：协议参数块（protocol_params）

serde_json Value 透传，SP4 新协议只需新 Source impl + 配置页注册协议参数表单 schema，不改 hc-core 类型。

### 升级稳定约束

- trait / manifest 字段从 SP1 起冻结，破坏性改必走 protocol_params / 扩展字段
- 工程文件 `version = 1` 锁定 SP1，未来升级走 hc-storage 迁移

---

## 8. 测试与验证策略

### 分层测试

| 层 | 工具 | 范围 |
|---|---|---|
| Rust 单元 | `cargo test` | hc-core 数据模型、hc-modbus 解码矩阵、hc-storage 序列化往返 |
| Rust 集成 | `cargo test` + tokio::test | hc-runtime 并发（FakeSource）、event-bus、背压、hc-ipc 节流 |
| Mock Source | `FakeSource` impl Source trait | 不依赖真机即跑全链路 |
| 前端单元 | Vitest | registry、store、ipc 类型、组件渲染 |
| 端到端 | 手动 + pymodbus.server | Tauri exe 连模拟器跑通主线 |

### hc-modbus 解码测试矩阵

6 种 data_type × 4 种 byte_order × scale/offset → 32 用例 + 异常用例（quantity 不足、超时、断开）。

### MVP 验收剧本

1. 起 `pymodbus.server` 模拟设备（holding 100-103 为 float, coil 200 为 bool）
2. 新建工程 → 添加 TCP 设备
3. 加 2 个 tag（f32 寄存器 + bool coil）
4. 建仪表盘：gauge + status-light
5. 启动 runtime → 1s 实时更新
6. write_tag → 确认模拟器侧写入
7. query_trend → 趋势曲线出现
8. 断网 → 状态红 + 日志报错 → 恢复后自动重连

### 非功能验证

50 设备 × 20 点 × 1s 轮询下 CPU < 15%（单核）、内存 < 200MB、SQLite 入库不积压超 2s。

### 不做（SP1 跳过）

自动化 E2E（Playwright 驱 Tauri）、大规模压力测试。

---

## 9. 任务级验证工作流（每步独立测试 + 关键任务 review）

### 闸门模式

```
任务实现 → ① 测试（所有任务强制）→ ② code review（仅关键任务 + 整体完成）→ 下一任务
```

**① 测试 — 所有任务强制**
- 每个任务实现后跑该任务相关测试（`cargo test <模块>` / `npm run test <文件>`）+ clippy/lint
- 不通过 → `systematic-debugging` skill 定位根因 → 修复 → 重测
- 测试不绿不进入 review

**② Code review — 仅关键任务 + 整体完成**
- 关键任务派 reviewer subagent 独立审查：
  - **#1** hc-core trait + 数据模型
  - **#2** hc-modbus 解码矩阵
  - **#6** hc-runtime 多设备并发 + event-bus + 背压
  - **#7** hc-ipc 节流/合并 + 类型绑定
  - **#10** 前端 widget registry + 内置组件契约
- 非关键任务只跑测试即放行
- 全部任务完成后派整体 review（跨 crate 一致性、零协议下穿、spec 兑现度）

### 任务粒度（12 项）

| # | 任务 | 关键 | 测试范围 |
|---|---|---|---|
| 1 | hc-core 数据模型 + Source/Hook trait | ⭐ | 单元 |
| 2 | hc-modbus 适配器 + 32 解码用例 | ⭐ | 单元 + 矩阵 |
| 3 | hc-storage 工程文件 toml 往返 + schema 版本 | | 单元 |
| 4 | hc-storage SQLite 读写 + 趋势查询降采样 | | 单元 + 集成 |
| 5 | hc-runtime 单设备轮询 + 重连（FakeSource） | | 集成 |
| 6 | hc-runtime 多设备并发 + event-bus + 背压 | ⭐ | 集成 |
| 7 | hc-ipc Tauri 命令/事件 + 节流 + 类型绑定 | ⭐ | 集成 |
| 8 | hc-app 装配 + Tauri 入口 + 生命周期 | | 集成 |
| 9 | 前端 ipc 客户端 + store + bindings.ts 消费 | | 单元 |
| 10 | 前端 widget registry + 5 内置组件 | ⭐ | 单元 + 组件 |
| 11 | 前端仪表盘 + 趋势页 + 配置页 + 状态页 | | 组件 |
| 12 | 端到端 pymodbus 验收 + 性能基线 | | 手动 |
| — | 整体完成 review | ⭐ | — |

### 闸门不通过的处理

- reviewer 提"必须修改"清单 → 同一任务内修复 → 再派 reviewer → 通过为止
- 测试失败 → `systematic-debugging` 定位根因 → 修复 → 重测
- 同一任务连续 3 轮未过 → 升级对齐用户（契约本身可能有问题）

---

## 10. CI/构建策略

### 本地 vs CI 分工

- **本地**：实现每个任务时跑测试 + clippy/lint（§9 闸门①）
- **GitHub Actions**：整体完成构建 + 全量测试，不在本地跑全量。触发：push / PR

### GitHub Actions 工作流

```yaml
jobs:
  rust:
    runs-on: windows-latest
    steps:
      - checkout
      - rustup toolchain stable
      - cargo fmt --check
      - cargo clippy --all-targets -- -D warnings
      - cargo test --workspace
  frontend:
    runs-on: windows-latest
    steps:
      - checkout
      - node setup
      - npm ci
      - npm run typecheck
      - npm run lint
      - npm run test
  build:
    needs: [rust, frontend]
    runs-on: windows-latest
    steps:
      - cargo tauri build   # 产出 .exe/.msi
```

### 关键点

- Windows runner（与目标 OS 一致），避免 Linux 上 RTU/串口/WebView2 行为偏差
- `cargo tauri build` 放最后（依赖前两段绿）
- CI 不跑 pymodbus 端到端（端到端只在本地手动验收剧本）

---

## 附录：技术选型摘要

| 领域 | 选型 | 理由 |
|---|---|---|
| 桌面框架 | Tauri 2.x | 小体积、安全、Rust 原生 |
| Modbus 通信 | tokio-modbus (slowtec) | Rust 生态最成熟 async Modbus 库 |
| 串口 | serialport | Tauri 2.x 推荐，Windows 兼容好 |
| 序列化 | serde + toml | 工程文件格式 |
| 历史存储 | rusqlite + SQLite | 零依赖嵌入，千级无压力 |
| 前端框架 | React 18 + TypeScript | 生态最广，组态/可视化资源丰富 |
| 构建 | Vite | Tauri 默认推荐 |
| 布局引擎 | react-grid-layout | 仪表盘拖拽布局 |
| 实时曲线 | lightweight-charts | canvas 渲染，万点不卡 |
| UI 组件 | shadcn/ui | 小而精，unstyled 底层 |
| 状态管理 | Zustand | 轻量、无 Redux 模板、TS 友好 |
| 类型生成 | ts-rs | Rust 类型 → TS 绑定自动生成 |

---

*本文档对应 Sub-Project #1（MVP）设计。SP2/SP3/SP4 将在各自阶段形成独立设计规格书。*

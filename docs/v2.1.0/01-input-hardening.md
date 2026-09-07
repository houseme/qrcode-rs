# v2.1.0 子任务 01 — 输入验证与运行时加固

> 父计划: `docs/v2.1.0-implementation-plan.md`。规格 §1.1 + §4.1 + §4.2。

## 目标

消除未定义行为与资源耗尽:审计/移除 `unsafe`、整数溢出保护、缓冲区边界、资源限制、确定性。

## 方案

1. **`unsafe` 审计**: 全 workspace `grep unsafe`;每处保留须有 SAFETY 注释证明。重点:v2.0 SIMD intrinsics(`std::arch::x86_64`)、cast trait。
2. **整数安全**: 算术用 `checked_*`/`saturating_*`/`usize::try_from`;新增 `QrError::NumericOverflow`(或复用 `InvalidVersion`)承载转换失败。
3. **边界**: 切片索引全部经范围检查;`find_min_version`/canvas 坐标 clamp。
4. **资源限制**(§4.1):
   ```rust
   pub struct ResourceLimits {
       pub max_data_length: usize,
       pub max_version: Version,
       pub max_render_size: (u32,u32),
       pub encoding_timeout: Option<u64>,
   }
   impl QrCode { pub fn with_limits(data: &[u8], limits: ResourceLimits) -> QrResult<Self> }
   ```
   默认:max_data=7089(V40 Byte)、max_version=V40、render≤4096²。
5. **确定性**(§4.2): 掩码选择已是确定性(无 RNG);消除任何全局/线程局部状态;`deterministic` feature 显式断言(同输入→字节级同输出)。

## 影响文件

- 各 crate 的 `unsafe` 点;`types.rs`(错误变体);`qrcode-core/src/limits.rs`(新增 ResourceLimits)。
- CI:`cargo miri test`、`cargo test` + ASAN。

## 实施步骤

1. `unsafe` 审计清单 + SAFETY 注释;移除非必要 unsafe。
2. 整数安全:`as` 转换→`try_from`;算术→checked;单测覆盖溢出。
3. `ResourceLimits` + `with_limits`;超限返回 `DataTooLong`/新错误。
4. 确定性:全局状态审计;`deterministic` feature + 跨平台字节级一致测试。
5. CI 加 miri + ASAN job。

## 验收

- `cargo miri test --workspace` 全绿。
- ASAN/fuzz 无越界/溢出/UB。
- `with_limits` 超限明确报错、开销可忽略。
- 同输入在 linux/macos/wasm32 字节级同输出。

## 风险

- miri 对 SIMD intrinsics 支持有限;用标量回退路径跑 miri,SIMD 路径单独 ASAN。
- 跨平台确定性受浮点/endianness 影响;掩码评分用整数运算避免浮点。

## 当前实施状态

已落地 `qrcode_core::ResourceLimits`、`QrCode::with_limits`、默认输入长度上限、最大版本/模块尺寸检查、同步构造 `encoding_timeout` 软预算检查、显式 `deterministic` feature/API，以及 Structured Append alphanumeric 越界返回 `MalformedStream` 的回归测试。`encoding_timeout` 在构造阶段的边界点检查耗时,不承诺抢占式取消;Miri、ASAN 和跨平台字节级复现仍由 CI/专用主机负责。

# v2.1.0 子任务 02 — 模糊与属性测试

> 父计划: `docs/v2.1.0-implementation-plan.md`。规格 §1.2 + §1.3。依赖子任务 01。

## 目标

全入口点模糊覆盖 + 安全/不变量属性测试;OSS-Fuzz 持续运行。

## 方案

### 模糊 targets(`fuzz/`,cargo-fuzz)
- `encode`(任意 `&[u8]` → `QrCode::new`,不 panic)。
- `encode_with_version`(data + version + ec,合法组合)。
- `render_svg`/`render_image`(encode→render,不 panic)。
- `parse_wifi`/`parse_vcard`/`parse_gs1`(畸形输入)。
- `structured_append`(data + symbols 2..16 → encode→reassemble 一致)。
- `decode_sa_parse`(任意字节流 → `parse_sa_datastream`,不 panic)。

### 属性测试(`tests/property.rs`,proptest)
- `no_panic_on_any_input`、`output_size_bounded`(width ∈ [11,177])、`memory_bounded`(colors.len() ≤ 177²)。
- `version_auto_minimal`(自动选最小版本)、`structured_append_roundtrip`。

### OSS-Fuzz
- `projects/qrcode-rs/` 集成;Dockerfile + build.sh;持续 24/7。

## 彞响文件

- `fuzz/Cargo.toml` + `fuzz/fuzz_targets/*.rs`(新增)。
- `tests/property.rs`(新增)。
- `projects/qrcode-rs/`(OSS-Fuzz upstream,或 CI 短时 fuzz)。

## 实施步骤

1. cargo-fuzz 工程初始化;6 个 target;本地跑 ≥1h 无崩溃。
2. proptest 属性测试;CI 跑(随机种子固定)。
3. ASAN/UBSAN 配置;CI fuzz job(定时短跑)。
4. OSS-Fuzz 集成 PR(或文档化 CI fuzz)。

## 验收

- fuzz ≥72h 无崩溃/ASAN 违例。
- 属性测试覆盖核心不变量,固定种子可复现。
- OSS-Fuzz 集成(或等价 CI fuzz)运行。

## 风险

- 模糊发现既有 bug 需回修(预期);预留修复窗口。
- OSS-Fuzz upstream 审批周期;先用 CI fuzz 过渡。

## 当前实施状态

已新增 `render_image`、`decode_sa_parse` 两个 target，并保留原有五个 target；属性测试覆盖任意输入不 panic、输出边界、最小版本和 SA parser。Build 与定时 workflow 使用固定 seed 的短时 smoke，不能替代 72 小时 ASAN/UBSAN 或 OSS-Fuzz 运行。

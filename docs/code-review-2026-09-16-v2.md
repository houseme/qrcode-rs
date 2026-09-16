# 全面代码审查报告 V2 — qrcode-rs v2.1.0

- **审查日期**：2026-09-16
- **V2 范围**：复核 `docs/code-review-2026-09-16.md` 中的 Critical/Major 结论，并对本轮确认的高优先级问题完成首批修复。
- **基线**：`main` 工作树复核后进入修复；Edition 2024，MSRV 1.85。
- **V2 结论**：V1 的 Critical #1/#2/#3/#4 与 Major #11/#19 经源码复核成立。本轮已修复这些项目的核心可达路径；其余 Major/Minor 仍保留为待办，未在 V2 中重新发现新问题。

---

## V2 复核摘要

V1 对工程整体质量的判断仍成立：核心 crate 拆分清晰，`no_std + alloc` 边界明确，测试/fuzz/示例覆盖面较宽。但 V1 中若干阻塞项并非单纯文档问题，而是可达的互操作性、安全输出与资源边界缺陷。

本轮按风险优先级完成以下修复：

1. **Structured Append 互操作性**：编码端改为 ISO/IEC 18004 规定的 `position - 1` / `total - 1` nibbles；解析端同步按 nibble + 1 还原，并拒绝 `total < 2` 或 `position > total` 的头。
2. **SVG/HTML 注入**：颜色与注入属性值均做实体转义；属性名只接受安全的 ASCII XML/HTML 属性名；SVG `aria_label` 和颜色 payload 增加回归测试。
3. **SVG animation XML 结构**：`animate()` 改为定位根 `<svg>` 标签后插入 `<style>`，避免把 style 放到 XML 声明之后。
4. **PNG 渲染资源边界**：`qrcode-render` 增加 `try_build()` 与 `RenderError::OutputTooLarge`；CLI 单码/批量 PNG 渲染在分配前校验最终边长和像素预算。
5. **CLI 颜色边界**：SVG/HTML 输出也统一要求 `#rgb` / `#rrggbb`，避免命令行直达不受控 CSS/XML/HTML 字符串。
6. **CI feature 门槛**：Build workflow 增加 `cargo test --all-features` 与 `cargo test -p qrcode-cli`，让已存在的 feature-gated 测试进入 CI。

---

## V2 修复状态

### 已修复

1. **Critical #1 Structured Append 头编码错误**
   - 修复文件：`crates/qrcode-core/src/bits.rs`、`crates/qrcode-decode/src/sa_parse.rs`、`src/structured_append.rs`
   - 验证：新增/更新 header byte-vector、16-symbol 边界和 malformed header 测试。

2. **Critical #2 SVG 输出注入**
   - 修复文件：`crates/qrcode-svg/src/lib.rs`
   - 验证：新增颜色转义、aria-label 转义、非法属性名跳过测试。

3. **Critical #3 HTML 输出注入**
   - 修复文件：`crates/qrcode-html/src/lib.rs`
   - 验证：新增颜色转义、aria-label 转义、非法属性名跳过测试。

4. **Critical #4 CLI `--size` 超大值导致 panic/abort**
   - 修复文件：`crates/qrcode-cli/src/main.rs`、`src/bin/qrencodes.rs`、`crates/qrcode-render/src/lib.rs`
   - 验证：CLI 单码 PNG、helper PNG 路径、renderer `try_build()` 均有回归测试。

5. **Major #11 SVG animate 插入位置错误**
   - 修复文件：`crates/qrcode-svg/src/lib.rs`
   - 验证：测试定位真实 `<svg>` 开标签结束位置，确认 `<style>` 位于根元素内。

6. **Major #19 feature-gated 测试未进 CI**
   - 修复文件：`.github/workflows/Build.yml`
   - 验证：workflow 增加 all-feature test 和 split `qrcode-cli` test 步骤。

### 仍待办

V1 中其余 Major/Minor 尚未在本轮修复，仍建议按风险排序继续处理：

- forced numeric/alphanumeric/kanji 手写编码路径的输入校验；
- Micro 半码字空数据、mask penalty `u16` 累加、公开 API panic 文档/checked 入口；
- EPS/PDF 输出几何与背景一致性；
- decode/rqrr 多 grid 错误处理与 GrayPixels 尺寸校验；
- batch 错误上下文、双 CLI 维护源和 ZIP 临时文件原子写；
- fuzz 语义断言与 Structured Append 真正编码输出解析覆盖。

---

## 验证记录

本轮已执行并通过：

- `cargo fmt --all -- --check`
- `cargo check --all-targets`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --all-targets`
- `cargo test --all-features`
- `cargo test -p qrcode-core structured_append_tests`
- `cargo test -p qrcode-decode sa_parse`
- `cargo test -p qrcode-svg`
- `cargo test -p qrcode-html`
- `cargo test -p qrcode-render try_build_rejects_overflowing_dimensions`
- `cargo test -p qrcode-cli`
- `cargo test --features cli structured_append`
- `cargo test --features cli run_rejects_invalid_svg_color`
- `cargo test --features cli run_rejects_oversized_png_size`

---

## V2 Verdict

**Ready：With fixes for V1 Critical #1-#4 and Major #11/#19.**

V2 不声称整个 V1 backlog 已清零；本轮把最容易造成互操作性损坏、安全输出注入、进程 abort 和 CI 漏测的路径先关上。剩余项应继续拆成后续功能 commit，而不是混入本轮高风险修复提交。

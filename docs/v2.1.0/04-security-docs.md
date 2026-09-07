# v2.1.0 子任务 04 — 安全文档

> 父计划: `docs/v2.1.0-implementation-plan.md`。规格 §3。收尾。

## 目标

公开安全策略、漏洞响应流程、审计报告。

## 方案

1. **`SECURITY.md`**(§3.1): 支持版本表(2.x 安全更新/1.x 仅关键修复/0.x 不支持)、私密报告渠道(GitHub Security Advisories)、SLA(48h 确认/7d 关键修复)、最佳实践。
2. **审计报告**(§3.2): `docs/security-audit.md` 记录每主版本前内部审计发现与修复;公开摘要。
3. **CONTRIBUTING 安全段**: 贡献者安全准则(unsafe 规范、fuzz 贡献)。

## 影响文件

- `SECURITY.md`(新增,根目录)、`docs/security-audit.md`(新增)。
- `CONTRIBUTING.md`(安全段)。
- README 安全徽章(`cargo deny`/audit 覆盖)。

## 实施步骤

1. `SECURITY.md`(版本支持/报告/SLA/最佳实践)。
2. `docs/security-audit.md`(v2.0/v2.1 审计摘要:unsafe 清单/溢出修复/fuzz 发现)。
3. CONTRIBUTING 安全段 + README 徽章。
4. 启用 GitHub Security Advisories(仓库设置)。

## 当前实施状态

- [x] 根目录 `SECURITY.md`：支持版本、私密报告入口、响应目标、范围和
  最佳实践。
- [x] `docs/security-audit.md`：记录 v2.0 安全边界、SIMD `unsafe` 审查范围、
  几何/尺寸溢出加固、属性/差分测试与 fuzz 证据，并明确未验证边界。
- [x] `CONTRIBUTING.md`：增加安全敏感改动、依赖变更、fuzz 与公开信息处理准则。
- [x] `README.md`：增加 RustSec 审计 workflow 徽章和安全说明。
- [ ] GitHub Security Advisories：仓库设置项需由有权限的维护者在 GitHub
  中确认启用；本地代码与文档无法证明该设置已开启。

## 验收

- `SECURITY.md` 完整、响应流程可执行。
- 审计报告记录本轮发现与修复。
- GitHub Security Advisories 启用。

## 风险

- SLA 承诺须能兑现;按维护者实际带宽设定。
- 审计报告披露须避免暴露未修复问题;已修复项才公开摘要。

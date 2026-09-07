# v2.1.0 子任务 03 — 供应链安全

> 父计划: `docs/v2.1.0-implementation-plan.md`。规格 §2.1-2.4。

## 目标

CI 自动化依赖审计、SBOM 生成、发布签名、依赖最小化。

## 方案

1. **`cargo deny`**(§2.1): `deny.toml` — 漏洞 deny、yanked deny、许可证白名单(MIT/Apache-2.0/BSD/ISC)、多版本 warn。CI 门禁。
2. **`cargo vet`**(§2.1): 审计第三方代码变更;`supply-chain/` 目录。
3. **SBOM**(§2.2): `cargo-cyclonedx` 生成 CycloneDX(`sbom.xml`/`.json`);`cargo auditable` 嵌入依赖信息到二进制;Release 附 SBOM。
4. **签名**(§2.3): Sigstore(cosign)签名 `.crate`;GitHub Artifact Attestation(`actions/attest-build-provenance`);README 验证方法。
5. **依赖最小化**(§2.4): `minimal` feature(零外部依赖,仅 svg+string);审计每个 dep 必要性。

## 影响文件

- `deny.toml`(新增)、`supply-chain/`(vet)、CI workflows(`security.yml`)。
- `Cargo.toml`(`minimal` feature、`auditable`)。
- Release workflow(SBOM + 签名)。

## 实施步骤

1. `deny.toml` + CI `cargo deny check`;修现有建议(多版本/yanked)。
2. `cargo vet` 初始化 + 初始审计(trusted imports)。
3. `cargo-cyclonedx` + `cargo auditable`;Release 附 SBOM。
4. Sigstore/GitHub Attestation 签名;README 验证段。
5. `minimal` feature;`cargo tree -F minimal` 零外部依赖。

## 验收

- CI `cargo deny check`/`cargo vet` 全绿。
- Release 含 SBOM(CycloneDX)+ 可验证签名。
- `minimal` feature 零外部依赖(`cargo tree` 空)。

## 风险

- `cargo vet` 初始工作量大(审计传递依赖);分批 trusted。
- 签名密钥/凭证管理;用 OIDC(keyless Sigstore/GitHub Attestation)避免长寿命密钥。

## 当前实施状态

已新增 `deny.toml`、`supply-chain/` 审计基线、security workflow，以及带 SBOM、auditable CLI、Sigstore 和 provenance attestation 的 release workflow；`minimal` feature 已加入并验证仅选择 workspace crate。本地 `cargo vet check` 已在锁定基线上通过；在线 advisory 数据、SBOM 发布和签名结果仍需在 GitHub runner/release tag 上复核。

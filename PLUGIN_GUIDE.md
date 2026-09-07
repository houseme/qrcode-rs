# qrcode-rs 插件扩展

v2.0 的插件注册表是显式、局部状态，不会修改全局状态。应用先创建
`PluginRegistry`，再注册插件并把注册表传给 `QrCode::render_with`。

```rust
use qrcode_rs::{PluginRegistry, QrCode, RenderConfig};
use qrcode_render::plugin::PlainTextRendererPlugin;

let mut registry = PluginRegistry::new();
PlainTextRendererPlugin.register(&mut registry);
let code = QrCode::new("hello")?;
let text = code.render_with(
    &registry,
    PlainTextRendererPlugin::RENDERER_NAME,
    &RenderConfig::new().with_option("dark", "#"),
)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

第三方插件实现 `QrPlugin`，在 `register` 中注入一个或多个
`RendererFactory`、`EncoderFactory` 或 `PostProcessor`。动态 renderer 的
输出使用 `RenderOutput`，编码器使用 `EncodedOutput`；这些对象安全 trait
都只接收借用的 `ModuleSource` 或字节切片。

factory 可以实现 `RendererFactory::validate_config`，在构造前拒绝无效配置，
注册表会把错误作为 `PluginError::InvalidConfig` 返回。后处理器按注册顺序
运行，并应通过 `ModuleStorage` 的边界访问模块，不依赖全局可变状态。

插件名称和版本由 `QrPlugin::name`、`QrPlugin::version` 提供；建议使用稳定
的 crate 名称作为 name，并以 `CARGO_PKG_VERSION` 作为 version。

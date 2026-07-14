//! Built-in renderer plugins backed by `qrcode-render`.

use crate::Renderer;
use alloc::boxed::Box;
use qrcode_core::{
    DynRenderer, ModuleSource, PluginError, PluginRegistry, QrPlugin, RenderConfig, RenderOutput, RendererFactory,
};

/// Built-in plugin that registers the plain-text renderer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlainTextRendererPlugin;

impl PlainTextRendererPlugin {
    /// Renderer name registered by this plugin.
    pub const RENDERER_NAME: &'static str = "plain-text";
}

impl QrPlugin for PlainTextRendererPlugin {
    fn name(&self) -> &str {
        "qrcode-render/plain-text"
    }

    fn version(&self) -> &str {
        env!("CARGO_PKG_VERSION")
    }

    fn register(&self, registry: &mut PluginRegistry) {
        registry.register_renderer(Self::RENDERER_NAME, Box::new(PlainTextRendererFactory));
    }
}

/// Factory for [`PlainTextRenderer`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlainTextRendererFactory;

impl RendererFactory for PlainTextRendererFactory {
    fn build(&self, config: &RenderConfig) -> Box<dyn DynRenderer> {
        let dark = config_char(config, "dark", '#');
        let light = config_char(config, "light", ' ');
        let quiet_zone = config_u32(config, "quiet_zone", 4);
        Box::new(PlainTextRenderer { dark, light, quiet_zone })
    }
}

/// Object-safe plain-text renderer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlainTextRenderer {
    dark: char,
    light: char,
    quiet_zone: u32,
}

impl DynRenderer for PlainTextRenderer {
    fn render(&self, code: &dyn ModuleSource) -> Result<RenderOutput, PluginError> {
        let output = Renderer::<char>::from_source(code, self.quiet_zone)
            .dark_color(self.dark)
            .light_color(self.light)
            .module_dimensions(1, 1)
            .build();
        Ok(RenderOutput::Text(output))
    }
}

fn config_char(config: &RenderConfig, key: &str, default: char) -> char {
    config.option(key).and_then(|value| value.chars().next()).unwrap_or(default)
}

fn config_u32(config: &RenderConfig, key: &str, default: u32) -> u32 {
    config.option(key).and_then(|value| value.parse().ok()).unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::{PlainTextRendererFactory, PlainTextRendererPlugin};
    use qrcode_core::{Color, ModuleGrid, PluginRegistry, QrPlugin, RenderConfig, RenderOutput, RendererFactory};

    #[test]
    fn plugin_registers_plain_text_renderer() {
        let mut registry = PluginRegistry::new();
        PlainTextRendererPlugin.register(&mut registry);

        assert!(registry.renderer(PlainTextRendererPlugin::RENDERER_NAME).is_some());
    }

    #[test]
    fn plain_text_renderer_uses_configured_colors_and_quiet_zone() {
        let modules = ModuleGrid::new(alloc::vec![Color::Dark, Color::Light, Color::Light, Color::Dark], 2, 2).unwrap();
        let renderer = PlainTextRendererFactory.build(
            &RenderConfig::new().with_option("dark", "X").with_option("light", ".").with_option("quiet_zone", "0"),
        );

        assert_eq!(renderer.render(&modules).unwrap(), RenderOutput::Text("X.\n.X".into()));
    }
}

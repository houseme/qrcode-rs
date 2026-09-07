//! Built-in renderer plugins backed by `qrcode-render`.

use alloc::{boxed::Box, string::String};
use qrcode_core::{
    Color, DynRenderer, ModuleSource, ModuleStorage, PluginError, PluginRegistry, PostProcessor, QrPlugin,
    RenderConfig, RenderOutput, RendererFactory,
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

/// Built-in plugin that registers [`InvertModulesPostProcessor`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InvertModulesPlugin;

impl QrPlugin for InvertModulesPlugin {
    fn name(&self) -> &str {
        "qrcode-render/invert-modules"
    }

    fn version(&self) -> &str {
        env!("CARGO_PKG_VERSION")
    }

    fn register(&self, registry: &mut PluginRegistry) {
        registry.register_postprocessor(Box::new(InvertModulesPostProcessor));
    }
}

/// Postprocessor that flips every module from dark to light, or light to dark.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InvertModulesPostProcessor;

impl PostProcessor for InvertModulesPostProcessor {
    fn process(&self, modules: &mut dyn ModuleStorage) -> Result<(), PluginError> {
        for y in 0..modules.height() {
            for x in 0..modules.width() {
                modules.set(x, y, !modules.get(x, y));
            }
        }
        Ok(())
    }
}

/// Factory for [`PlainTextRenderer`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlainTextRendererFactory;

impl RendererFactory for PlainTextRendererFactory {
    fn build(&self, config: &RenderConfig) -> Box<dyn DynRenderer> {
        let config_valid = self.validate_config(config).is_ok();
        let dark = config_char(config, "dark", '#').unwrap_or('#');
        let light = config_char(config, "light", ' ').unwrap_or(' ');
        let quiet_zone = config_u32(config, "quiet_zone", 4).unwrap_or(4);
        Box::new(PlainTextRenderer { dark, light, quiet_zone, config_valid })
    }

    fn validate_config(&self, config: &RenderConfig) -> Result<(), PluginError> {
        config_char(config, "dark", '#')?;
        config_char(config, "light", ' ')?;
        config_u32(config, "quiet_zone", 4)?;
        Ok(())
    }
}

/// Object-safe plain-text renderer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlainTextRenderer {
    dark: char,
    light: char,
    quiet_zone: u32,
    config_valid: bool,
}

impl DynRenderer for PlainTextRenderer {
    fn render(&self, code: &dyn ModuleSource) -> Result<RenderOutput, PluginError> {
        qrcode_core::Renderer::render(self, code).map(RenderOutput::Text)
    }
}

impl<Code> qrcode_core::Renderer<Code> for PlainTextRenderer
where
    Code: ModuleSource + ?Sized,
{
    type Output = String;
    type Error = PluginError;

    fn render(&self, code: &Code) -> Result<Self::Output, Self::Error> {
        if !self.config_valid {
            return Err(PluginError::InvalidConfig("plain-text renderer configuration is invalid".into()));
        }
        validate_module_source(code)?;
        render_plain_text(code, self.dark, self.light, self.quiet_zone)
    }
}

fn validate_module_source<Code>(code: &Code) -> Result<(), PluginError>
where
    Code: ModuleSource + ?Sized,
{
    let width = code.width();
    let height = code.height();
    match width.checked_mul(height) {
        Some(len) if width != 0 && width == height && len == code.modules().len() => Ok(()),
        _ => Err(PluginError::InvalidModuleGrid),
    }
}

fn render_plain_text<Code>(code: &Code, dark: char, light: char, quiet_zone: u32) -> Result<String, PluginError>
where
    Code: ModuleSource + ?Sized,
{
    let width = code.width();
    let quiet_zone = usize::try_from(quiet_zone)
        .map_err(|_| PluginError::InvalidConfig("quiet_zone does not fit in platform dimensions".into()))?;
    let border =
        quiet_zone.checked_mul(2).ok_or_else(|| PluginError::InvalidConfig("quiet_zone dimensions overflow".into()))?;
    let total_width = width
        .checked_add(border)
        .ok_or_else(|| PluginError::InvalidConfig("plain-text output dimensions overflow".into()))?;
    let module_end = quiet_zone
        .checked_add(width)
        .ok_or_else(|| PluginError::InvalidConfig("plain-text module dimensions overflow".into()))?;
    let capacity = total_width
        .checked_mul(total_width)
        .and_then(|area| area.checked_add(total_width.saturating_sub(1)))
        .ok_or_else(|| PluginError::InvalidConfig("plain-text output size overflow".into()))?;
    let mut output = String::with_capacity(capacity);

    for y in 0..total_width {
        if y > 0 {
            output.push('\n');
        }

        let row = (quiet_zone..module_end).contains(&y).then(|| code.row(y - quiet_zone));
        for x in 0..total_width {
            let color =
                row.filter(|_| (quiet_zone..module_end).contains(&x)).map_or(Color::Light, |row| row[x - quiet_zone]);
            output.push(color.select(dark, light));
        }
    }

    Ok(output)
}

fn config_char(config: &RenderConfig, key: &str, default: char) -> Result<char, PluginError> {
    let Some(value) = config.option(key) else {
        return Ok(default);
    };
    let mut chars = value.chars();
    let Some(character) = chars.next() else {
        return Err(PluginError::InvalidConfig(alloc::format!("{key} must contain exactly one character")));
    };
    if chars.next().is_some() {
        return Err(PluginError::InvalidConfig(alloc::format!("{key} must contain exactly one character")));
    }
    Ok(character)
}

fn config_u32(config: &RenderConfig, key: &str, default: u32) -> Result<u32, PluginError> {
    config.option(key).map_or(Ok(default), |value| {
        value.parse().map_err(|_| PluginError::InvalidConfig(alloc::format!("{key} must be a valid u32")))
    })
}

#[cfg(test)]
mod tests {
    use super::{InvertModulesPlugin, InvertModulesPostProcessor, PlainTextRendererFactory, PlainTextRendererPlugin};
    use qrcode_core::{
        Color, ModuleGrid, ModuleSource, PluginError, PluginRegistry, PostProcessor, QrPlugin, RenderConfig,
        RenderOutput, Renderer as CoreRenderer, RendererFactory,
    };

    struct BadSource {
        modules: [Color; 4],
    }

    impl ModuleSource for BadSource {
        fn get(&self, x: usize, y: usize) -> Color {
            self.modules[y * self.width() + x]
        }

        fn width(&self) -> usize {
            3
        }

        fn height(&self) -> usize {
            2
        }

        fn modules(&self) -> &[Color] {
            &self.modules
        }
    }

    #[test]
    fn plugin_registers_plain_text_renderer() {
        let mut registry = PluginRegistry::new();
        PlainTextRendererPlugin.register(&mut registry);

        assert!(registry.renderer(PlainTextRendererPlugin::RENDERER_NAME).is_some());
    }

    #[test]
    fn invert_modules_plugin_registers_postprocessor() {
        let mut registry = PluginRegistry::new();
        InvertModulesPlugin.register(&mut registry);

        assert_eq!(registry.postprocessors().len(), 1);
    }

    #[test]
    fn invert_modules_postprocessor_flips_all_modules() {
        let mut modules =
            ModuleGrid::new(alloc::vec![Color::Dark, Color::Light, Color::Light, Color::Dark], 2, 2).unwrap();

        InvertModulesPostProcessor.process(&mut modules).unwrap();

        assert_eq!(modules.modules(), [Color::Light, Color::Dark, Color::Dark, Color::Light]);
    }

    #[test]
    fn plain_text_renderer_uses_configured_colors_and_quiet_zone() {
        let modules = ModuleGrid::new(alloc::vec![Color::Dark, Color::Light, Color::Light, Color::Dark], 2, 2).unwrap();
        let renderer = PlainTextRendererFactory.build(
            &RenderConfig::new().with_option("dark", "X").with_option("light", ".").with_option("quiet_zone", "0"),
        );

        assert_eq!(renderer.render(&modules).unwrap(), RenderOutput::Text("X.\n.X".into()));
    }

    #[test]
    fn plain_text_core_renderer_matches_dyn_renderer() {
        let modules = ModuleGrid::new(alloc::vec![Color::Dark, Color::Light, Color::Light, Color::Dark], 2, 2).unwrap();
        let renderer = super::PlainTextRenderer { dark: 'X', light: '.', quiet_zone: 0, config_valid: true };

        let core_output = CoreRenderer::render(&renderer, &modules).unwrap();
        let dyn_output = qrcode_core::DynRenderer::render(&renderer, &modules).unwrap();

        assert_eq!(core_output, "X.\n.X");
        assert_eq!(dyn_output, RenderOutput::Text(core_output));
    }

    #[test]
    fn plain_text_renderer_reports_invalid_module_source() {
        let renderer = PlainTextRendererFactory.build(&RenderConfig::new());

        assert_eq!(renderer.render(&BadSource { modules: [Color::Dark; 4] }), Err(PluginError::InvalidModuleGrid));
    }

    #[test]
    fn plain_text_factory_rejects_invalid_configuration_before_building() {
        let factory = PlainTextRendererFactory;

        assert!(matches!(
            factory.validate_config(&RenderConfig::new().with_option("quiet_zone", "not-a-number")),
            Err(PluginError::InvalidConfig(_))
        ));
        assert!(matches!(
            factory.validate_config(&RenderConfig::new().with_option("dark", "XX")),
            Err(PluginError::InvalidConfig(_))
        ));
    }

    #[test]
    fn plain_text_registry_reports_invalid_configuration() {
        let mut registry = PluginRegistry::new();
        PlainTextRendererPlugin.register(&mut registry);

        assert!(matches!(
            registry.build_renderer(
                PlainTextRendererPlugin::RENDERER_NAME,
                &RenderConfig::new().with_option("quiet_zone", "-1")
            ),
            Err(PluginError::InvalidConfig(_))
        ));
    }

    #[test]
    fn plain_text_renderer_checks_dimensions_before_arithmetic() {
        let renderer = super::PlainTextRenderer { dark: 'X', light: '.', quiet_zone: u32::MAX, config_valid: true };
        let modules = ModuleGrid::new(alloc::vec![Color::Dark], 1, 1).unwrap();

        assert!(matches!(CoreRenderer::render(&renderer, &modules), Err(PluginError::InvalidConfig(_))));
    }
}

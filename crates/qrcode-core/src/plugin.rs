//! Explicit plugin registry and object-safe extension points.
//!
//! The registry is intentionally local state: callers create a
//! [`PluginRegistry`], register plugins into it, and pass it to facade or
//! application code. This keeps plugin behavior deterministic and avoids hidden
//! global mutation.

use crate::{Color, ModuleSource, ModuleStorage};
use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

/// Error type used by object-safe plugin entry points.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PluginError {
    /// A named renderer was not present in the registry.
    RendererNotFound(String),

    /// A named encoder was not present in the registry.
    EncoderNotFound(String),

    /// The plugin configuration was invalid.
    InvalidConfig(String),

    /// A module grid shape was invalid.
    InvalidModuleGrid,

    /// A renderer failed.
    RenderFailed(String),

    /// An encoder failed.
    EncodeFailed(String),

    /// A postprocessor failed.
    PostProcessFailed(String),
}

impl fmt::Display for PluginError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RendererNotFound(name) => write!(f, "renderer plugin not found: {name}"),
            Self::EncoderNotFound(name) => write!(f, "encoder plugin not found: {name}"),
            Self::InvalidConfig(message) => write!(f, "invalid plugin config: {message}"),
            Self::InvalidModuleGrid => f.write_str("invalid module grid"),
            Self::RenderFailed(message) => write!(f, "renderer plugin failed: {message}"),
            Self::EncodeFailed(message) => write!(f, "encoder plugin failed: {message}"),
            Self::PostProcessFailed(message) => write!(f, "postprocessor plugin failed: {message}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for PluginError {}

/// Runtime renderer configuration passed to renderer factories.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RenderConfig {
    format: Option<String>,
    options: BTreeMap<String, String>,
}

impl RenderConfig {
    /// Creates an empty render configuration.
    #[must_use]
    pub const fn new() -> Self {
        Self { format: None, options: BTreeMap::new() }
    }

    /// Sets the requested output format.
    #[must_use]
    pub fn with_format(mut self, format: impl Into<String>) -> Self {
        self.format = Some(format.into());
        self
    }

    /// Adds or replaces an arbitrary string option.
    #[must_use]
    pub fn with_option(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.options.insert(key.into(), value.into());
        self
    }

    /// Returns the requested output format, if one was configured.
    #[must_use]
    pub fn format(&self) -> Option<&str> {
        self.format.as_deref()
    }

    /// Returns a string option by key.
    #[must_use]
    pub fn option(&self, key: &str) -> Option<&str> {
        self.options.get(key).map(String::as_str)
    }
}

/// Runtime encoder configuration passed to encoder factories.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EncodeConfig {
    options: BTreeMap<String, String>,
}

impl EncodeConfig {
    /// Creates an empty encode configuration.
    #[must_use]
    pub const fn new() -> Self {
        Self { options: BTreeMap::new() }
    }

    /// Adds or replaces an arbitrary string option.
    #[must_use]
    pub fn with_option(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.options.insert(key.into(), value.into());
        self
    }

    /// Returns a string option by key.
    #[must_use]
    pub fn option(&self, key: &str) -> Option<&str> {
        self.options.get(key).map(String::as_str)
    }
}

/// Type-erased render output returned by dynamic renderers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RenderOutput {
    /// Text output such as SVG, HTML, ANSI, or plain strings.
    Text(String),

    /// Binary output such as PNG, PDF, or other encoded bytes.
    Bytes(Vec<u8>),

    /// A module-grid output for plugins that transform but do not serialize.
    Modules(ModuleGrid),
}

/// Type-erased encode output returned by dynamic encoders.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EncodedOutput {
    /// Encoded QR modules.
    Modules(ModuleGrid),

    /// Opaque encoded bytes.
    Bytes(Vec<u8>),
}

/// Owned mutable module grid used by plugin postprocessors.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModuleGrid {
    modules: Vec<Color>,
    width: usize,
    height: usize,
}

impl ModuleGrid {
    /// Creates a module grid from row-major modules.
    ///
    /// # Errors
    ///
    /// Returns [`PluginError::InvalidModuleGrid`] when the dimensions are zero
    /// or `modules.len() != width * height`.
    pub fn new(modules: Vec<Color>, width: usize, height: usize) -> Result<Self, PluginError> {
        if width == 0 || height == 0 || modules.len() != width * height {
            return Err(PluginError::InvalidModuleGrid);
        }
        Ok(Self { modules, width, height })
    }

    /// Returns the grid modules as a mutable row-major slice.
    #[must_use]
    pub fn modules_mut(&mut self) -> &mut [Color] {
        &mut self.modules
    }
}

impl ModuleStorage for ModuleGrid {
    fn get(&self, x: usize, y: usize) -> Color {
        self.modules[y * self.width + x]
    }

    fn set(&mut self, x: usize, y: usize, color: Color) {
        self.modules[y * self.width + x] = color;
    }

    fn width(&self) -> usize {
        self.width
    }

    fn height(&self) -> usize {
        self.height
    }

    fn modules(&self) -> &[Color] {
        &self.modules
    }
}

/// Object-safe renderer used by [`RendererFactory`].
pub trait DynRenderer {
    /// Renders a module source.
    ///
    /// # Errors
    ///
    /// Returns [`PluginError`] when the renderer cannot produce output.
    fn render(&self, code: &dyn ModuleSource) -> Result<RenderOutput, PluginError>;
}

/// Factory for object-safe renderers.
pub trait RendererFactory {
    /// Builds a renderer from `config`.
    fn build(&self, config: &RenderConfig) -> Box<dyn DynRenderer>;
}

/// Object-safe encoder used by [`EncoderFactory`].
pub trait DynEncoder {
    /// Encodes raw input.
    ///
    /// # Errors
    ///
    /// Returns [`PluginError`] when the encoder cannot produce output.
    fn encode(&self, input: &[u8]) -> Result<EncodedOutput, PluginError>;
}

/// Factory for object-safe encoders.
pub trait EncoderFactory {
    /// Builds an encoder from `config`.
    fn build(&self, config: &EncodeConfig) -> Box<dyn DynEncoder>;
}

/// Object-safe postprocessor for in-place module-grid transforms.
pub trait PostProcessor {
    /// Processes `modules` in place.
    ///
    /// # Errors
    ///
    /// Returns [`PluginError`] when processing fails.
    fn process(&self, modules: &mut dyn ModuleStorage) -> Result<(), PluginError>;
}

/// A plugin that registers one or more extension points.
pub trait QrPlugin {
    /// Stable plugin name.
    fn name(&self) -> &str;

    /// Plugin version string.
    fn version(&self) -> &str;

    /// Registers this plugin's extension points into `registry`.
    fn register(&self, registry: &mut PluginRegistry);
}

/// Explicit plugin registry.
#[derive(Default)]
pub struct PluginRegistry {
    renderers: BTreeMap<String, Box<dyn RendererFactory>>,
    encoders: BTreeMap<String, Box<dyn EncoderFactory>>,
    postprocessors: Vec<Box<dyn PostProcessor>>,
}

impl PluginRegistry {
    /// Creates an empty registry.
    #[must_use]
    pub const fn new() -> Self {
        Self { renderers: BTreeMap::new(), encoders: BTreeMap::new(), postprocessors: Vec::new() }
    }

    /// Registers all extension points provided by `plugin`.
    pub fn register_plugin<P: QrPlugin + ?Sized>(&mut self, plugin: &P) {
        plugin.register(self);
    }

    /// Registers or replaces a renderer factory by name.
    pub fn register_renderer(
        &mut self,
        name: impl Into<String>,
        factory: Box<dyn RendererFactory>,
    ) -> Option<Box<dyn RendererFactory>> {
        self.renderers.insert(name.into(), factory)
    }

    /// Registers or replaces an encoder factory by name.
    pub fn register_encoder(
        &mut self,
        name: impl Into<String>,
        factory: Box<dyn EncoderFactory>,
    ) -> Option<Box<dyn EncoderFactory>> {
        self.encoders.insert(name.into(), factory)
    }

    /// Appends a postprocessor to the registry.
    pub fn register_postprocessor(&mut self, postprocessor: Box<dyn PostProcessor>) {
        self.postprocessors.push(postprocessor);
    }

    /// Returns a renderer factory by name.
    #[must_use]
    pub fn renderer(&self, name: &str) -> Option<&dyn RendererFactory> {
        self.renderers.get(name).map(Box::as_ref)
    }

    /// Returns an encoder factory by name.
    #[must_use]
    pub fn encoder(&self, name: &str) -> Option<&dyn EncoderFactory> {
        self.encoders.get(name).map(Box::as_ref)
    }

    /// Returns all postprocessors in registration order.
    #[must_use]
    pub fn postprocessors(&self) -> &[Box<dyn PostProcessor>] {
        &self.postprocessors
    }

    /// Iterates renderer names in deterministic order.
    pub fn renderer_names(&self) -> impl Iterator<Item = &str> {
        self.renderers.keys().map(String::as_str)
    }

    /// Iterates encoder names in deterministic order.
    pub fn encoder_names(&self) -> impl Iterator<Item = &str> {
        self.encoders.keys().map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DynEncoder, DynRenderer, EncodeConfig, EncodedOutput, EncoderFactory, ModuleGrid, PluginRegistry,
        PostProcessor, QrPlugin, RenderConfig, RenderOutput, RendererFactory,
    };
    use crate::{Color, ModuleSource, ModuleStorage};
    use alloc::boxed::Box;
    use alloc::string::ToString;

    struct TextRenderer {
        dark: char,
    }

    impl DynRenderer for TextRenderer {
        fn render(&self, code: &dyn ModuleSource) -> Result<RenderOutput, super::PluginError> {
            let mut out = String::new();
            for y in 0..code.height() {
                for x in 0..code.width() {
                    out.push(if code.get(x, y) == Color::Dark { self.dark } else { '.' });
                }
            }
            Ok(RenderOutput::Text(out))
        }
    }

    struct TextRendererFactory;

    impl RendererFactory for TextRendererFactory {
        fn build(&self, config: &RenderConfig) -> Box<dyn DynRenderer> {
            let dark = config.option("dark").and_then(|s| s.chars().next()).unwrap_or('#');
            Box::new(TextRenderer { dark })
        }
    }

    struct LengthEncoder;

    impl DynEncoder for LengthEncoder {
        fn encode(&self, input: &[u8]) -> Result<EncodedOutput, super::PluginError> {
            Ok(EncodedOutput::Bytes(input.len().to_string().into_bytes()))
        }
    }

    struct LengthEncoderFactory;

    impl EncoderFactory for LengthEncoderFactory {
        fn build(&self, _config: &EncodeConfig) -> Box<dyn DynEncoder> {
            Box::new(LengthEncoder)
        }
    }

    struct FlipFirst;

    impl PostProcessor for FlipFirst {
        fn process(&self, modules: &mut dyn ModuleStorage) -> Result<(), super::PluginError> {
            modules.set(0, 0, Color::Dark);
            Ok(())
        }
    }

    struct DemoPlugin;

    impl QrPlugin for DemoPlugin {
        fn name(&self) -> &str {
            "demo"
        }

        fn version(&self) -> &str {
            "0.1.0"
        }

        fn register(&self, registry: &mut PluginRegistry) {
            registry.register_renderer("text", Box::new(TextRendererFactory));
            registry.register_encoder("length", Box::new(LengthEncoderFactory));
            registry.register_postprocessor(Box::new(FlipFirst));
        }
    }

    #[test]
    fn registry_registers_and_uses_plugin_extension_points() {
        let mut registry = PluginRegistry::new();
        registry.register_plugin(&DemoPlugin);

        let grid = ModuleGrid::new(alloc::vec![Color::Dark, Color::Light, Color::Light, Color::Dark], 2, 2).unwrap();
        let config = RenderConfig::new().with_option("dark", "X");
        let renderer = registry.renderer("text").unwrap().build(&config);
        assert_eq!(renderer.render(&grid).unwrap(), RenderOutput::Text("X..X".into()));

        let encoder = registry.encoder("length").unwrap().build(&EncodeConfig::new());
        assert_eq!(encoder.encode(b"abcd").unwrap(), EncodedOutput::Bytes(b"4".to_vec()));
    }

    #[test]
    fn registry_keeps_names_deterministic() {
        let mut registry = PluginRegistry::new();
        registry.register_renderer("zeta", Box::new(TextRendererFactory));
        registry.register_renderer("alpha", Box::new(TextRendererFactory));

        let names = registry.renderer_names().collect::<Vec<_>>();
        assert_eq!(names, ["alpha", "zeta"]);
    }

    #[test]
    fn postprocessors_mutate_module_storage_in_order() {
        let mut registry = PluginRegistry::new();
        registry.register_postprocessor(Box::new(FlipFirst));
        let mut grid = ModuleGrid::new(alloc::vec![Color::Light; 4], 2, 2).unwrap();

        for postprocessor in registry.postprocessors() {
            postprocessor.process(&mut grid).unwrap();
        }

        assert_eq!(ModuleSource::get(&grid, 0, 0), Color::Dark);
    }
}

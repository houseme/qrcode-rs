use qrcode_rs::render::Renderer as RenderBuilder;
use qrcode_rs::{
    Color, DynEncoder, DynRenderer, EcLevel, EncodeConfig, EncodedOutput, EncoderFactory, ModuleSource, ModuleStorage,
    PluginError, PluginRegistry, PostProcessor, QrCode, QrPlugin, QrSymbol, RenderConfig, RenderOutput,
    RendererFactory, Version,
};

struct ThirdPartySymbol {
    modules: [Color; 4],
    version: Version,
    ec_level: EcLevel,
}

impl ModuleSource for ThirdPartySymbol {
    fn get(&self, x: usize, y: usize) -> Color {
        self.modules[y * self.width() + x]
    }

    fn width(&self) -> usize {
        2
    }

    fn height(&self) -> usize {
        2
    }

    fn modules(&self) -> &[Color] {
        &self.modules
    }
}

impl QrSymbol for ThirdPartySymbol {
    fn version(&self) -> Version {
        self.version
    }

    fn error_correction_level(&self) -> EcLevel {
        self.ec_level
    }
}

struct ThirdPartyRenderer;

impl DynRenderer for ThirdPartyRenderer {
    fn render(&self, code: &dyn ModuleSource) -> Result<RenderOutput, PluginError> {
        let mut output = String::with_capacity(code.width() * code.height());
        for y in 0..code.height() {
            for x in 0..code.width() {
                output.push(if code.get(x, y) == Color::Dark { '1' } else { '0' });
            }
        }
        Ok(RenderOutput::Text(output))
    }
}

struct ThirdPartyRendererFactory;

impl RendererFactory for ThirdPartyRendererFactory {
    fn build(&self, _config: &RenderConfig) -> Box<dyn DynRenderer> {
        Box::new(ThirdPartyRenderer)
    }
}

struct ThirdPartyEncoder;

impl DynEncoder for ThirdPartyEncoder {
    fn encode(&self, input: &[u8]) -> Result<EncodedOutput, PluginError> {
        Ok(EncodedOutput::Bytes(input.len().to_string().into_bytes()))
    }
}

struct ThirdPartyEncoderFactory;

impl EncoderFactory for ThirdPartyEncoderFactory {
    fn build(&self, _config: &EncodeConfig) -> Box<dyn DynEncoder> {
        Box::new(ThirdPartyEncoder)
    }
}

struct ForceFirstModuleLight;

impl PostProcessor for ForceFirstModuleLight {
    fn process(&self, modules: &mut dyn ModuleStorage) -> Result<(), PluginError> {
        modules.set(0, 0, Color::Light);
        Ok(())
    }
}

struct ThirdPartyPlugin;

impl QrPlugin for ThirdPartyPlugin {
    fn name(&self) -> &str {
        "third-party-test"
    }

    fn version(&self) -> &str {
        "0.1.0"
    }

    fn register(&self, registry: &mut PluginRegistry) {
        registry.register_renderer("third-party/text", Box::new(ThirdPartyRendererFactory));
        registry.register_encoder("third-party/length", Box::new(ThirdPartyEncoderFactory));
        registry.register_postprocessor(Box::new(ForceFirstModuleLight));
    }
}

#[test]
fn third_party_symbol_renders_through_public_qr_symbol_trait() {
    let symbol = ThirdPartySymbol {
        modules: [Color::Dark, Color::Light, Color::Light, Color::Dark],
        version: Version::Normal(1),
        ec_level: EcLevel::M,
    };

    let rendered =
        RenderBuilder::<char>::from_symbol(&symbol).quiet_zone(false).dark_color('#').light_color('.').build();

    assert_eq!(rendered, "#.\n.#");
}

#[test]
fn third_party_plugin_registers_and_runs_through_facade() {
    let mut registry = PluginRegistry::new();
    registry.register_plugin(&ThirdPartyPlugin);

    let encoded = QrCode::encode_with(&registry, "third-party/length", b"abcd", &EncodeConfig::new()).unwrap();
    assert_eq!(encoded, EncodedOutput::Bytes(b"4".to_vec()));

    let code = QrCode::new(b"plugin").unwrap();
    let rendered = code.render_with(&registry, "third-party/text", &RenderConfig::new()).unwrap();
    let RenderOutput::Text(text) = rendered else {
        panic!("expected text output");
    };

    assert_eq!(text.len(), code.width() * code.width());
    assert!(text.starts_with('0'));
}

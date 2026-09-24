use starforge::export_ai_plugin;
use starforge::plugins::ai::{AICapability, AIPlugin, AIPluginRegistrar, AIRequest, AIResponse};

struct MyAIPlugin;

impl AIPlugin for MyAIPlugin {
    fn name(&self) -> String {
        "starforge-ai-audit".to_string()
    }

    fn version(&self) -> String {
        "1.0.0".to_string()
    }

    fn capabilities(&self) -> Vec<AICapability> {
        vec![AICapability::NetworkAccess]
    }

    fn execute(&self, request: AIRequest) -> AIResponse {
        AIResponse {
            text: format!("Audited contract based on prompt: {}", request.prompt),
            error: None,
        }
    }
}

fn register(registrar: &mut dyn AIPluginRegistrar) {
    registrar.register_ai_plugin(Box::new(MyAIPlugin));
}

export_ai_plugin!(register);

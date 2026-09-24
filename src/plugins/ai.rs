use std::any::Any;
use std::collections::HashMap;

/// Capabilities an AI plugin can request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AICapability {
    NetworkAccess,
    FileSystemAccess,
    ExecuteCode,
    Custom(String),
}

#[derive(Debug, Clone)]
pub struct AIRequest {
    pub prompt: String,
    pub context: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct AIResponse {
    pub text: String,
    pub error: Option<String>,
}

pub trait AIPlugin: Any + Send + Sync {
    fn name(&self) -> String;
    fn version(&self) -> String;
    fn capabilities(&self) -> Vec<AICapability>;
    fn execute(&self, request: AIRequest) -> AIResponse;
}

pub struct AIPluginDeclaration {
    pub rustc_version: &'static str,
    pub core_version: &'static str,
    pub register: unsafe fn(&mut dyn AIPluginRegistrar),
}

pub trait AIPluginRegistrar {
    fn register_ai_plugin(&mut self, plugin: Box<dyn AIPlugin>);
}

#[macro_export]
macro_rules! export_ai_plugin {
    ($register:expr) => {
        #[doc(hidden)]
        #[no_mangle]
        pub static AI_PLUGIN_DECLARATION: $crate::plugins::ai::AIPluginDeclaration =
            $crate::plugins::ai::AIPluginDeclaration {
                rustc_version: $crate::plugins::interface::RUSTC_VERSION,
                core_version: $crate::plugins::interface::CORE_VERSION,
                register: $register,
            };
    };
}

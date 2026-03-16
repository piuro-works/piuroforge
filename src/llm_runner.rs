use anyhow::Result;

pub trait PromptRunner: Send + Sync {
    fn run_prompt_named(&self, label: &str, prompt: &str) -> Result<String>;
}

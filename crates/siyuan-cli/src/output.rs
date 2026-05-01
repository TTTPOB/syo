use clap::ValueEnum;

#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
#[value(rename_all = "kebab-case")]
pub enum OutputFormat {
    AgentMd,
    Json,
    JsonPretty,
}

impl Default for OutputFormat {
    fn default() -> Self {
        Self::AgentMd
    }
}

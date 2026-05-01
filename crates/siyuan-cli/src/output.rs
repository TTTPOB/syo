use clap::ValueEnum;

#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq, Default)]
#[value(rename_all = "kebab-case")]
pub enum OutputFormat {
    #[default]
    AgentMd,
    Json,
    JsonPretty,
}

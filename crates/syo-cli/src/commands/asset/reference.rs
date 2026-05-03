use anyhow::Result;
use clap::Args;

#[derive(Args, Debug)]
pub struct ReferenceArgs {
    /// Kernel-relative asset path (the value returned by `syo asset upload`).
    #[arg(long)]
    pub path: String,

    /// Alt text. For images, defaults to the file basename.
    #[arg(long, default_value = "")]
    pub alt: String,
}

pub fn run(args: ReferenceArgs) -> Result<()> {
    let output = syo_core::asset::reference(syo_core::asset::ReferenceInput {
        path: args.path,
        alt: args.alt,
    });
    println!("{}", output.markdown);
    Ok(())
}

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
    let alt = if args.alt.is_empty() {
        args.path.rsplit('/').next().unwrap_or("").to_string()
    } else {
        args.alt
    };
    // Image-style markdown reference.
    println!("![{alt}]({})", args.path);
    Ok(())
}

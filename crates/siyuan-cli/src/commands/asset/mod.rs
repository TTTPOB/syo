use anyhow::Result;
use clap::Subcommand;

use siyuan_client::SiyuanClient;

pub mod reference;
pub mod upload;

use self::reference::ReferenceArgs;
use self::upload::UploadArgs;

#[derive(Subcommand, Debug)]
pub enum AssetCmd {
    /// Upload a local file as a SiYuan asset.
    ///
    /// Sibling commands: `syo asset reference` formats a markdown
    /// embed for an already-uploaded path; this command performs the
    /// upload and prints the kernel-relative path. Pipe one into the
    /// other if you want both steps.
    ///
    /// Inputs:
    ///   --file (required): path to a local file readable by this
    ///     process.
    ///
    /// Prints the kernel-relative asset path to stdout (e.g.
    /// `assets/image-20260501-abc.png`). The kernel copies the bytes
    /// into its `assets/` directory and assigns a stable name; the
    /// returned path is what you embed in markdown.
    ///
    /// Example:
    ///   in:  --file ./diagram.png
    ///   out: assets/diagram-20260501090000-abc.png
    #[command(verbatim_doc_comment)]
    Upload(UploadArgs),
    /// Print the markdown snippet for embedding an already-uploaded asset.
    ///
    /// Sibling commands: `syo asset upload` performs the upload step;
    /// this is purely a formatter — it does NOT call the kernel. There
    /// is no anchor concept here: the snippet is unconditionally an
    /// image-style markdown reference (`![alt](path)`); for non-image
    /// assets edit the printed line.
    ///
    /// Inputs:
    ///   --path (required): kernel-relative asset path (the value
    ///     printed by `syo asset upload`, e.g. `assets/foo.png`).
    ///   --alt (optional, default empty): alt text. When empty, the
    ///     filename component of `--path` is used as alt text.
    ///
    /// Example:
    ///   in:  --path assets/diagram-20260501090000-abc.png --alt Diagram
    ///   out: ![Diagram](assets/diagram-20260501090000-abc.png)
    #[command(verbatim_doc_comment)]
    Reference(ReferenceArgs),
}

pub async fn run(client: &SiyuanClient, cmd: AssetCmd) -> Result<()> {
    match cmd {
        AssetCmd::Upload(a) => upload::run(client, a).await,
        AssetCmd::Reference(a) => reference::run(a),
    }
}

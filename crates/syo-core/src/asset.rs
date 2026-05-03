use std::path::Path;

use anyhow::{Context, Result};

use siyuan_client::SiyuanClient;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct UploadInput {
    pub file_path: String,
}

#[derive(Debug)]
pub struct UploadOutput {
    pub asset_path: String,
}

#[derive(Debug)]
pub struct ReferenceInput {
    pub path: String,
    pub alt: String,
}

#[derive(Debug)]
pub struct ReferenceOutput {
    pub markdown: String,
}

// ---------------------------------------------------------------------------
// Operations
// ---------------------------------------------------------------------------

/// Upload a local file as a SiYuan asset.
///
/// Returns the kernel-relative asset path (e.g. `assets/img-abc.png`),
/// suitable for embedding in markdown as `![alt](assets/img-abc.png)`.
pub async fn upload(client: &SiyuanClient, input: UploadInput) -> Result<UploadOutput> {
    let file_path = Path::new(&input.file_path);
    let asset_path = client
        .upload_asset(file_path)
        .await
        .context("asset upload")?;
    Ok(UploadOutput { asset_path })
}

/// Build a markdown image reference from a path and alt text.
///
/// If `alt` is empty, the filename portion of `path` is used as alt text.
/// This is a pure formatting function — it does not contact the SiYuan kernel.
pub fn reference(input: ReferenceInput) -> ReferenceOutput {
    let alt = if input.alt.is_empty() {
        input.path.rsplit('/').next().unwrap_or("").to_string()
    } else {
        input.alt
    };
    ReferenceOutput {
        markdown: format!("![{alt}]({})", input.path),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_uses_alt_when_provided() {
        let out = reference(ReferenceInput {
            path: "assets/img.png".into(),
            alt: "My Image".into(),
        });
        assert_eq!(out.markdown, "![My Image](assets/img.png)");
    }

    #[test]
    fn reference_falls_back_to_filename() {
        let out = reference(ReferenceInput {
            path: "assets/img.png".into(),
            alt: "".into(),
        });
        assert_eq!(out.markdown, "![img.png](assets/img.png)");
    }

    #[test]
    fn reference_falls_back_to_last_segment() {
        let out = reference(ReferenceInput {
            path: "foo/bar/img.png".into(),
            alt: "".into(),
        });
        assert_eq!(out.markdown, "![img.png](foo/bar/img.png)");
    }

    #[test]
    fn reference_empty_path_empty_alt() {
        let out = reference(ReferenceInput {
            path: "".into(),
            alt: "".into(),
        });
        assert_eq!(out.markdown, "![]()");
    }

    #[test]
    fn structs_derive_debug() {
        fn _assert_debug<T: std::fmt::Debug>(_t: &T) {}

        let ui = UploadInput {
            file_path: "/tmp/img.png".into(),
        };
        _assert_debug(&ui);

        let uo = UploadOutput {
            asset_path: "assets/img-abc123.png".into(),
        };
        _assert_debug(&uo);

        let ri = ReferenceInput {
            path: "assets/img.png".into(),
            alt: "img".into(),
        };
        _assert_debug(&ri);

        let ro = ReferenceOutput {
            markdown: "![img](assets/img.png)".into(),
        };
        _assert_debug(&ro);
    }
}

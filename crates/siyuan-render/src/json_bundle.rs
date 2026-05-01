use serde::Serialize;

use siyuan_model::bundle::DocBundle;

pub fn render<T: Serialize>(value: &T, pretty: bool) -> serde_json::Result<String> {
    if pretty {
        serde_json::to_string_pretty(value)
    } else {
        serde_json::to_string(value)
    }
}

pub fn render_bundle(bundle: &DocBundle, pretty: bool) -> serde_json::Result<String> {
    render(bundle, pretty)
}

use toml_edit::{DocumentMut, value};

pub(super) fn install(manifest: &mut DocumentMut, wasm: bool) {
    let feature = if wasm { "aurelin-framework?/tokio-console" } else { "aurelin-framework/tokio-console" };
    manifest["features"]["tokio-console"] = value([feature].into_iter().collect::<toml_edit::Array>());
}

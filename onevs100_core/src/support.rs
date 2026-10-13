pub const EXTENSIONS: &[&str] = &[
    "xzp", "xus", "xur", "mct", "png", "jpg", "jpeg", "psd", "lua", "ext", "resx", "updb", "xml", "txt", "wmv", "xsb", "xwb", "xex", "bin", "avd", "fxlobj", "msliveavatarasset", "model", "avatarasset", "toc", "colours", "animation", "metadata", "live", "pirs", "con", "xdbf", "xstr", "xlast", "lex",
];

fn extension(name: &str) -> Option<&str> {
    let (stem, ext) = name.rsplit_once('.')?;
    (!stem.is_empty() && !ext.is_empty()).then_some(ext)
}

pub fn is_supported(name: &str) -> bool {
    extension(name).is_none_or(|ext| EXTENSIONS.contains(&ext.to_lowercase().as_str()))
}

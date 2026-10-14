use crate::support;

#[test]
fn names_without_an_extension_are_supported() {
    for name in ["default", "00001000-e74d-7233-cd20-11ab58570b10", ".hidden", "trailing.", "SIGNIN", ""] {
        assert!(support::is_supported(name), "{name:?}");
    }
}

#[test]
fn only_allowlisted_extensions_are_supported_case_insensitively() {
    for name in ["a.xzp", "B.XWB", "x.mb.bin", "pic.PNG", "p.jpeg", "q.resx", "a.avatarasset", "x.toc", "block7.colours", "Dash.Kiosk.lex", "x.xdbf", "s.xstr", "a.xlast"] {
        assert!(support::is_supported(name), "{name}");
    }
    for name in ["movie.mp4", "notes.docx", "a.zip", "a.dll", "a.ttf", "a.dds", "block5.block5", "a.exe", "x.bin.bak"] {
        assert!(!support::is_supported(name), "{name}");
    }
}

#[test]
fn every_extension_the_inspector_dispatches_on_is_allowlisted() {
    for ext in ["lua", "resx", "updb", "xml", "ext", "txt", "mct", "avd", "model", "animation", "colours", "metadata", "avatarasset", "toc"] {
        assert!(support::EXTENSIONS.contains(&ext), "{ext}");
    }
    assert!(support::EXTENSIONS.iter().all(|e| *e == e.to_lowercase() && !e.contains('.')));
}

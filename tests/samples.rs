mod common;

use common::{read_part_of, text, type_keys};
use tuidoc::app::App;
use tuidoc::document::Document;
use tuidoc::docx::saving::package_bytes;

/// The sample documents Word wrote, if any have been generated.
fn samples() -> Vec<std::path::PathBuf> {
    let folder = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("samples/docx");
    let Ok(entries) = std::fs::read_dir(folder) else {
        return Vec::new();
    };
    entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "docx")
        })
        .collect()
}

#[test]
fn saving_a_sample_unchanged_keeps_its_document_part() {
    for path in samples() {
        let original = std::fs::read(&path).unwrap();
        let document = Document::open(&path).unwrap();
        let saved = package_bytes(&document).unwrap();
        let part = &document.package.main;
        assert_eq!(
            read_part_of(&saved, part),
            read_part_of(&original, part),
            "{path:?}"
        );
    }
}

#[test]
fn an_edited_sample_reads_back_as_it_was_left() {
    for path in samples() {
        let mut app = App::new(Document::open(&path).unwrap());
        app.fit(76, 20);
        type_keys(&mut app, "Aedited⎋gbiwjg2Gonew paragraph⎋glggdd");
        let saved = package_bytes(app.document()).unwrap();
        let reopened = Document::from_bytes(saved, None).unwrap();
        assert_eq!(reopened.text(), text(&app), "{path:?}");
    }
}

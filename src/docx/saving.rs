//! Writes a document back to its file without disturbing what wasn't edited.

use std::collections::BTreeMap;
use std::io::{Cursor, Write};
use std::path::Path;
use std::sync::Arc;

use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use super::{DocxError, writer};
use crate::document::Document;

/// The bytes of the package as it should now be saved. Parts tuidoc didn't change are copied
/// as they are compressed in the original, so they come out bit for bit the same.
pub fn package_bytes(document: &Document) -> Result<Vec<u8>, DocxError> {
    let mut changed: BTreeMap<&str, &str> = document
        .parts
        .iter()
        .map(|(name, content)| (name.as_str(), content.as_str()))
        .collect();
    let body = writer::story_xml(document.body());
    if body != *document.body().source {
        changed.insert(&document.body().part, &body);
    }

    let mut archive = ZipArchive::new(Cursor::new(document.package.bytes.as_slice()))?;
    let mut output = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    for index in 0..archive.len() {
        let file = archive.by_index_raw(index)?;
        let name = file.name()?.to_string();
        match changed.remove(name.as_str()) {
            Some(content) => {
                drop(file);
                output.start_file(name, options)?;
                output.write_all(content.as_bytes())?;
            }
            None => output.raw_copy_file(file)?,
        }
    }
    for (name, content) in changed {
        output.start_file(name, options)?;
        output.write_all(content.as_bytes())?;
    }
    Ok(output.finish()?.into_inner())
}

/// Saves to the document's own file, or to a new one, which then becomes its file.
pub fn save(document: &mut Document, target: Option<&Path>) -> Result<(), DocxError> {
    let own = document.path().map(Path::to_path_buf);
    let target = match target {
        Some(path) => std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf()),
        None => own.clone().ok_or(DocxError::NoFileName)?,
    };
    // Another program's changes to the file must never be silently replaced.
    if own.as_deref() == Some(target.as_path())
        && let Some(expected) = &document.on_disk
        && target.exists()
        && std::fs::read(&target)? != **expected
    {
        return Err(DocxError::ChangedOnDisk);
    }
    let bytes = package_bytes(document)?;
    let folder = target
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    // Written beside the target and moved into place, so a failure half way leaves the old
    // file whole.
    let mut temporary = tempfile::NamedTempFile::new_in(folder)?;
    temporary.write_all(&bytes)?;
    temporary.flush()?;
    if let Ok(existing) = std::fs::metadata(&target) {
        let _ = std::fs::set_permissions(temporary.path(), existing.permissions());
    }
    temporary.persist(&target).map_err(|error| error.error)?;
    document.on_disk = Some(Arc::new(bytes));
    document.path = Some(target);
    document.mark_saved();
    Ok(())
}

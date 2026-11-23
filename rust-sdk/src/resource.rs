use crate::Error;
use std::{
    fs::{self, File, OpenOptions},
    io,
    path::{Component, Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use zip::{write::SimpleFileOptions, ZipWriter};

pub(crate) fn validate(path: &Path, h5_id: &str) -> Result<(), Error> {
    if path.extension().and_then(|value| value.to_str()) == Some("amr") {
        return Err(Error::new(
            "config",
            "upload.resource is an AMR package; provide a directory or <h5Id>.zip",
        ));
    }
    let metadata = fs::symlink_metadata(path)
        .map_err(|e| Error::new("config", format!("cannot read upload.resource: {e}")))?;
    if metadata.is_dir() {
        validate_directory(path)?;
        return Ok(());
    }
    if !metadata.is_file() {
        return Err(Error::new(
            "config",
            "upload.resource must be a directory or ZIP file",
        ));
    }
    validate_zip(path, h5_id)
}

pub(crate) fn package_directory(path: &Path, h5_id: &str) -> Result<(PathBuf, PathBuf), Error> {
    validate_directory(path)?;
    let parent = unique_directory()?;
    let output = parent.join(format!("{h5_id}.zip"));
    let file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&output)
        .map_err(|e| Error::new("file", format!("cannot create temporary upload ZIP: {e}")))?;
    let mut writer = ZipWriter::new(file);
    let options = SimpleFileOptions::default();
    let result = write_directory(&mut writer, path, path, h5_id, options);
    let result = match result {
        Ok(()) => writer
            .finish()
            .map(|_| ())
            .map_err(|e| Error::new("file", format!("cannot finish temporary upload ZIP: {e}"))),
        Err(error) => Err(error),
    };
    if let Err(error) = result {
        let _ = fs::remove_dir_all(&parent);
        return Err(error);
    }
    Ok((output, parent))
}

pub(crate) fn cleanup(parent: &Path) {
    let _ = fs::remove_dir_all(parent);
}

fn validate_directory(path: &Path) -> Result<(), Error> {
    let mut count = 0;
    let mut source_project = false;
    walk_directory(path, &mut |_, metadata| {
        if metadata.is_file() {
            count += 1;
        }
        Ok(())
    })?;
    if count == 0 {
        return Err(Error::new(
            "config",
            "upload.resource directory contains no files",
        ));
    }
    for marker in ["mini.project.json", "mini.project.jsonc"] {
        if path.join(marker).is_file() {
            source_project = true;
            break;
        }
    }
    if !source_project {
        walk_directory(path, &mut |entry, metadata| {
            if metadata.is_file()
                && matches!(
                    entry.extension().and_then(|value| value.to_str()),
                    Some("axml" | "acss" | "less")
                )
            {
                source_project = true;
            }
            Ok(())
        })?;
    }
    if source_project {
        return Err(Error::new(
            "config",
            "upload.resource is a source project; use the complete mpaas-amr executable to compile it before upload",
        ));
    }
    Ok(())
}

fn walk_directory<F>(path: &Path, callback: &mut F) -> Result<(), Error>
where
    F: FnMut(&Path, fs::Metadata) -> Result<(), Error>,
{
    let metadata = fs::symlink_metadata(path)
        .map_err(|e| Error::new("config", format!("cannot read upload.resource entry: {e}")))?;
    if metadata.file_type().is_symlink() {
        return Err(Error::new(
            "config",
            "upload.resource cannot contain symbolic links",
        ));
    }
    if metadata.is_file() {
        callback(path, metadata)?;
        return Ok(());
    }
    if !metadata.is_dir() {
        return Err(Error::new(
            "config",
            "upload.resource contains a non-file entry",
        ));
    }
    for entry in fs::read_dir(path).map_err(|e| {
        Error::new(
            "config",
            format!("cannot read upload.resource directory: {e}"),
        )
    })? {
        let entry = entry
            .map_err(|e| Error::new("config", format!("cannot read upload.resource entry: {e}")))?;
        walk_directory(&entry.path(), callback)?;
    }
    Ok(())
}

fn write_directory(
    writer: &mut ZipWriter<File>,
    root: &Path,
    path: &Path,
    h5_id: &str,
    options: SimpleFileOptions,
) -> Result<(), Error> {
    for entry in fs::read_dir(path).map_err(|e| {
        Error::new(
            "file",
            format!("cannot read upload.resource directory: {e}"),
        )
    })? {
        let entry = entry
            .map_err(|e| Error::new("file", format!("cannot read upload.resource entry: {e}")))?;
        let entry_path = entry.path();
        let metadata = fs::symlink_metadata(&entry_path)
            .map_err(|e| Error::new("file", format!("cannot read upload.resource entry: {e}")))?;
        if metadata.file_type().is_symlink() {
            return Err(Error::new(
                "config",
                "upload.resource cannot contain symbolic links",
            ));
        }
        if metadata.is_dir() {
            write_directory(writer, root, &entry_path, h5_id, options)?;
            continue;
        }
        if !metadata.is_file() {
            return Err(Error::new(
                "config",
                "upload.resource contains a non-file entry",
            ));
        }
        let relative = entry_path
            .strip_prefix(root)
            .map_err(|_| Error::new("file", "cannot resolve upload.resource entry"))?;
        let name = archive_name(h5_id, relative)?;
        writer
            .start_file(name, options)
            .map_err(|e| Error::new("file", format!("cannot create ZIP entry: {e}")))?;
        let mut input = File::open(&entry_path)
            .map_err(|e| Error::new("file", format!("cannot open upload.resource entry: {e}")))?;
        io::copy(&mut input, writer)
            .map_err(|e| Error::new("file", format!("cannot read upload.resource entry: {e}")))?;
    }
    Ok(())
}

fn archive_name(h5_id: &str, relative: &Path) -> Result<String, Error> {
    let mut parts = Vec::new();
    for component in relative.components() {
        let Component::Normal(value) = component else {
            return Err(Error::new(
                "config",
                "upload.resource contains an unsafe path",
            ));
        };
        let value = value
            .to_str()
            .ok_or_else(|| Error::new("config", "upload.resource contains a non-UTF-8 filename"))?;
        if value.is_empty() || value.contains('/') || value.contains('\\') || value.contains(':') {
            return Err(Error::new(
                "config",
                "upload.resource contains an unsafe path",
            ));
        }
        parts.push(value);
    }
    if parts.is_empty() {
        return Err(Error::new(
            "config",
            "upload.resource contains an empty path",
        ));
    }
    Ok(format!("{h5_id}/{}", parts.join("/")))
}

fn unique_directory() -> Result<PathBuf, Error> {
    let base = std::env::temp_dir();
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| Error::new("file", "system clock is before UNIX epoch"))?
        .as_nanos();
    for attempt in 0..100u32 {
        let path = base.join(format!(
            "mpaas-upload-{}-{stamp}-{attempt}",
            std::process::id()
        ));
        match fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(Error::new(
                    "file",
                    format!("cannot create temporary upload directory: {error}"),
                ))
            }
        }
    }
    Err(Error::new(
        "file",
        "cannot allocate a temporary upload directory",
    ))
}

fn validate_zip(path: &Path, h5_id: &str) -> Result<(), Error> {
    if path.file_name().and_then(|s| s.to_str()) != Some(format!("{h5_id}.zip").as_str()) {
        return Err(Error::new(
            "config",
            "resource must be a directory or named <h5Id>.zip; AMR cannot be uploaded directly",
        ));
    }
    let file =
        File::open(path).map_err(|e| Error::new("config", format!("cannot open ZIP: {e}")))?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|_| Error::new("config", "upload.resource is not a valid ZIP archive"))?;
    let prefix = format!("{h5_id}/");
    let mut has_file = false;
    for index in 0..archive.len() {
        let entry = archive.by_index(index).map_err(|_| {
            Error::new("config", "upload.resource contains an unreadable ZIP entry")
        })?;
        let name = entry.name();
        if !name.starts_with(&prefix)
            || name.contains('\\')
            || name.split('/').any(|part| matches!(part, "." | ".."))
        {
            return Err(Error::new(
                "config",
                "ZIP entries must all be inside the <h5Id>/ directory",
            ));
        }
        has_file |= !entry.is_dir();
    }
    if !has_file {
        return Err(Error::new(
            "config",
            "ZIP must contain files inside the <h5Id>/ directory",
        ));
    }
    Ok(())
}

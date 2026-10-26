//! Try zarchive-rust the way an application uses it after `cargo add zarchive-rust`.
//!
//! `cargo run` writes sample archives under `out/` next to this crate.

use std::fs;
use std::path::{Path, PathBuf};

use zarchive_rust::{
    create_archive, create_empty, format_from_path, format_infos, is_archive, library_description,
    library_name, library_version, selfextract_append, selfextract_check, selfextract_read,
    selfextract_remove, ArchiveReader, ArchiveWriter, Error, Result, ARC_EXTRACT_DEFAULT,
    ARCHIVE_7Z, ARCHIVE_BZ, ARCHIVE_GZ, ARCHIVE_RAR, ARCHIVE_Z, ARCHIVE_ZIP, VERSION,
};

fn out_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("out")
}

fn section(title: &str) {
    println!();
    println!("== {title}");
}

fn show(label: &str, value: impl std::fmt::Display) {
    println!("  {label}: {value}");
}

fn show_text(label: &str, bytes: &[u8]) {
    match std::str::from_utf8(bytes) {
        Ok(text) => show(label, text),
        Err(_) => show(label, format!("{bytes:?}")),
    }
}

fn reset_out() -> Result<PathBuf> {
    let out = out_dir();
    if out.is_dir() {
        fs::remove_dir_all(&out).map_err(|err| Error::new(err.to_string()))?;
    }
    fs::create_dir_all(&out).map_err(|err| Error::new(err.to_string()))?;
    Ok(out)
}

fn showcase_library() -> Result<()> {
    section("Library");
    show("name", library_name()?);
    show("description", library_description()?);
    show("version", format!("{:#x}", library_version()?));
    show("package", VERSION);
    Ok(())
}

fn showcase_formats() -> Result<()> {
    section("Registered formats");
    for info in format_infos()? {
        let exts = if info.extensions.is_empty() {
            "-"
        } else {
            info.extensions.as_str()
        };
        show(
            &info.name,
            format!(
                "type={} creatable={} exts={exts}",
                info.trait_name, info.creatable
            ),
        );
    }
    for path in ["notes.zip", "pack.7z", "data.tar", "mail.gz", "readme.txt"] {
        match format_from_path(path) {
            Ok(kind) => show(path, kind),
            Err(err) => show(path, err.to_string()),
        }
    }
    Ok(())
}

fn showcase_zip(out: &Path) -> Result<PathBuf> {
    section("Create a zip");
    let disk = out.join("on-disk.txt");
    fs::write(&disk, "from disk").map_err(|err| Error::new(err.to_string()))?;
    let archive_path = out.join("bundle.zip");
    let mut writer = ArchiveWriter::create(&archive_path, Some(ARCHIVE_ZIP))?;
    writer.write_folder("docs")?;
    writer.write_bytes("docs/readme.txt", b"hello from memory")?;
    writer.write_bytes("empty.txt", b"")?;
    writer.write_file(&disk, "on-disk.txt")?;
    writer.finish()?;
    show("path", archive_path.display());
    show("identify", is_archive(&archive_path)?);

    let plain = out.join("plain.txt");
    fs::write(&plain, "not an archive").map_err(|err| Error::new(err.to_string()))?;
    show("plain text is archive", is_archive(&plain)?);

    section("List, read, and verify");
    let mut archive = ArchiveReader::open(&archive_path)?;
    show("files", archive.file_count()?);
    show("folders", archive.folder_count()?);
    show("solid", archive.is_solid()?);
    show("names", archive.namelist()?.join(", "));
    for info in archive.infolist()? {
        let kind = if info.is_dir { "dir" } else { "file" };
        show(&info.filename, format!("{kind} size={}", info.file_size));
    }
    show("contains docs/readme.txt", archive.contains("docs/readme.txt")?);
    show_text("readme", &archive.read("docs/readme.txt")?);
    show_text("on-disk", &archive.read("on-disk.txt")?);
    let (ok, good, bad) = archive.verify()?;
    show("verify", format!("ok={ok} good={good} bad={bad}"));
    show("verify readme", archive.verify_file("docs/readme.txt")?);

    section("Extract");
    let dest = out.join("extracted");
    fs::create_dir_all(&dest).map_err(|err| Error::new(err.to_string()))?;
    show(
        "extract file",
        archive.extract_file("on-disk.txt", &dest, ARC_EXTRACT_DEFAULT)?,
    );
    show(
        "extract folder",
        archive.extract_folder("docs", &dest, ARC_EXTRACT_DEFAULT)?,
    );
    show(
        "extract all",
        archive.extract_all(out.join("extracted-all"), ARC_EXTRACT_DEFAULT)?,
    );
    drop(archive);
    show_text(
        "extracted on-disk",
        &fs::read(dest.join("on-disk.txt")).map_err(|err| Error::new(err.to_string()))?,
    );
    show_text(
        "extracted readme",
        &fs::read(dest.join("docs").join("readme.txt")).map_err(|err| Error::new(err.to_string()))?,
    );
    Ok(archive_path)
}

fn showcase_nested(out: &Path, inner_zip: &Path) -> Result<()> {
    section("Nested archive");
    let blob = fs::read(inner_zip).map_err(|err| Error::new(err.to_string()))?;
    let outer = out.join("outer.zip");
    create_archive(
        &outer,
        &[("bundle.zip", blob.as_slice()), ("note.txt", b"outside")],
        &[],
        Some(ARCHIVE_ZIP),
    )?;
    let mut archive = ArchiveReader::open(&outer)?;
    let recovered = archive.read("bundle.zip")?;
    show("outer names", archive.namelist()?.join(", "));
    drop(archive);
    let nested = out.join("recovered.zip");
    fs::write(&nested, &recovered).map_err(|err| Error::new(err.to_string()))?;
    let mut archive = ArchiveReader::open(&nested)?;
    show_text("nested readme", &archive.read("docs/readme.txt")?);
    Ok(())
}

fn showcase_other_formats(out: &Path) -> Result<()> {
    section("Other creatable formats");
    for (name, archive_type, empty) in [
        ("empty.7z", ARCHIVE_7Z, true),
        ("empty.rar", ARCHIVE_RAR, true),
        ("stream.gz", ARCHIVE_GZ, false),
        ("stream.bz2", ARCHIVE_BZ, false),
        ("stream.za", ARCHIVE_Z, false),
    ] {
        let path = out.join(name);
        if empty {
            create_empty(&path, Some(archive_type))?;
        } else {
            ArchiveWriter::create(&path, Some(archive_type))?.finish()?;
        }
        show(
            name,
            format!(
                "identify={} format={}",
                is_archive(&path)?,
                format_from_path(&path)?
            ),
        );
    }

    let rar_path = out.join("hello.rar");
    create_archive(
        &rar_path,
        &[("hello.txt", b"rar-out".as_slice())],
        &[],
        Some(ARCHIVE_RAR),
    )?;
    let mut archive = ArchiveReader::open(&rar_path)?;
    show("rar names", archive.namelist()?.join(", "));
    show_text("rar hello", &archive.read("hello.txt")?);
    Ok(())
}

fn showcase_self_extract(out: &Path) -> Result<()> {
    section("Windows self-extract");
    if !cfg!(windows) {
        show("skipped", "self-extract trailers are a Windows build of the library");
        return Ok(());
    }
    let payload_path = out.join("sfx-payload.zip");
    create_archive(
        &payload_path,
        &[("note.txt", b"from-exe".as_slice())],
        &[],
        Some(ARCHIVE_ZIP),
    )?;
    let payload = fs::read(&payload_path).map_err(|err| Error::new(err.to_string()))?;
    let exe = out.join("setup.exe");
    let mut original = b"MZ".to_vec();
    original.extend(std::iter::repeat(0u8).take(62));
    original.extend_from_slice(b"windows-stub");
    fs::write(&exe, &original).map_err(|err| Error::new(err.to_string()))?;
    show("before", selfextract_check(&exe)?);
    show("trailer bytes", selfextract_append(&exe, &payload)?);
    show("after", selfextract_check(&exe)?);
    let embedded = selfextract_read(&exe)?;
    show("embedded matches zip", embedded == payload);
    selfextract_remove(&exe)?;
    let restored = fs::read(&exe).map_err(|err| Error::new(err.to_string()))?;
    show(
        "restored",
        restored == original && !selfextract_check(&exe)?,
    );
    Ok(())
}

fn main() -> Result<()> {
    let out = reset_out()?;
    showcase_library()?;
    showcase_formats()?;
    let bundle = showcase_zip(&out)?;
    showcase_nested(&out, &bundle)?;
    showcase_other_formats(&out)?;
    showcase_self_extract(&out)?;
    section("Done");
    show("output", out.display());
    Ok(())
}

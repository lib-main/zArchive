"""Try zarchive-py the way it is used after installation.

    pip install zarchive-py
    python demo.py

The wheel ships the native library. This script imports the installed
zarchive_py package and writes sample archives under out/ next to itself.
"""

from __future__ import annotations

import os
import shutil
import sys

import zarchive_py


OUT = os.path.join(os.path.dirname(__file__), "out")


def section(title: str) -> None:
    print()
    print(f"== {title}")


def show(label: str, value: object) -> None:
    print(f"  {label}: {value}")


def reset_out() -> None:
    if os.path.isdir(OUT):
        shutil.rmtree(OUT)
    os.makedirs(OUT)


def showcase_library() -> None:
    section("Library")
    lib = zarchive_py.get_library()
    show("name", lib.lib_name())
    show("description", lib.lib_description())
    show("version", hex(lib.lib_version()))
    show("package", zarchive_py.__version__)
    show("native", zarchive_py.resolve_dll_path())


def showcase_formats() -> None:
    section("Registered formats")
    for info in zarchive_py.format_infos():
        exts = ", ".join(info.extensions) if info.extensions else "-"
        show(info.name, f"type={info.archive_type} creatable={info.creatable} exts={exts}")
    samples = ("notes.zip", "pack.7z", "data.tar", "mail.gz", "readme.txt")
    for path in samples:
        show(path, zarchive_py.format_from_path(path))


def showcase_zip() -> str:
    section("Create a zip")
    disk = os.path.join(OUT, "on-disk.txt")
    with open(disk, "w", encoding="utf-8") as handle:
        handle.write("from disk")
    archive_path = os.path.join(OUT, "bundle.zip")
    with zarchive_py.ArchiveWriter(archive_path) as writer:
        writer.mkdir("docs")
        writer.writestr("docs/readme.txt", "hello from memory")
        writer.writestr("empty.txt", b"")
        writer.write(disk, "on-disk.txt")
    show("path", archive_path)
    show("identify", zarchive_py.identify(archive_path))

    plain = os.path.join(OUT, "plain.txt")
    with open(plain, "w", encoding="utf-8") as handle:
        handle.write("not an archive")
    show("plain text is archive", zarchive_py.identify(plain))

    section("List, read, and verify")
    with zarchive_py.ArchiveReader(archive_path) as arc:
        show("files", arc.file_count)
        show("folders", arc.folder_count)
        show("solid", arc.solid)
        show("names", arc.namelist())
        for info in arc.infolist():
            kind = "dir" if info.is_dir else "file"
            show(info.filename, f"{kind} size={info.file_size}")
        show("contains docs/readme.txt", "docs/readme.txt" in arc)
        show("readme", arc.read("docs/readme.txt"))
        show("on-disk", arc.read("on-disk.txt"))
        show("verify", arc.verify())
        show("verify readme", arc.verify_file("docs/readme.txt"))

        section("Extract")
        dest = os.path.join(OUT, "extracted")
        os.makedirs(dest)
        show("extract file", arc.extract_file("on-disk.txt", dest))
        show("extract folder", arc.extract_folder("docs", dest))
        show("extract all", arc.extractall(os.path.join(OUT, "extracted-all")))
    with open(os.path.join(dest, "on-disk.txt"), "rb") as handle:
        show("extracted on-disk", handle.read())
    with open(os.path.join(dest, "docs", "readme.txt"), "rb") as handle:
        show("extracted readme", handle.read())
    return archive_path


def showcase_nested(inner_zip: str) -> None:
    section("Nested archive")
    with open(inner_zip, "rb") as handle:
        blob = handle.read()
    outer = os.path.join(OUT, "outer.zip")
    zarchive_py.create_archive(outer, entries=[("bundle.zip", blob), ("note.txt", "outside")])
    with zarchive_py.ArchiveReader(outer) as arc:
        recovered = arc.read("bundle.zip")
        show("outer names", arc.namelist())
    nested = os.path.join(OUT, "recovered.zip")
    with open(nested, "wb") as handle:
        handle.write(recovered)
    with zarchive_py.ArchiveReader(nested) as arc:
        show("nested readme", arc.read("docs/readme.txt"))


def showcase_other_formats() -> None:
    section("Other creatable formats")
    for name, archive_type in (
        ("empty.7z", zarchive_py.ARCHIVE_7Z),
        ("empty.rar", zarchive_py.ARCHIVE_RAR),
        ("stream.gz", zarchive_py.ARCHIVE_GZ),
        ("stream.bz2", zarchive_py.ARCHIVE_BZ),
        ("stream.za", zarchive_py.ARCHIVE_Z),
    ):
        path = os.path.join(OUT, name)
        if name.startswith("empty."):
            zarchive_py.create_empty(path, archive_type)
        else:
            with zarchive_py.ArchiveWriter(path, archive_type) as writer:
                writer.finish()
        show(name, f"identify={zarchive_py.identify(path)} format={zarchive_py.format_from_path(path)}")

    rar_path = os.path.join(OUT, "hello.rar")
    with zarchive_py.RarArchive.create(rar_path) as writer:
        writer.writestr("hello.txt", "rar-out")
    with zarchive_py.ArchiveReader(rar_path) as arc:
        show("rar names", arc.namelist())
        show("rar hello", arc.read("hello.txt"))


def showcase_self_extract() -> None:
    section("Windows self-extract")
    if sys.platform != "win32" or not zarchive_py.selfextract_supported():
        show("skipped", "self-extract trailers are a Windows build of the library")
        return
    payload_path = os.path.join(OUT, "sfx-payload.zip")
    with zarchive_py.ZipArchive.create(payload_path) as writer:
        writer.writestr("note.txt", "from-exe")
    with open(payload_path, "rb") as handle:
        payload = handle.read()
    exe = os.path.join(OUT, "setup.exe")
    original = b"MZ" + b"\0" * 62 + b"windows-stub"
    with open(exe, "wb") as handle:
        handle.write(original)
    show("before", zarchive_py.selfextract_check(exe))
    show("trailer bytes", zarchive_py.selfextract_append(exe, payload))
    show("after", zarchive_py.selfextract_check(exe))
    embedded = zarchive_py.selfextract_read(exe)
    show("embedded matches zip", embedded == payload)
    zarchive_py.selfextract_remove(exe)
    with open(exe, "rb") as handle:
        restored = handle.read()
    show("restored", restored == original and not zarchive_py.selfextract_check(exe))


def main() -> None:
    reset_out()
    showcase_library()
    showcase_formats()
    bundle = showcase_zip()
    showcase_nested(bundle)
    showcase_other_formats()
    showcase_self_extract()
    section("Done")
    show("output", OUT)


if __name__ == "__main__":
    main()

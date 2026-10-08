# zArchive

**An archive is a filesystem with a staged editor — not a bag of compressed files.**

zArchive models every archive as a mounted filesystem. Format support sits under three separate interfaces, and a format can implement any subset of them. The container is a detail; the filesystem is the model.

```text
                    ┌───────────────────────────────┐
                    │        CArchiveFileSystem      │
                    │  open · rename · move · delete │
                    │  create folder · enumerate     │
                    │  mount read-only · nested      │
                    └───────────────────────────────┘
                          ▲          ▲          ▲
                ┌─────────┴──┐  ┌────┴─────┐  ┌─┴──────────┐
                │IArchive    │  │IArchive  │  │IArchive    │
                │Reader      │  │Creator   │  │Editor      │
                └────────────┘  └──────────┘  └────────────┘
                          ▲          ▲          ▲
                ┌─────────┴──────────┴──────────┴─────────┐
                │ zip · gzip · bzip2 · tar · 7z · cab      │
                │ rar · lzh · uue · rez · ArchiveZ         │
                └──────────────────────────────────────────┘
```

---

## Three interfaces, independently implemented

Format support sits under three separate interfaces:

| Interface          | Responsibility                                  |
| ------------------ | ----------------------------------------------- |
| `IArchiveReader`   | Open and read members.                          |
| `IArchiveCreator`  | Create a new archive.                           |
| `IArchiveEditor`   | Modify a tree before the file is rewritten.     |

A format can implement **any subset** of those three. Nothing pretends to be more than it is.

`CArchiveClasses` registers each format as a kin-document processor with three runtime ids: reader, editor, creator. Callers ask those independently through `CheckFileReadableByExt`, `CheckFileCreatableByExt`, and `CheckFileEditableByExt`.

`CArchiveFile` sniffs the stream — `IsFormat` on several readers, with extension as a fallback — and wires the matching trio. `CArchive::SaveAs` then replays any reader through a different creator, so **conversion between formats is the same operation as writing.**

---

## Format traits

Traits on each format declare what it actually is:

| Trait                  | Meaning                                          |
| ---------------------- | ------------------------------------------------ |
| `ARC_TRAIT_INTERIORFS` | Has an interior tree (a real filesystem).        |
| `ARC_TRAIT_ENCRYPTION` | Can encrypt.                                     |

| Format       | Interior tree | Encryption |
| ------------ | :-----------: | :--------: |
| gzip         |       —       |     —      |
| bzip2        |       —       |     —      |
| UUE          |       —       |     —      |
| ZIP          |      ✓        |     ✓      |
| 7z           |      ✓        |     ✓      |
| RAR          |      ✓        |     ✓      |
| tar          |      ✓        |     —      |
| cab          |      ✓        |     —      |
| lzh          |      ✓        |     —      |
| Rez          |      ✓        |     —      |
| ArchiveZ     |      ✓        |     ✓      |

gzip, bzip2, and UUE are single-payload streams. The rest are trees.

---

## Archive as a mounted filesystem

`CArchiveFileSystem` implements the same file-system interface as a disk volume:

- open, rename, move, delete
- create folder
- enumerate with filters
- mount read-only

It is published as a plugin slot (`UDE_GetFileSystem`), so other code can treat an archive path as a volume.

### Walking into nested archives

With `ARCFS_OPT_EMBEDARC`, a path component that is itself an archive is opened as the **next volume**:

```text
backup.zip/inner.tar.gz/documents/report.pdf
```

`FindOpenFileArchive` walks the path, builds a section stream over that member, and continues the lookup inside it. A file node holds `m_pEmbededArcFile` for that nested archive.

Members are marked `FILE_ITEM_CORRUPTED` or `FILE_ITEM_ENCRYPTED` as they are read.

---

## Staged editing

`IArchiveEditor` changes the tree **before** the file is rewritten. Nothing is half-applied.

- **Add** a member from a disk path, an `IStream`, a memory block, or an empty placeholder.
- **Rename, restamp, and change attributes** through `SetItemInfo`.
- **Delete** moves the node into a shadow tree. `RecoverItem` puts it back. `GetItemState` reports `AIES_DELETED` or `AIES_MODIFIED`.
- **Edit member bytes** with `CreateStream` / `DestroyStream(commit)`. `STREAM_OPEN_EDITED` makes those unflushed streams visible to a reader.
- **Apply or drop the batch** with `Commit` and `Revert`. `GetModifiedParts` distinguishes metadata changes from content changes.
- **`GetCotypeReader`** is a second reader bound to the edited view.

ZIP's editor is the one that finishes the job: after the base commit it rewrites the central directory and end header, and it can change the archive comment. ArchiveZ's `CzEditor::Commit` and `Revert` return immediately, so that format's editor is a stub — an honest declaration, not a hidden failure.

---

## Members are codec streams

Opening a member does not dump a blob.

`OpenStream` takes `STREAM_OPEN_DECODE` and `STREAM_OPEN_DECRYPT` **separately**, so a caller can read compressed ciphertext or plaintext.

`CArchiveStream` is a codec stream: read and write run the compress and encrypt codecs named on that item. Each open duplicates the backing stream and seeks to the member offset, so **concurrent readers do not share one file position.**

### Embedded bytes or links

A member is either:

- **`FITYPE_EMBED`** — embedded bytes: offset, stored size, original size, codec flags.
- **`FITYPE_LINK`** — a link to an external path.

New content can come from a file, an `IStream`, or a memory block. `IArchiveStreamInfo` exposes stored size, position inside the archive, and compression ratio. Streams can also be memory-mapped through `BeginMemorylize`.

### Per-member codecs

On create, each `IArchiveSaveItem` names its **own** compress and encrypt algorithm. `CCodecAlgMgr` resolves those names to codec plugins.

**Honest storage:** `ACO_SIZECHECK` (on by default) compares the compressed size with the original and, when compression grows the member, rewrites it as **stored**. The ZIP writer cites BZ2 and LZMA on SWF as the case this is for.

### Solid archives and passwords

- `ARCATTR_SOLID` and `IsSolid()` describe solid archives, where members share one compressed stream.
- Passwords go through `ISecretKeyGet` / `ISecretKeySet` rather than a string on the archive object.
- `ARCATTR_NEEDPWD` marks archives that require one.
- `VerifyItem` checks a member **without extracting it**.

---

## Self-extract executables

`CSelfExtractExe` appends a payload to a Windows EXE or DLL:

```text
┌────────────────┬─────────┬────────┬──────────────┬─────────────┬───────┐
│ original file  │ payload │ header │ content size │ header size │ magic │
└────────────────┴─────────┴────────┴──────────────┴─────────────┴───────┘
```

The header can require a password, stored as a hash, and the payload stream can sit on a codec stream. The same class can detect that trailer, attach it as a content stream, or strip it with `Remove` so the executable is restored.

---

## Tasks around the archive

Extraction and "save as" are tasks, not only blocking calls.

- **`CArchiveExtractTask`** walks the tree breadth-first on a thread pool, with separate locks for fetching the next node and for editing the tree. Extract options preserve timestamps. `ExtractAllFiles` can return the paths that failed, and a progress callback is available.
- **`CArchiveSaveAsTask`** is the matching multi-threaded writer.
- **`SetCurrentDirectory`** pairs an archive-relative folder with a disk folder so `WriteItems` can pour a filesystem enumeration in while keeping both hierarchies.

---

## What the Python layer shows

The Python package (`ArchiveReader`, `ArchiveWriter`, `Archive`) covers:

- identify
- list
- read
- verify
- extract
- create

Nested mounts, staged edit, per-member codecs, cross-format `SaveAs`, and self-extract stay on the C++ interfaces.

---

## Design principles

1. **Filesystem first.** Paths, volumes, and members are the primary abstraction.
2. **Three roles, independently implemented.** A format implements only what it genuinely supports.
3. **Capabilities are declared, not assumed.** Traits and runtime ids say what is real.
4. **Separable concerns.** Decode and decrypt are independent; codecs are per-member.
5. **Reversible edits.** A staged batch commits together or rolls back.
6. **Correctness over ratio.** Never store a larger compressed member than the original.

---

## License

See `zLibs-LICENSE` for details.
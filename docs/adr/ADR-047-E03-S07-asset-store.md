# ADR-047. Asset store: images named by hash, type read from content, file before row

- **Status:** Accepted
- **Type:** Architecture
- **Date:** 2026-10-29
- **Story:** E03-S07

## Context

Image blocks (E05-S07) need somewhere to keep the picture. `BLOCK_MODEL.md` already sets the layout: files in
`assets/<blake3-hash>`, and an `assets` table (`asset_id`, `mime`, `bytes`, `created_at`) in the database. What was
left to decide: how a file is written safely, how its type is known, and what happens on a second import.

## Decision

- **API on `Workspace`:**
  - `import_asset(bytes, now)` and `import_asset_file(path, now)` return an `Asset { id, mime, bytes, created_at }`;
  - `asset(id)` reads the record;
  - `asset_path(id)` gives where its file is.
  - The original file is copied, never moved.
- **The name is the hash.**
  - The `AssetId` is the BLAKE3 hash of the bytes, so the same picture imported twice is one file and one row.
  - The second import keeps the first record, including when it was first imported (`INSERT OR IGNORE`).
- **The file is written before the row, atomically.**
  - It is written to `<hash>.tmp` in the same folder, flushed to disk, then renamed into place, so the file is
    either absent or complete.
  - The table never names a missing or half-written file.
  - An existing file is re-hashed before it is trusted; a damaged copy is rewritten by the next import of the
    same image.
- **The type comes from the content.**
  - The first bytes are matched against fixed signatures for PNG, JPEG, GIF, WebP, BMP, TIFF, AVIF, HEIC, and
    SVG (as text).
  - The file name and extension are ignored, so a renamed file can't pass for an image.
  - Anything else is refused with `StoreError::NotAnImage`, and nothing is written.

## Alternatives considered

- **A detection crate (`infer`, `mime_guess`):** `mime_guess` trusts the extension, which is the thing to avoid.
  `infer` knows hundreds of formats Leaf won't store. Nine signatures are about 40 lines with tests.
- **Row first, then file:** a crash between the two would leave a row naming a file that doesn't exist.
- **Keeping the extension in the file name (`<hash>.png`):** `BLOCK_MODEL.md` names files by hash alone. Export
  adds an extension from `mime` when it writes Markdown.

## Consequences

- Deleting images that no page uses any more (garbage collection) is not done yet; files simply stay. A later
  story can sweep `assets/` against the image blocks.
- There is no size limit yet; very large files are copied in full.
- Plain English: when you add a picture, Leaf keeps its own copy inside the workspace folder, named by a
  fingerprint of the picture. Adding the same picture again doesn't make a second copy, and only real image
  files are accepted.

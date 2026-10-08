# MoviNight portable snapshots

Open **Settings → Data transfer** to export or import a ZIP. Snapshots transfer progress between MoviNight installations without copying browsing cache. Other applications can read or convert the documented JSON; they must implement this format to import it directly.

The **Include TMDB API key** export checkbox is optional and off by default. A ZIP containing a key is private, unencrypted data. Import previews show whether a key is present and let you keep the destination key. MCP tokens, WebView profiles, browser caches, offline databases, and recovery backups are excluded.

## ZIP layout, format version 1

| Entry | Data |
| --- | --- |
| `manifest.json` | Format identifier `movinight-snapshot`, schema version `1`, application version, creation time in UTC, file sizes and SHA-256 digests, thumbnail index, and unavailable thumbnail paths |
| `data/watched.json` | Watched title records, saved dates, watched seasons, known season counts, and custom record fields |
| `data/white_list.json` | Waitlist title records and original added dates |
| `data/ai_workspace.json` | Active research text/instructions/agent notes, saved research batches, pending research matches, pending suggestions, approved suggestion history, reasons, and recommended seasons |
| `data/snapshot_preferences.json` | Zoom percentage, offline caching preference, and cache capacity setting; no cached objects |
| `data/config.json` | `{}` by default, or an `api_key` string when explicitly included |
| `thumbnails/<hash>.<jpg/png/webp>` | Only artwork referenced by exported progress |
| `README.txt` | Portable format and privacy notes |

Title identity is **TMDB ID plus `content_type`**, where the type is `movie` or `tv`. Dates and record metadata retain their saved values. The thumbnail index maps each original TMDB poster path to its ZIP filename and MIME type. Missing thumbnails are listed in the manifest and reported after export; the progress data remains complete. Already saved artwork is used first; export attempts to download missing thumbnails at a bounded size.

## Import behavior

Preview validates the complete ZIP before changing progress. Files, sizes, checksums, record types, thumbnail mappings, and schema version are checked. An import verifies the same ZIP digest again to detect changes after preview. Archives with unexpected filenames, path traversal, duplicate entries, symbolic links, or invalid records are rejected. Supported archives are limited to 512 MB of expanded data, 32 MB per JSON file, 8 MB per thumbnail, and 20,000 entries.

**Merge** is the default. Existing watched dates and fields win on conflicting titles; checked seasons are combined, and the largest known season count is retained. Movies and TV with the same numeric ID remain separate. Watched titles are removed from Waitlist. Research batches with different IDs remain accessible through **Reel Research → Saved research batches**. Pending records merge by their persistent record IDs; approved records do not revert to pending. Repeating the same import does not duplicate records.

**Replace** requires an explicit in-app checkbox. It restores the snapshot progress, removes destination-only progress, and keeps the destination TMDB key unless the included key is selected. Watched takes precedence if a legacy snapshot contains the same title in both lists.

Before changing progress, the app creates a backup under `backups/snapshot-import-<id>`. A journal and verified original files allow rollback if writing fails or the app exits during import. Recovery runs before app commands at the next launch. Imported thumbnails are stored separately from browsing cache and remain available when offline caching is disabled. Import does not bring any cache objects from the source installation.

# What's new in the current MoviNight UI

These features are included in the [published MoviNight 1.3.3 Windows release](https://github.com/mahostar/MoviNight/releases/tag/MoviNight-v1.3.3), with setup EXE and MSI installers.

## 1.3.3 — portable progress snapshots

- Export watched dates and seasons, waitlist, saved research, pending matches, suggestions and approved history, preferences, and referenced thumbnails in a versioned ZIP.
- Exclude browsing cache and connection tokens. Offer TMDB key inclusion as an option, off by default.
- Validate and preview imports. Default to merging progress, with explicit replacement available.
- Preserve watched progress, combine seasons, retain research archives and approved status, and recover interrupted imports.

![Data transfer](../branding/screenshots/v1.3.3/data-transfer.png)

## 1.3.2 — broader AI Discovery and approval behavior

- Search All movies and TV in Discovery through the UI and MCP.
- Give agents filter menus, up to 1,000 useful records per call and continuation cursors.
- Publish ordinary Discovery recommendations without requiring pasted research.
- Keep pending recommendations and approved history; preserve watched dates and progress on approval.
- Show aired unwatched season picks in **Watched → AI season suggestions**.
- Apply incomplete-rating bounds before pagination so cleaner filters do not hide whole highest/lowest-rated pages.

![Discovery recommendations](../branding/screenshots/v1.3.3/ai-suggestions.png)

## 1.3.1 — poster cards and detailed review

- Present research matches as poster-led cards.
- Show the original request, canonical metadata and explanation in the detail popup.
- Offer approval/removal from cards and details, with watched/waitlist indicators and actions.

![Detailed review](../branding/screenshots/v1.3.3/match-details.png)

Read the [user guide](user-guide.md) or [architecture diagrams](architecture.md) for the complete current feature set.

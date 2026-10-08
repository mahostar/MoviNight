# MoviNight user guide

## Set up the app

Install a Windows build, open **Settings → TMDB & library**, enter your TMDB API key and save it. The key enables online catalogue requests. Progress is stored locally under `%APPDATA%\movinight`.

Keep the same installation scope when upgrading. Export a snapshot or use **Back up library** before moving devices. An upgrade does not require clearing your library.

## Discovery and search

**Discovery** supports All, Movies and TV Shows. All interleaves the formats' independent rankings. TV year bounds refer to a show's premiere, not each season's date.

| Control | Meaning |
| --- | --- |
| Genres | Any selected genre can match; a title need not match every genre. |
| Original language | The work's original language, not a promise of dubbed audio or subtitles. |
| Streaming | Search and select services; multiple providers match any selection. UI availability uses the US region. |
| Years | Restrict movie release dates or TV first-air dates. |
| Sort & rating | Choose popularity, date or rating order and a minimum TMDB rating. Consider vote counts when evaluating recommendations. |
| Hide animation | Exclude animation from results. |
| Hide spam / incomplete | Hide missing posters and ratings exactly 0 or 10. This is a completeness heuristic, not a quality assessment. |

Use **Discover titles** to apply filters, **Reset** to clear them, and **Refresh** for fresh TMDB responses. Load more results as you browse. Card indicators identify watched or waitlisted titles.

**Search** looks up movie and TV names together and can load more results. Open a card for synopsis, genres, ratings, runtime or TV information and available trailers. Trailers require internet.

## Watched, Waitlist and seasons

Save titles to **Waitlist** for later. Mark titles watched to record dates in **Watched**. Both libraries can be filtered by format.

For a watched TV show, open its details, check seasons in the **Season Tracker** and save. Update checks compare saved progress with aired seasons. Older entries without tracked seasons receive a baseline rather than an invented list of remaining seasons.

**Watched → AI season suggestions** lists recommendations for aired, unchecked seasons. Approval preserves the viewing date and checked seasons; it does not mark the new season watched. When all recommended seasons are checked off, the pick leaves that section while approved Suggestions history remains.

## Connect an AI agent

1. Start **Settings → MCP Server**.
2. Configure your local MCP client with the displayed address and bearer token.
3. Keep MoviNight open while it works. Reconnect with the new token after restarting the server.
4. Ask your connected agent to research and publish picks to Suggestions.

The full [MCP guide](../MCP_GUIDE.md) is also available through **Copy documentation**, even when stopped. MoviNight does not include a model. A remote service cannot directly reach its loopback listener.

Ordinary Discovery research needs no pasted list. For example:

> Read my watched history and waitlist. Find well-rated older mysteries I missed, excluding saved titles. Research large Discovery batches and publish a shortlist with reasons.

Agents can select format, years, genres, language, provider, region, sorting and rating. They can absorb up to 1,000 useful records per call and continue with a cursor. Responses include useful metadata and progress rather than poster and trailer URLs.

## Reel Research

Paste plain text, tables, CSV or spreadsheet cells, add instructions, and save the batch. Copy its prompt to your connected agent. It searches canonical TMDB titles, verifies year/type conflicts and proposes matches without directly adding them to Waitlist.

Review **Suggestions → Research matches** or **Waitlist → Review AI matches**. Detail popups retain the original requested title and explanation. Agent notes report completed or unresolved entries. Saved batches remain accessible when research changes or snapshots merge.

## Suggestions and approval

Use the Suggestions brief to specify source, format, mood, available time, language and count, then copy it to your client. The agent can also use your direct request. Picks can come from Discovery, your waitlist, watched history for rewatches, or aired unwatched TV seasons.

| Condition or action | Result |
| --- | --- |
| Approve a new title | Add once to Waitlist and retain approved Suggestions history. |
| Approve a waitlisted title | Preserve its existing entry and saved date. |
| Approve a watched title | Keep dates, seasons and custom fields in Watched; remove any duplicate waitlist entry. |
| Mark watched before reviewing | Approval uses the current watched status. |
| Approve a season pick | Preserve progress and keep unchecked recommended seasons in the Watched section. |
| Reject or dismiss pending | Leave libraries unchanged. The title may be recommended in a later request. |
| Dismiss approved history | Remove only the recommendation record. |
| Publish the same pending pick again | Update its reason rather than creating another pending card. |

Only in-app review can approve. MCP has no approval, deletion or mark-watched tool.

## Offline data and cache controls

With caching enabled, fetched metadata and artwork stay on this device. **Download saved library** retries or completes saved-title downloads. Settings shows usage and a selectable 1–5 GB cap. The 4 GB default is a maximum, not a reserved allocation.

Downloaded pages can be reused during connection failures or TMDB server outages, with offline data identified in the app. Offline filters/pages must have been fetched previously; the complete catalogue is not downloaded. Authentication and rate-limit failures remain visible. Trailers require internet.

**Clear Discover cache** removes Discovery responses and exclusively associated browsing artwork. Library progress, saved-library downloads and other cached categories remain. Disabling caching stops new downloads and offline fallback without deleting existing objects.

## Backups and snapshots

**Back up library** copies watched, waitlist and AI workspace data to a timestamped backup. **Data transfer** creates a portable ZIP with progress, research, suggestions, preferences and saved-title thumbnails, excluding browsing cache.

**Include TMDB API key** is off by default. An included key is unencrypted. Import previews indicate whether a key is present and allow you to keep the destination key.

**Merge** is the default. Existing watched dates/fields win conflicts, checked seasons combine, and watched titles are removed from Waitlist. Research archives survive and approved records do not revert to pending. Repeat imports avoid duplicate progress records.

**Replace** requires an explicit choice and confirmation. Both modes validate the archive and create a recovery backup. Snapshots transfer directly between MoviNight installations; other apps need format support or a converter. See [the specification](../SNAPSHOT_FORMAT.md).

## Display

Settings saves app zoom from **75% to 175%**. Snapshots include this preference for restoration on import.

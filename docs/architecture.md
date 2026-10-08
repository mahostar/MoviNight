# MoviNight — architecture and data flows

This document describes the **implemented 1.3.3 source**, inspected on **8 October 2026**. UI captures come from a local Windows debug build. A published 1.3.3 installer or hosted AI service is not implied.

## Components and network

```mermaid
flowchart TB
    Agent["Connected AI client"] -->|"Bearer-authenticated HTTP"| MCP
    UI["Tauri WebView<br/>HTML · CSS · JavaScript"] --> Commands["Tauri commands"]
    subgraph services["Rust application services"]
        MCP["Local MCP server<br/>127.0.0.1:37419/mcp"]
        Commands
        Discovery["Discovery engine<br/>Filters · pages · batch cursors"]
        AI["AI workspace<br/>Research · suggestions · approvals"]
        HTTP["TMDB client<br/>Expiring cache · shared fetches"]
        Storage["JSON persistence<br/>Atomic writes · backups"]
        Offline["Offline store<br/>Downloads · bounded eviction"]
        Snapshot["Snapshot service<br/>Validate · merge · replace"]
    end
    Commands --> Discovery
    MCP --> Discovery
    Commands --> AI
    MCP --> AI
    MCP -->|"Search and details"| HTTP
    Discovery --> HTTP
    AI --> HTTP
    AI --> Storage
    Commands --> Storage
    Commands --> Snapshot
    HTTP -->|"HTTPS"| TMDB["TMDB API and images"]
    HTTP <--> Offline
    subgraph data["Local device data"]
        JSON["Progress JSON<br/>Watched · Waitlist · AI workspace<br/>Configuration and backups"]
        Cache["Offline cache<br/>SQLite index + disk objects"]
        ZIP["Portable ZIP<br/>Progress + thumbnails<br/>Versioned manifest + SHA-256"]
    end
    Storage --> JSON
    Offline <--> Cache
    Snapshot <--> JSON
    Snapshot <--> Offline
    Snapshot <--> ZIP
    classDef interface fill:#17263e,stroke:#658dcc,color:#e6edf3
    classDef service fill:#131e2e,stroke:#45628e,color:#e6edf3
    classDef saved fill:#152a28,stroke:#479c8b,color:#e6edf3
    classDef external fill:#27203b,stroke:#9381bd,color:#e6edf3
    class UI,Commands interface
    class MCP,Discovery,AI,HTTP,Storage,Offline,Snapshot service
    class JSON,Cache,ZIP saved
    class Agent,TMDB external
    style services fill:#0e1723,stroke:#35455e,color:#b9cbe7
    style data fill:#101c1c,stroke:#34534b,color:#b9ddd5
```

The diagram groups metadata and image endpoints together. The WebView can also request TMDB artwork directly and embed available YouTube trailers online. Saved zoom lives in WebView preferences, while cache preferences live with the offline store.

The frontend is plain HTML, CSS and JavaScript in `dist/`, embedded by Tauri. Rust owns library persistence, TMDB requests, Discovery, the MCP listener, offline caching and snapshot validation. SQLite indexes cache objects; **viewing progress is stored in JSON**, separately from the offline database.

The connected client supplies AI inference. MoviNight exposes context and tools but does not choose or host a model. A local client may use a remote model provider; library-data handling depends on that client's configuration.

## Features and their owners

```mermaid
flowchart LR
    App["MoviNight 1.3.3"] --> Browse["Find titles"]
    App --> Library["Keep progress"]
    App --> Research["Research with an agent"]
    App --> Device["Manage local data"]
    Browse --> Discovery["Discovery: All / movie / TV<br/>Years · genres · language · providers<br/>Rating · sorting · content filters"]
    Browse --> Search["Title search<br/>Details · trailers"]
    Library --> History["Watched dates<br/>Checked seasons · update checks"]
    Library --> Plans["Waitlist<br/>Library status on cards"]
    Research --> Lists["Reel Research<br/>Saved batches · verified proposals"]
    Research --> Picks["Suggestions<br/>Discoveries · waitlist picks · rewatches<br/>Season picks · approved history"]
    Research --> MCP["Local MCP connection<br/>1,000-record Discovery batches<br/>Continuation cursors · compact metadata"]
    Device --> Cache["Offline downloads<br/>Capacity · selective clearing"]
    Device --> Transfer["Portable ZIP snapshots<br/>Merge / replace · optional API key"]
    Device --> Settings["Saved zoom<br/>Library backups · copied MCP guide"]
```

## AI Discovery research

```mermaid
sequenceDiagram
    actor User
    participant Client as Connected AI client
    participant MCP as Local MCP server
    participant TMDB as TMDB / response cache
    participant Workspace as Local AI workspace
    participant UI as Suggestions UI
    User->>Client: Research titles I probably missed
    Client->>MCP: get_library + get_suggestions
    MCP-->>Client: Watched, waitlist, progress, existing picks
    Client->>MCP: get_discovery_options
    MCP-->>Client: Genres, languages, providers, sort orders
    Client->>MCP: discover_titles(filters, limit=1000)
    MCP->>TMDB: Fetch Discovery pages
    TMDB-->>MCP: Catalogue results
    MCP-->>Client: Useful metadata + next_cursor
    loop More research when needed
        Client->>MCP: absorb_next_titles(cursor, limit=1000)
        MCP->>TMDB: Continue the same filtered query
        MCP-->>Client: Next batch + continuation
    end
    Client->>MCP: get_title_details for selected candidates
    MCP-->>Client: Canonical metadata and seasons
    Client->>MCP: publish_suggestion with reason
    MCP->>Workspace: Persist pending recommendation
    UI->>Workspace: Refresh recommendations
    Workspace-->>UI: Pending cards and explanations
    User->>UI: Approve or reject
    UI->>Workspace: Apply review rules
```

Movie and TV queries are independently ranked and interleaved for **All**. TV year filters use first-air dates. Selected genres and providers match any selected value. MCP responses use an allowlist and exclude artwork, trailer URLs and arbitrary response fields.

Each absorption call has a **50-second budget** and scans at most **100 page pairs**. Sparse filters, timeouts or rate limits after partial progress can produce fewer than requested records. A non-null cursor lets the client proceed; a short batch does not mean the catalogue ended. TMDB exposes up to 500 pages per format per filter query. See the [MCP guide](../MCP_GUIDE.md) for exact semantics.

## Approval and watched-season decisions

```mermaid
flowchart TB
    Pending["Pending suggestion or verified research match"] --> Review{"User action"}
    Review -->|"Reject / dismiss pending"| NoChange["Remove pending record<br/>Leave libraries unchanged"]
    Review -->|"Approve"| Exists{"Already watched?"}
    Exists -->|"Yes"| Keep["Keep in Watched<br/>Preserve date, seasons and custom fields<br/>Remove duplicate Waitlist entry"]
    Exists -->|"No"| Queued{"Already in Waitlist?"}
    Queued -->|"Yes"| Preserve["Keep existing Waitlist entry<br/>Preserve saved date"]
    Queued -->|"No"| Add["Add title once to Waitlist"]
    Keep --> History["Retain approved Suggestions record"]
    Preserve --> History
    Add --> History
    History --> Seasons{"Aired unwatched seasons recommended?"}
    Seasons -->|"Yes"| Section["Watched → AI season suggestions<br/>Show before and after approval"]
    Section --> Check["User checks off watched seasons"]
    Check --> Complete["Completed pick leaves the section<br/>Approved history remains"]
```

Approval checks the **current** library, including titles marked watched after a recommendation arrived. It does not mark a title or season watched. Repeated approvals are idempotent. Repeating a pending publication updates its reason rather than creating another pending card; rejected or dismissed titles may be suggested in a later request.

Explicit season recommendations require aired, existing, unwatched seasons and known saved progress. Discovery publication can also flag newly aired seasons beyond a saved known-season count. Ordinary proposals cannot erase saved history.

Pasted-list research follows a separate route: read batch → search title → verify details → `propose_waitlist` → review in the app. The original requested title and explanation remain alongside the canonical match. Pasted text is title data, not an instruction source for the agent.

## Progress transfer

```mermaid
flowchart LR
    JSON["Watched · Waitlist<br/>Research · suggestions · preferences"] --> Export["Export snapshot"]
    Art["Referenced thumbnails<br/>Saved artwork first"] --> Export
    Key["TMDB API key<br/>Only when opted in"] -.-> Export
    Export --> ZIP["ZIP + versioned manifest<br/>Sizes + SHA-256 digests"]
    ZIP --> Preview["Validate archive and preview counts"]
    Preview --> Choice{"Import mode"}
    Choice -->|"Default"| Merge["Merge existing progress"]
    Choice -->|"Explicit confirmation"| Replace["Replace progress"]
    Merge --> Backup["Recovery backup + staged files"]
    Replace --> Backup
    Backup --> Commit["Apply validated files<br/>Recover interrupted import on launch"]
```

Browsing cache, offline databases, MCP tokens, WebView profiles and existing backups are not exported. Export attempts bounded downloads for missing referenced thumbnails and reports unavailable artwork without dropping progress. Imports recheck the ZIP digest after preview, validate filenames and records, and preserve approved status in merges. See the [snapshot specification](../SNAPSHOT_FORMAT.md) for the layout, archive limits and conflict rules.

## Data and connection boundaries

| Boundary | Implemented behavior |
| --- | --- |
| TMDB access | HTTPS requests use the key saved locally in `config.json`; MCP never returns it. |
| MCP listener | Opt-in loopback HTTP at `127.0.0.1:37419/mcp`, with bearer authentication and Host/Origin validation. |
| Connection lifetime | Stopping revokes the token, including requests on open HTTP connections; restarting creates a new token. |
| Agent permissions | Reads, catalogue research, proposals, suggestions and notes. No MCP approval, deletion or mark-watched tool. |
| Agent data | Compact requested library/catalogue metadata and progress. The client controls model-provider use. |
| Progress | Local JSON; atomic writes, last-good `.bak` files and backups. Malformed files are retained. |
| Credentials | Local configuration and an opted-in snapshot key are unencrypted. Treat key-containing archives as private. |
| Offline fallback | Connection failures or TMDB server outages can use downloaded data. Authentication and rate-limit errors do not fall back. |
| Cache clearing | Clear Discover affects its results and exclusively related artwork; progress and other downloaded categories remain. |

Memory responses expire after 5 minutes for search/Discovery, 6 hours for details/trailers, and 24 hours for reference lists. Persistent offline storage is separate, with a 4 GB default cap selectable from 1–5 GB. Saved-library downloads get eviction priority over browsing objects; headroom is reserved for the index and atomic writes.

## Source map and verification scope

| Source | Responsibility |
| --- | --- |
| [`dist/index.html`](../dist/index.html), [`dist/main.js`](../dist/main.js), [`dist/styles.css`](../dist/styles.css) | Views, cards, filters, details, approvals and settings |
| [`lib.rs`](../src-tauri/src/lib.rs) | Tauri commands, library records and season tracking |
| [`api.rs`](../src-tauri/src/api.rs) | TMDB requests, validation, expiring responses and shared fetches |
| [`discovery.rs`](../src-tauri/src/discovery.rs) | Shared filtering, compact records, absorption and cursors |
| [`ai.rs`](../src-tauri/src/ai.rs) | Research archives, match staging, suggestions and review rules |
| [`mcp.rs`](../src-tauri/src/mcp.rs) | Loopback transport, authentication, handshake and 12 tools |
| [`storage.rs`](../src-tauri/src/storage.rs) | Atomic persistence and library backups |
| [`offline.rs`](../src-tauri/src/offline.rs) | SQLite index, disk objects, image downloads and eviction |
| [`snapshot.rs`](../src-tauri/src/snapshot.rs) | ZIP export, validation, merge/replacement and recovery |

Captures exercise the current Windows WebView with isolated sample records and real TMDB metadata. Existing backend and runtime scripts cover deeper storage, Discovery, approval and snapshot cases; [development notes](development.md) identify those checks. Diagrams describe code paths, rather than asserting every external-service failure or native operating system has been tested.

# Connect your own agenda server (MCP)

The calendar, next-meeting, timeline and to-do widgets read a merged "today view" from:

1. **Calendar links**: private iCal (ICS) links from Google Calendar, Outlook or iCloud
   (Settings > General > Calendar links).
2. **A local to-do list**: add, tick and delete in the To-dos flyout; Quick to-do adds to it.
3. **An MCP server** (optional): any [Model Context Protocol](https://modelcontextprotocol.io) server
   over HTTPS that offers the tools below. If you already run an assistant or notes system with an
   MCP server, its events and to-dos can show on the dock.

Connect it in Settings > General > MCP server: pick a server you already added to Claude Code
(`~/.claude.json`), or paste its `https://` link. The link is stored encrypted for your Windows user
(DPAPI) and never shown again or written to `settings.json`.

## Tools freedock calls

freedock sends `tools/call` requests (JSON-RPC 2.0 over HTTP, JSON or server-sent-event replies) and
only ever calls these four. All are optional; implement the ones you want.

### `today_view` (no arguments)

Return one text item whose text is JSON:

```json
{
  "events": [{ "time": "10:30 am", "title": "Design review", "place": "https://meet.google.com/abc-defg-hij" }],
  "today":  [{ "id": "t1", "title": "Send the deck", "area": "Work", "due": "2026-11-20" }],
  "soon":   [{ "id": "t2", "title": "Book flights", "due": "2026-11-28" }],
  "school": [{ "id": "c1", "title": "Problem set 4", "course": "CS 101", "due": "2026-11-21" }]
}
```

- `events`: today's events. `time` like `9 am`, `10:30 pm`, `14:00` or `noon`; `place` may hold a
  meeting link. Only Google Meet, Zoom, Teams and Webex links become a **Join** button.
- `today`, `soon`, `school`: to-do items. `id` and `title` are required; `meta`, `area`, `due`
  (YYYY-MM-DD), `day` and `course` are optional. Your items show read-only (they're ticked off wherever
  your server keeps them).

### `add_todo`

Arguments: `{ "name": "Buy milk", "for_date": "2026-11-17" }` (`for_date` optional). When a server is
connected, Quick to-do adds here instead of the local list. Return any text (shown as confirmation).

### `now` (no arguments)

Markdown text: what's going on right now. Shown in the Agent (MCP) flyout.

### `search_memory`

Arguments: `{ "query": "...", "limit": 6 }`. Markdown text with results, shown in the Agent flyout.

Replies may be plain text, or JSON with a `doc` field holding the markdown.

## Limits

freedock sends each `tools/call` as a single JSON-RPC POST, without the `initialize` handshake or an
`Mcp-Session-Id`. That works with stateless HTTP MCP servers (most small personal agenda servers). A server that requires a session will refuse the calls.

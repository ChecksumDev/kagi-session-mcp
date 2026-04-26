# kagi-session-mcp

An MCP server that lets your agent search [Kagi](https://kagi.com) using your existing browser session. No API key, no extra subscription, no extra config: it reads the Kagi cookie from whatever browser you're already logged into.

## Install

Until the first crates.io release, install straight from git:

```sh
cargo install --git https://github.com/checksumdev/kagi-session-mcp --locked
```

This builds and drops `kagi-session-mcp` in `~/.cargo/bin`, which is already on your `PATH` if cargo is set up normally. The `--locked` flag reuses the repo's `Cargo.lock` so you get the same dependency versions that CI tested with. To pin further:

```sh
# pin to a release tag
cargo install --git https://github.com/checksumdev/kagi-session-mcp --tag v0.1.0 --locked

# pin to a specific commit
cargo install --git https://github.com/checksumdev/kagi-session-mcp --rev <sha> --locked
```

Or build locally:

```sh
git clone https://github.com/checksumdev/kagi-session-mcp
cd kagi-session-mcp
cargo build --release
```

The binary ends up at `target/release/kagi-session-mcp`.

## Configure your client

The server speaks plain MCP over stdio, so it works anywhere stdio-based MCP is supported. Click your client:

<details>
<summary><b>Claude Desktop</b></summary>

Edit `claude_desktop_config.json`:

- macOS: `~/Library/Application Support/Claude/claude_desktop_config.json`
- Windows: `%APPDATA%\Claude\claude_desktop_config.json`

```json
{
  "mcpServers": {
    "kagi": {
      "command": "kagi-session-mcp"
    }
  }
}
```

Restart Claude.

</details>

<details>
<summary><b>Claude Code</b></summary>

```sh
claude mcp add kagi -- kagi-session-mcp
```

</details>

<details>
<summary><b>Cursor</b></summary>

Edit `~/.cursor/mcp.json` (or the project-local `.cursor/mcp.json`):

```json
{
  "mcpServers": {
    "kagi": {
      "command": "kagi-session-mcp"
    }
  }
}
```

</details>

<details>
<summary><b>VS Code</b></summary>

Edit `.vscode/mcp.json` in your workspace (or the user-level equivalent):

```json
{
  "servers": {
    "kagi": {
      "type": "stdio",
      "command": "kagi-session-mcp"
    }
  }
}
```

</details>

<details>
<summary><b>Windsurf</b></summary>

Edit `~/.codeium/windsurf/mcp_config.json`:

```json
{
  "mcpServers": {
    "kagi": {
      "command": "kagi-session-mcp"
    }
  }
}
```

</details>

<details>
<summary><b>Cline</b></summary>

In Cline's settings, open `cline_mcp_settings.json` and add:

```json
{
  "mcpServers": {
    "kagi": {
      "command": "kagi-session-mcp",
      "disabled": false
    }
  }
}
```

</details>

<details>
<summary><b>Continue</b></summary>

Edit `~/.continue/config.json`:

```json
{
  "experimental": {
    "modelContextProtocolServers": [
      { "transport": { "type": "stdio", "command": "kagi-session-mcp" } }
    ]
  }
}
```

</details>

<details>
<summary><b>Zed</b></summary>

In `~/.config/zed/settings.json`:

```json
{
  "context_servers": {
    "kagi": {
      "command": { "path": "kagi-session-mcp", "args": [] }
    }
  }
}
```

</details>

<details>
<summary><b>Gemini CLI, OpenCode, anything else</b></summary>

Anything that speaks stdio MCP works. Just point it at `kagi-session-mcp` with no arguments.

</details>

## Manual session token

If your browser is locked down (Chrome v20 ABE, headless setups, sandboxed profiles) you can paste a Session Link token from `kagi.com/settings?p=user_details`:

```json
{
  "mcpServers": {
    "kagi": {
      "command": "kagi-session-mcp",
      "env": { "KAGI_SESSION_TOKEN": "your-session-link-token" }
    }
  }
}
```

## Tools

| Tool | What it does |
| ---- | ------------ |
| `kagi_search` | Web search with operators, lens, time/region/safe filters, pagination |
| `kagi_fastgpt` | Grounded one-shot Q&A with inline `[N]` citations |
| `kagi_wikipedia` | Knowledge-panel lookup |
| `kagi_suggest` | Autocomplete |
| `kagi_news` / `kagi_images` / `kagi_videos` / `kagi_podcasts` | Per-vertical search |
| `kagi_list_lenses` | List the user's configured Kagi Lenses |
| `kagi_fetch` | Authenticated URL fetch (cookie only sent to kagi.com) |
| `kagi_status` | Report which session was discovered and how |

`kagi_search` accepts `query`, `limit`, `page`, `time` (day/week/month/year), `region` (e.g. `us`, `de`), `safe` (off/moderate/strict), and `lens` (toolbar slot 0..7). It supports the full Kagi operator set: `site:`, `filetype:`, `-exclude`, `"exact phrase"`, `before:YYYY-MM-DD`, and `after:YYYY-MM-DD`.

## Troubleshooting

Run `kagi_status` from your agent. If `session_found` is false:

- **`App-Bound Encryption`** (Chrome 127+): the cookie can't be decrypted from outside the browser. Use `KAGI_SESSION_TOKEN` instead.
- **`no kagi session found`**: log in to Kagi in any supported browser, then retry.
- **`session was found but kagi rejected it`**: cookie expired. Reload Kagi in the browser, then retry.

For deeper logs, set `RUST_LOG=kagi_session_mcp=debug` and re-launch the host. Logs go to stderr (stdout is reserved for the MCP JSON-RPC frames).

## Contributing

PRs welcome. There's a short [CONTRIBUTING.md](./CONTRIBUTING.md) covering issue conventions, commit format, and how to handle Kagi changing its markup or routes, which is the most common reason this repo needs patches.

## License

MIT. See the [LICENSE](./LICENSE) file for details.

## Sponsor

If this saves your agent some search time, you can buy me a coffee on [Ko-fi](https://ko-fi.com/checksum). And please pay for Kagi too: agentic search is exactly the kind of usage that funds the work they do.
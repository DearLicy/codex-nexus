# Image MCP fallback

`packages/image-mcp/server.mjs` is a small stdio MCP server for Codex runtimes
that cannot expose native `image_gen` to a third-party route. Configure it as a
managed MCP entry using the generated loopback URL and API key:

```json
{
  "mcpServers": {
    "codex-nexus-image": {
      "command": "node",
      "args": ["packages/image-mcp/server.mjs"],
      "env": {
        "CODEX_NEXUS_GATEWAY": "http://127.0.0.1:8787",
        "CODEX_NEXUS_API_KEY": "${secret:codex-nexus-local}"
      }
    }
  }
}
```

The server advertises `generate_image`, `edit_image`, `list_image_models` and
`get_image_job`. Generated base64 output is saved under the managed image
directory and returned as MCP image content for inline display. Account
eligibility and image capability are checked by the gateway; MCP does not
override an upstream provider's permissions.

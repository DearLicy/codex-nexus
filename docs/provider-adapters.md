# Provider adapters

Codex Nexus routes requests through a capability declaration rather than
assuming every provider behaves like the official Codex service. A route must
declare its wire protocol (`responses`, `chat`, `anthropic` or `custom`), model
catalog and each capability it actually supports: streaming, vision, compact,
image generation and image editing.

The loopback gateway accepts strict `namespace/model` IDs. Unknown namespaces
or models return `model_route_not_available`; they never silently fall back to
an OAuth subscription account. API keys can further restrict route namespaces
and model IDs.

The Rust `ProviderAdapter` type is the native contract. The generated gateway
manifest is its runtime projection. Provider credentials are represented by a
`credential_ref` and resolved by the platform secret store before a sidecar is
started. The manifest must be written with restrictive permissions and removed
when the service stops.

Use a mock HTTP server when adding an adapter test. Cover successful JSON,
Responses SSE, Chat Completions SSE, upstream 4xx (no retry), upstream 5xx
(cooldown and bounded retry), and an unknown namespace.

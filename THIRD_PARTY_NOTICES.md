# Third-party notices

Codex Nexus may distribute code from open-source packages in its Slint native shell, Rust core, and optional Node sidecars. Each dependency remains under its own license. Cargo and pnpm lockfiles are the source of truth for the exact versions resolved in a build.

## How notices are maintained

When adding or updating a runtime dependency:

1. review its license and transitive license obligations;
2. keep the package name and license information in the relevant manifest/lockfile;
3. add any required attribution or license text to this file; and
4. run the normal build and CI checks.

The project currently has no separately vendored third-party source files. Generated bundles may contain dependency code; their notices should be regenerated from the package manager metadata as release automation is introduced.

## Trademarks and service names

“Codex”, “OpenAI”, and other product names belong to their respective owners. Codex Nexus is an independent third-party project and is not endorsed by or affiliated with OpenAI unless explicitly stated in a release note.

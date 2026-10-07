# Security policy

## Supported versions

Codex Nexus is under active development. Security fixes are applied to the latest commit on the default branch and to the most recent published release, when releases are available. Older versions may not receive fixes.

## Reporting a vulnerability

Please do not open a public issue for a suspected vulnerability. Report it privately through the repository’s GitHub **Security → Report a vulnerability** flow, or contact the maintainers through the private contact channel configured for `DearLicy/codex-nexus`.

Include, when safe to share:

- a short description of the impact;
- the affected version or commit;
- reproducible steps or a minimal proof of concept;
- logs with secrets and personal data removed; and
- a suggested fix, if you have one.

The maintainers will acknowledge a report as soon as practical, investigate it, and coordinate a fix and disclosure timeline with the reporter. Please allow reasonable time for a fix before public disclosure.

## Handling sensitive data

Never include API tokens, cookies, private keys, personal access tokens, or unredacted local database files in reports. If a token may have been exposed, revoke it first and mention only the token type and approximate exposure window.

## Scope

This policy covers the Codex Nexus source tree, release artifacts, and the official repository. Vulnerabilities in third-party services or dependencies should be reported to their respective maintainers as well; please include the dependency name and affected version in the Codex Nexus report so we can track an upgrade or mitigation.

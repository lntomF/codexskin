# Security Policy

> 中文版：[查看安全政策](../../SECURITY.md)

## Scope

CodeSkin interacts with local desktop processes and local Chrome DevTools
Protocol endpoints to apply and restore runtime visual customizations. Security
reports are welcome for unintended network exposure, connections to non-local
endpoints, local privilege escalation, arbitrary code execution, unsafe file
access, injection into an unintended process, sensitive-data leakage, unsafe
imported-file handling, or failure to restore the official appearance.

## Supported versions

Security fixes are prioritized for the latest released version of CodeSkin.

## Reporting a vulnerability

Please do **not** open a public GitHub Issue for a suspected security
vulnerability. Use the repository's private **Report a vulnerability** channel
and include a clear description, affected versions, reproducible steps, impact,
and sanitized evidence if needed.

Do not include API keys, account credentials, private repository data, personal
files, or information that is unnecessary to reproduce the issue.

## What to expect

The maintainer will review the report, confirm whether it is in scope, and work
toward a fix or mitigation. Please allow reasonable time for a response and
coordinated disclosure before discussing the issue publicly.

## Security boundaries

CodeSkin is designed to use local runtime communication only, avoid modifying
official application installation files, avoid reading or changing
model-provider API configuration, and provide a restoration path for
CodeSkin-injected visual layers. Please report any behavior that violates these
boundaries.

# Security Policy

## Supported versions

Only the latest release of Yog-Mod-Loader gets security fixes.

## Reporting a vulnerability

Please **do not** open a public issue for a security problem.

Report it privately through GitHub:
[Security → Report a vulnerability](https://github.com/F000NKKK/Yog-Mod-Loader/security/advisories/new).

Include what is affected, how to reproduce it and what an attacker could do
with it. You will get an answer within a week; once a fix is released the
advisory is published and you are credited, unless you prefer otherwise.

## Scope

Yog mods are native code that runs inside Minecraft with the player's rights,
so installing a mod means trusting its author. Reports about the loader, the
build tools or the IDE letting code or data cross a boundary it shouldn't —
a mod escaping what it was given, a crafted file running code when opened,
a path outside a project being written — are exactly what this policy is for.

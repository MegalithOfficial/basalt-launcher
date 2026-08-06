# Security Policy

Basalt handles Microsoft and Minecraft credentials, downloads third-party content,
writes to game directories, starts Java processes, and installs signed application
updates. Please report vulnerabilities privately so users can be protected before
technical details are public.

## Supported versions

Security fixes are made for the latest published Basalt release and the current
`main` branch. Older releases and development prereleases may not receive separate
patches. Confirm that the problem still exists on a current build when doing so does
not put data or credentials at risk.

## Report a vulnerability

Email `basaltlauncher@gmail.com` with **Basalt security** in the subject. Do not open a
public issue or include vulnerability details in a GitHub discussion.

Include:

- the affected Basalt version or commit and operating system;
- the impact and who can trigger it;
- the smallest reliable reproduction or proof of concept;
- the affected files, features, or external services if known;
- any temporary mitigation you have tested.

Use test accounts and disposable data. Do not send real tokens, API keys, signing
keys, or another person's private information. If a secret is necessary to explain
the report, redact it and describe its type instead.

Reports are particularly useful for issues involving:

- exposure or misuse of Microsoft, Minecraft, CurseForge, or updater credentials;
- arbitrary file access, path traversal, unsafe archive extraction, or unintended
  overwrites outside an instance;
- command or launch-argument injection and unintended code execution;
- bypasses of download hash checks or application update signatures;
- a malicious modpack, world, resource, or remote response escaping its intended
  trust boundary;
- sensitive information written to logs or uploaded through log sharing.

Ordinary crashes, broken upstream APIs, Minecraft server vulnerabilities, malicious
mods doing what the user explicitly installed them to do, and dependency advisories
without a demonstrated impact on Basalt can use the normal bug tracker.

## What to expect

The maintainer will aim to acknowledge a complete report within seven days, confirm
the affected versions, and coordinate a fix and disclosure timeline with the
reporter. Complex reports may take longer to investigate. Basalt does not currently
operate a paid bug bounty program.

Please keep the report and exploit details private until a fix is available or a
disclosure date has been agreed. Credit will be included in the advisory or release
notes if requested.

## Good-faith research

Research is welcome when it is limited to accounts and systems you control, avoids
service disruption and privacy violations, and stops once enough evidence has been
collected to demonstrate the issue. Do not access other users' data, persist on a
system, or use a vulnerability against third-party services.

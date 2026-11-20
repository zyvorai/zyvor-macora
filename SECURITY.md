# Security

## Model

MACORA is local-first. Inspection uses fixed macOS executable paths and argument arrays, never a shell. Settings are validated, previewed and authorized through the operating system. The app does not run a persistent root service, store administrator passwords, install a packet filter or send telemetry.

Active checks (`diagnose`, `ping`, `trace`, `probe`, `resolve`) run only when you invoke them. `diagnose` sends a few ICMP packets to your gateway, one TCP connection to 1.1.1.1:443 and one DNS lookup.

Full captures, scoped DNS, routes, service names and SSIDs can identify a network. `--redact` masks modeled IP/MAC strings only. Snapshots are private (mode 0600) and never overwritten. They are not a rollback mechanism.

## Supported versions

The latest release receives fixes.

## Reporting a vulnerability

Please report privately through [GitHub's private vulnerability reporting](https://github.com/zyvorai/zyvor-macora/security/advisories/new). Do not open a public issue or attach unredacted captures or credentials. We aim to acknowledge within 5 working days.

# Security model

MACORA is local-first. Inspection uses fixed macOS executable paths and argument arrays. Settings are validated, previewed and authorized through the operating system. The app does not run a persistent root service, store administrator passwords, install a packet filter or send telemetry.

Full captures, scoped DNS, routes, service names and SSIDs can identify a network. The sharing redaction option masks modeled IP/MAC strings only. Snapshots are not a complete rollback mechanism. An administrator or configuration profile can independently change settings.

Report vulnerabilities privately to the maintainers before opening a public proof of exploit. Do not attach unredacted captures or credentials to public issues.

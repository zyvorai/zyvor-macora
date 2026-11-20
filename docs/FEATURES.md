# macOS network management coverage

| Linux-style task | MACORA implementation | Boundary |
| --- | --- | --- |
| Device inventory / `ip addr` | Rust interface groups, all addresses, flags, media, MTU, MAC | Native macOS tools, not netlink |
| Wi-Fi scan and personal-network join | Native CoreWLAN scan/join sheet | Requires hardware and possibly Location permission; 802.1X opens System Settings |
| Wi-Fi power | Previewed `networksetup -setairportpower` | Uses hardware mapping, not an assumed en0 |
| Disconnect Wi-Fi | Explicit CoreWLAN disassociate confirmation | macOS auto-join can reconnect |
| Remove saved network preference | Previewed preferred-network removal | Does not erase all Keychain identities/certificates |
| DHCP / static IPv4 | Per-service DHCP or address/netmask/router plan | Can interrupt networking |
| IPv6 | Automatic, link-local only, static address/prefix/router | Scoped DNS remains visible |
| DNS servers / automatic reset | Per-service server list / `Empty` reset | Profile/VPN policies may override it |
| Search domains | Per-service domain list / clear | No global resolver flattening |
| HTTP / HTTPS / SOCKS proxy | Per-service proxy host/port | Authenticated proxy credentials go through System Settings |
| PAC proxy and exclusions | PAC URL, bypass domains, per-type off | Existing managed policy may override configuration |
| Network profiles | macOS locations: create populated, switch, delete | Not NetworkManager connection-profile format |
| Interface MTU | Validated preview, device limits still enforced by OS | 576–9000 tool range, not a promise every value works |
| Service enable / disable | Native per-service state change | Separate from interface admin up/down |
| Service priority | Exact permutation of all services | No partial order silently accepted |
| Interface admin up/down | Rust CLI preview; explicit `--apply` | CLI requires caller-provided privileges |
| Routes | Both IP families, default path/reachability | Route editing and policy-routing management not implemented |
| Neighbor discovery | ARP + IPv6 neighbor inspectors | Read-only |
| Connections | TCP/UDP socket inspector | Current user visibility; no per-app traffic accounting |
| Bandwidth history | Live adapter rates and 90 samples in memory | Counter resets clamp to zero; no persistent capture |
| Troubleshooting | Address/state checks, numeric TCP probe, subnet calculator | TCP probe is an explicit outbound operation |
| Directory DNS cache | Authorized directory-cache flush | Does not restart mDNSResponder |
| VPN and enterprise 802.1X | Open native settings | No custom tunnel, certificate store or VPN provider |
| Firewall / Internet Sharing | Open appropriate system networking settings | No packet filter or NAT daemon shipped |
| Bond / VLAN / bridge creation | Native settings handoff | Member inventory is implemented; virtual-device creation is not |
| Interactive exploration | REPL and six-page TUI with filtering, history/palette, baselines and previews | No shell evaluation; offline sessions cannot apply |
| Native workspace navigation | Cmd-K palette, pinned adapters, saved appearance/density/refresh preferences | UserDefaults stores display choices only |
| Configuration comparison | Native JSON baseline import and typed diff; CLI raw-capture diff | Not a restorable network profile |
| Change observation | In-memory timeline, capped at 200 events | Not a durable audit log |
| Menu bar monitoring | Shared background polling task with state and rates | No privileged daemon |
| Persistence and automation | macOS owns persisted settings | No background service, auto rollback or transactional profile engine |

MACORA targets macOS 14 and later. It is a network inspection and configuration workspace, not a replacement operating-system networking stack. All mutating commands surface system failures rather than reporting synthetic success.

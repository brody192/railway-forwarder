# Railway Forwarder

A macOS menu bar app for forwarding individual Railway service ports over `ssh.railway.com`. Settings use Rust, GPUI and GPUI Kit. The menu uses AppKit with Liquid Glass switches. Requires macOS 26 or later, Apple's Command Line Tools with the macOS 26 SDK or later, and Rust 1.99.1. SSH runs in-process through `russh` and does not require Railway CLI, OpenSSH executables or a shell.

## Build and open

```sh
bash scripts/build.sh
open "dist/Railway Forwarder.app"
```

The app stays in the menu bar without a Dock icon. Its native menu has a Railway Forwarder header, project sections with workspace labels, the selected ports, a Disconnect action between separators, then Settings and Quit. Every port stays at the top level with its own switch. These actions have no icons, shortcuts, or ellipses. Launching or reopening the app keeps settings closed, including on first launch. Choose Settings or Sign in with Railway from the menu to open the window. The explicit `--settings` launch flag also opens it.

Enable **Settings → General → Start at login** to open the app automatically when you sign in to macOS. The switch uses Apple's native ServiceManagement API and reflects macOS's current login-item state, including changes made in System Settings. While General is open, it checks for changes once a second even when the window is unfocused, updating the UI only when the state changes. If approval is required, use **Open Login Items** to approve it there. Registration belongs to this Mac user and survives signing out of Railway. Automatic launch keeps Settings closed. Keep the app in its permanent location before enabling this option.

**Settings → General → Restore active tunnels on launch** reconnects the last enabled tunnels when the app starts with a saved Railway session. This is off by default and independent of Start at login. Tunnel switches are always remembered in `settings.json`, even while restoration is disabled. Quitting and signing out preserve that memory; switching a tunnel off, unselecting its port, or choosing Disconnect removes it. Failed connection attempts remain remembered for the next launch. Restoration tries each remembered, still-selected port once, using its saved local port; it does not scan for new ports or continuously retry failures. Changing the setting only affects future launches.

The app is entirely Rust. GPUI owns the application event loop and settings window. Rust calls AppKit, AuthenticationServices and ServiceManagement directly through objc2 for the native status menu, Liquid Glass switches, OAuth browser and login-item registration. OAuth, API transport, credential files and SSH management also run in Rust. Credentials stay out of the settings model. The build script packages and signs one executable.

## Reusable SDKs

The root is a virtual Cargo workspace. The desktop app lives in `apps/railway-forwarder`, and the reusable SDKs live in `crates/`. The app consumes their public APIs through focused application adapters. See the [architecture guide](docs/architecture.md) for the module map and dependency boundaries.

| Crate | Owns | Documentation |
| --- | --- | --- |
| `ssh-forward` | SSH identities, host-key pins, SFTP listener discovery and owned TCP tunnels | [SDK usage and example](crates/ssh-forward/README.md) |
| `macos-status-menu` | Native status items, menu descriptions and events, Liquid Glass switches, copy feedback and native previews | [SDK usage and example](crates/macos-status-menu/README.md) |

Neither library depends on Railway or GPUI. Each can be used through a path dependency in another Rust application. The app retains OAuth, Railway API calls, project selection, display names and settings. The SDKs own their transport or native controls without taking over the application's event loop. `scripts/test.sh` checks all workspace crates and their documentation examples.

Settings always use dark mode. GraphQL operations use `graphql_client` 0.14.0, matching Railway CLI, with generated types checked against the CLI's schema. Rust encodes queries, validates responses and supplies authenticated HTTP transport through reqwest.

Workspace, project, environment and service display names omit emoji and trim whitespace at their edges. Spaces inside names remain intact. Their original names and IDs remain unchanged internally.

Custom names are global mappings keyed by resource type and Railway UUID. Changes update the services tree, native menu, menu preview and cached editor fields as you type. The same service UUID uses the same custom name in every environment. Resetting an override restores the latest catalog name.

The Services tab searches workspace, project, environment and service names or UUIDs, including service-instance UUIDs. Matching is case insensitive and accepts partial UUIDs or a combination of names and IDs. Full service and environment UUIDs resolve their owning project directly. Text, partial UUIDs and uncached service-instance UUIDs load missing metadata with at most four concurrent requests. Editing or clearing the search cancels the previous requests. A 90-second deadline stops incomplete searches, with an info button in the search field explaining the error and offering retry. Search never scans service ports. The selected-ports button filters the tree to saved ports and their workspace, project, environment and service ancestors. Search narrows those results. Clear resets both filters and restores the full tree without changing any saved port selections.

Typing a search reveals matching paths already in the catalog. Background results update the list without opening dropdowns. Manual open and close choices survive search edits and Clear. The selected-ports button explicitly reveals its saved paths. Clear restores the unfiltered tree while preserving manual dropdown choices.

Once you interact with a filtered dropdown, the visible result layout stays in place while other projects load. Opening a branch can still load that branch's children. Edit the search or use Refresh to show newly discovered matches. Loading indicators stay inside the search field and disclosure rows so they do not shift click targets.

The Menu tab previews the same layout used by the native menu. Drag project headings to reorder sections and drag ports to reorder them within a project. Select a project heading or port to edit its workspace, project, environment or service display names below the preview. Port labels are generated from the service name and port numbers. The app header, Disconnect, Settings and Quit remain fixed. Overrides and ordering are saved locally; resetting the menu preserves port selections and local port numbers. **Open menu** shows the actual native menu.

## Connect

1. Click **Sign in with Railway** and choose which projects to share.
2. Expand a workspace, a project, an environment, and a service in settings.
3. Expanding a service registers a dedicated SSH key, connects to that service, and reads its listening TCP sockets.
4. Check individual ports to add them to the menu bar dropdown. Settings use the workspace → project → environment → service hierarchy. The native macOS menu shows each selected port as one top-level row with a native switch, labeled with its service name and port mapping. Project headings show the workspace on the right. Each port shows its project and environment as secondary text.
5. Set a different local port if needed. Commit the field with Return or by leaving it.
6. Use the switch beside a port in the menu to toggle its tunnel. Each port occupies one line; a different local port is shown as `local → remote`.
7. Click a port’s service label or secondary text to copy its local address, such as `127.0.0.1:8080`. The address uses the configured local port, even when the tunnel is off.

Nothing is forwarded until its switch is enabled or startup restoration reconnects a remembered tunnel. Unchecking a port, signing out, or quitting stops its tunnel. Closing settings keeps tunnels running. Startup restoration is opt-in; failed connections can be retried manually or on the next launch with restoration enabled.

Selected ports, local port mappings, display-name overrides and menu ordering are saved together in `~/Library/Application Support/Railway Forwarder/settings.json`. Signing out preserves these preferences. Saved ports reappear after signing in, with forwarding switched off.

## OAuth

Forwarder uses automatic dynamic client registration and Authorization Code with PKCE through `ASWebAuthenticationSession`. No client secret or manually created OAuth app is required. Like Railway CLI, it saves its session in local files protected by filesystem permissions. Access and rotating refresh tokens are in `~/.railway/forwarder/tokens.json`, and OAuth registration credentials are in `~/.railway/forwarder/client.json`. The directory is mode `0700`, files are created with mode `0600`, and writes replace the entire file atomically. Refreshes within the app share one in-flight request. Forwarder does not read or modify Railway CLI's own `~/.railway/config.json` or use macOS Keychain.

Requested scopes are `openid email profile offline_access project:viewer ssh_keys`. Project metadata access is read only. The separate `ssh_keys` permission lets the app register its SSH credential. SSH access follows the Railway account's permissions; the menu allowlist is a local display and forwarding preference, not a server-side restriction on that key.

Under Railway's current consent logic, a dynamically registered app requesting `project:viewer` does not trigger the unverified write-access warning. The normal consent screen still appears.

If dynamic registration is unavailable, **Account → Advanced** accepts an optional native OAuth client ID. Its registered redirect URI must be exactly `com.brody.railway-forwarder://oauth/callback`. Leave this blank for automatic registration.

## SSH and discovery

Rust generates an Ed25519 key in `~/Library/Application Support/Railway Forwarder/` with mode `0600`. Public keys are registered through Railway's API. The Rust client stores Railway's host key in the existing app `known_hosts` file using first-use trust and rejects changed keys. Existing private keys and host-key pins are retained. Signing out removes local tokens while preserving saved preferences. It does not revoke the registered SSH key or OAuth grant. These can be revoked from Railway account settings.

Discovery reads `/proc/net/tcp` and `/proc/net/tcp6` over SFTP using `russh-sftp` and extracts only TCP listeners. It does not execute a command, invoke a shell, probe a port range or install tools in the container. If SFTP or the kernel socket tables are unavailable, add a loopback port manually. UDP and Unix sockets are not forwarded.

Forwarding uses the current service instance ID as the SSH username, so a public domain is unnecessary. Local listeners bind only to `127.0.0.1`. The app binds the local listener and authenticates the SSH connection before marking a tunnel connected. Each local TCP connection uses its own SSH `direct-tcpip` channel. Disconnecting cancels all forwarding tasks and closes their sockets. Connected status confirms the listener and SSH session; it does not establish remote application health. If two services use the same port, assign distinct local ports.

## Checks and preview

```sh
bash scripts/test.sh
bash scripts/test-rust.sh
open -n "dist/Railway Forwarder.app" --args --demo --settings
```

The Rust checks cover credential persistence and permissions, OAuth callback validation and PKCE, existing sessions, IPv4 and IPv6 socket parsing, native SSH/SFTP transport, concurrent forwarding, half-closed connections, cancellation, host-key rejection, port validation, global display names, menu ordering, settings commands and typed GraphQL operations. No app build is required before running tests. The preview uses sample projects, never reads credentials, and disables SSH forwarding.

For a read-only check of saved authentication and selected project access, run `.build/rust/release/RailwayForwarder --check-catalog`. Run `.build/rust/release/RailwayForwarder --check-ssh` to verify native SSH authentication, SFTP discovery and direct TCP channels for the saved ports. It does not send application data or change the saved port selections. To inspect the native glass controls in an isolated demo, launch the app with `--demo --settings --native-controls-preview`.

The build is locally ad hoc signed. Distribution to other Macs needs an Apple Developer signing identity and notarization.

References: [Railway OAuth](https://docs.railway.com/integrations/oauth), [OAuth scopes](https://docs.railway.com/integrations/oauth/scopes-and-user-consent), [Railway SSH](https://docs.railway.com/cli/ssh).

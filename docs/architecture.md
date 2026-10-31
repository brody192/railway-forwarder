# Workspace structure

The repository root is a virtual Cargo workspace. The desktop application and reusable SDKs are separate packages with explicit path dependencies. Shared dependency versions and build profiles live in the root manifest. Each package enables the features it needs.

```text
apps/
  railway-forwarder/
    Cargo.toml
    graphql/                 Railway schema, operations and source attribution
    resources/               macOS bundle metadata
    src/
      main.rs                Entry point calling app::run()
      app/
        lifecycle.rs         GPUI startup, windows and application event loop
        events.rs            Delivery between background tasks and the UI
        backend/
          mod.rs             State ownership and task lifetime coordination
          catalog.rs         Catalog loading and service scans
          search.rs          Direct UUID lookup and cancellable bounded search
          commands.rs        Settings command validation and dispatch
          session.rs         Sign-in and sign-out transitions
          tunnels.rs         Forwarding lifecycle and remembered startup intent
          diagnostics.rs     Headless catalog, search and SSH checks
          demo.rs            In-memory preview data
          tests.rs           State and command regression tests
      domain/
        resources.rs         Workspace > project > environment > service
        ports.rs             Allowed ports, identities and tunnel states
        preferences.rs       Persisted user preferences
        snapshot.rs          Data exposed to the UI
        startup.rs           Login-item registration and approval states
        names.rs             Global display-name resolution
        menu.rs              Pure menu layout and ordering
        catalog.rs           Pure search and selection filtering
      integrations/
        railway/
          auth.rs            OAuth, session refresh and Railway HTTP errors
          client.rs          Railway API calls and catalog mapping
          graphql.rs         Typed GraphQL operations and response validation
        ssh.rs               Railway targets and app tunnel reservations
      platform/
        macos/
          menu.rs            App model and SDK event translation
          oauth.rs           AuthenticationServices browser sessions
          login_item.rs      ServiceManagement main-app login registration
          preview.rs         Native control review window
      ui/
        controls.rs          Shared button cursors and disabled-state styling
        settings/
          mod.rs             Settings state, subscriptions and refresh
          view.rs            Window frame, sidebar and active tab
          services.rs        Resource tree, services and port rows
          filters.rs         Search, selected ports and disclosure state
          disclosures.rs     Manual tree choices and automatic reveal ownership
          results.rs         Stable search results and explicitly requested child updates
          menu.rs            Editable menu preview and drag ordering
          account.rs         Account controls
          general.rs         Login items, approval status and tunnel restoration
          names.rs           Reactive display-name inputs
          inputs.rs          Shared input creation and focus behavior
      persistence.rs         Protected local JSON storage and filesystem paths
crates/
  ssh-forward/
    src/
      lib.rs                 Public API exports and usage example
      config.rs              Connection options and remote endpoints
      client.rs              SSH authentication, transport and probes
      credentials.rs         Identity generation and host-key pins
      discovery.rs           SFTP discovery and Linux socket-table parsing
      tunnel.rs              Forwarding ownership, streams and cancellation
      files.rs               Protected credential-file operations
      tests.rs               Local SSH/SFTP fixture and transport checks
    examples/                Standalone SDK consumer
  macos-status-menu/
    src/
      lib.rs                 Public API exports and AppKit helpers
      model.rs               Framework-independent menu descriptions
      menu.rs                Status-item ownership and native menu rendering
      controls/
        mod.rs               Row composition
        glass_switch.rs      Standalone native switch
        copy_label.rs        Copy actions, cursor and tooltip lifecycle
    examples/                Standalone AppKit consumer
docs/                        Developer architecture documentation
scripts/                     Workspace checks and app packaging
```

## Boundaries

`main.rs` only declares top-level modules and calls the app. The app layer owns state and background work. Domain modules transform ordinary Rust values without network, storage or UI access. UI modules submit commands through `app::events` and render snapshots. Platform modules adapt native macOS APIs. Integration modules translate Railway concepts into provider requests or generic SSH SDK calls.

Both SDKs are independent of the app and each other. `ssh-forward` requires a caller-owned Tokio runtime. `macos-status-menu` requires the AppKit main thread and a caller-owned application event loop. Neither imports GPUI or Railway-specific code. Public APIs are re-exported from each SDK's `lib.rs`; implementation modules remain private.

Keep new work beside the feature it changes. Add settings rendering to its tab module, Railway operations to its integration, and pure naming/filtering rules to the domain. Only add a workspace crate when it has an independent purpose and API. Avoid catch-all `utils`, `helpers` or `common` modules. Use private modules and the narrowest useful visibility. Small unit tests stay beside their implementation; larger fixtures and regression suites use a local `tests.rs` module.

## Build and checks

Run commands from the workspace root. The default Cargo member is the desktop app, so `cargo run` still selects it. The scripts keep build output under ignored `.build/` and package the app under ignored `dist/`.

```sh
bash scripts/build.sh
bash scripts/test.sh
```

The build script selects the app package explicitly and stamps the executable with the selected macOS SDK for Liquid Glass. The test script checks every workspace member, including SDK documentation examples. For a single SDK, use `cargo test -p ssh-forward` or `cargo check -p macos-status-menu --all-targets`.

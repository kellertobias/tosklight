# Tauri and the TypeScript Process Boundary

Read [Rust language basics](01-language-basics.md) before this guide. The CodeSafari
**Tauri Desktop Apps** component page contains the complete application map.

## What Tauri is here

Tauri is the native host around the React UI. It is not ToskLight's application or domain layer.
`apps/light-desktop/src-tauri/` owns native windows, menus, the child-server process, and a small set
of native commands. The sibling `apps/light-hardware-controls/src-tauri/` owns native UDP OSC.

The frontend depends on `DesktopBridge` in
`apps/light-desktop/src/platform/desktop/types.ts`, not directly on Tauri globals. The Tauri adapter
implements that interface, while the browser adapter makes the UI testable without a native host.

## Command attributes and registration

Before the example:

- `#[tauri::command]` is a procedural macro attribute. It generates argument decoding and return
  serialization for the following function.
- `tauri::generate_handler![...]` is a macro that registers the explicit command allowlist.
- Neither construct automatically exposes every Rust function to the frontend.

```rust
#[tauri::command]
fn list_console_displays(app: tauri::AppHandle) -> Result<Vec<ConsoleDisplay>, String> {
    app.available_monitors()
        .map_err(|error| error.to_string())
        .map(|monitors| {
            monitors
                .into_iter()
                .map(|monitor| ConsoleDisplay {
                    id: monitor_id(&monitor),
                    name: monitor.name().cloned().unwrap_or_else(|| "Display".into()),
                })
                .collect()
        })
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![list_console_displays])
        .run(tauri::generate_context!())
        .expect("failed to run ToskLight");
}
```

Tauri injects `AppHandle`; the TypeScript caller does not provide it.
`Result<Vec<ConsoleDisplay>, String>` becomes a resolved promise containing an array or a rejected
invocation containing the error.

The TypeScript adapter turns the wire name into an application-facing method:

```ts
listDisplays(): Promise<DesktopDisplay[]>;
```

React components call `DesktopBridge.listDisplays()`, not `invoke("list_console_displays")`.

## Conditional compilation and async startup

`#[cfg(target_os = "macos")]` includes the following code only in a macOS build. It is a compiler
attribute, not a runtime OS check.

`#[tokio::main]` on the server entry point generates the synchronous startup wrapper and Tokio
runtime needed to execute an async `main`. Like other procedural macro attributes, it transforms
code at compile time.

## Process and window ownership

The main Tauri app launches and supervises the sibling `light-headless`. Closing the application must
not orphan that process. Secondary windows borrow the primary session; closing one must not destroy
the authoritative desk session.

Async tasks need:

1. an owner,
2. a cancellation path,
3. visible errors for operator-triggered work, and
4. cleanup when the owning window or application exits.

These lifecycle rules matter more than whether a function happens to be written with `async`.

## Adding a native capability

Add the narrow capability through every explicit boundary:

1. Add a method to `DesktopBridge`.
2. Implement the Tauri adapter.
3. Provide a working browser/test adapter.
4. Add the Rust command and its `#[tauri::command]` attribute.
5. Register it in `generate_handler!`.
6. Add only the Tauri permissions the command needs.
7. Test the browser contract, then verify real desktop behavior with `npm run open`.

Keep domain rules out of the command. A native command decodes, delegates, and maps its typed
result at the process edge.

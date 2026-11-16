use std::path::Path;

use tauri::plugin::{Builder, PluginHandle, TauriPlugin};
use tauri::{Manager, Wry};

struct Bridge(PluginHandle<Wry>);

pub fn plugin() -> TauriPlugin<Wry> {
    Builder::<Wry>::new("liber-file-bridge")
        .setup(|app, api| {
            let handle = api.register_android_plugin("rs.liber.app", "OpenFilePlugin")?;
            app.manage(Bridge(handle));
            Ok(())
        })
        .build()
}

fn call(
    app: &tauri::AppHandle,
    command: &str,
    path: &Path,
    mime: &str,
    name: Option<&str>,
) -> Result<(), String> {
    let bridge = app.state::<Bridge>();
    bridge
        .0
        .run_mobile_plugin::<serde_json::Value>(
            command,
            serde_json::json!({
                "path": path.to_string_lossy(),
                "mime": mime,
                "name": name,
            }),
        )
        .map(|_| ())
        .map_err(|e| e.to_string())
}

pub fn open(app: &tauri::AppHandle, path: &Path, mime: &str) -> Result<(), String> {
    call(app, "open", path, mime, None)
}

pub fn share(app: &tauri::AppHandle, path: &Path, mime: &str, name: &str) -> Result<(), String> {
    call(app, "share", path, mime, Some(name))
}

pub fn save(app: &tauri::AppHandle, path: &Path, mime: &str, name: &str) -> Result<(), String> {
    call(app, "save", path, mime, Some(name))
}

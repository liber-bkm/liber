fn main() {
    let attributes = tauri_build::Attributes::new().plugin(
        "liber-file-bridge",
        tauri_build::InlinedPlugin::new()
            .commands(&[
                "open",
                "share",
                "save",
                "take_shared",
                "register_listener",
                "remove_listener",
            ])
            .default_permission(tauri_build::DefaultPermissionRule::AllowAllCommands),
    );
    tauri_build::try_build(attributes).expect("failed to run tauri-build");
}

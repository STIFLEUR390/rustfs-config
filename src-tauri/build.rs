fn main() {
    // tauri-build needs the `tauri` crate's build script (DEP_TAURI_DEV), so it only
    // runs with the GUI feature. Headless tests skip it.
    #[cfg(feature = "gui")]
    tauri_build::build();
}

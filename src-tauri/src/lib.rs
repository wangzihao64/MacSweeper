mod icons;
mod leftovers;
mod scanner;
mod uninstaller;
mod util;

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            scanner::scan_apps,
            scanner::app_sizes,
            icons::app_icon,
            leftovers::find_leftovers,
            uninstaller::uninstall,
        ])
        .run(tauri::generate_context!())
        .expect("error while running MacSweeper");
}

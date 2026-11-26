mod icons;
mod leftovers;
mod scanner;
mod skills;
mod uninstaller;
mod updates;
mod util;

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            scanner::scan_apps,
            scanner::app_sizes,
            icons::app_icon,
            leftovers::find_leftovers,
            uninstaller::uninstall,
            uninstaller::uninstall_many,
            updates::check_updates,
            updates::perform_update,
            skills::scan_skills,
            skills::update_skill,
        ])
        .run(tauri::generate_context!())
        .expect("error while running MacSweeper");
}

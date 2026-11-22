//! Linux keeps its window visible: tray protocols vary across desktop shells.
//! No GTK event loop or tray dependency is needed for the egui frontend.
#[allow(dead_code)]
pub enum TrayAction {
    Show,
    PauseAll,
    ResumeAll,
    Quit,
}
pub struct OrbitTray;
impl OrbitTray {
    pub fn new(_: &eframe::egui::IconData) -> anyhow::Result<Self> {
        anyhow::bail!("Tray operation is unavailable on Linux")
    }
    pub fn set_status(&self, _: &str) {}
    pub fn drain_actions(&self) -> Vec<TrayAction> {
        Vec::new()
    }
}

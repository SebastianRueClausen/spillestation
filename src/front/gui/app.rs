
/// Egui App/Window.
pub trait App {
    /// Show the app as a window.
    fn show(&mut self, ctx: &egui::CtxRef, open: &mut bool);
    
    /// Show the app on any given ['egui::Ui']. This can be used to display the app inline in
    /// another GUI. This is most likely called by 'show' onto an empty window.
    fn update(&mut self, ui: &mut egui::Ui);
}

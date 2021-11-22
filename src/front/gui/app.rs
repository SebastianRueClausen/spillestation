
/// Egui App/Window.
///
/// Allows types to draw a egui window to the screen.
pub trait App {
    /// Show the window.
    ///
    /// Sets open paramter.
    fn show(&mut self, ctx: &egui::CtxRef, open: &mut bool);
    
    /// Update the window.
    ///
    /// Should be called by show.
    fn update(&mut self, ui: &mut egui::Ui);
}

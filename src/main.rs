mod audio;
mod dsp;
mod melody;
mod monitor;
mod piano;
mod range;
mod training;
mod ui;
fn main() -> eframe::Result {
    let playback_only = std::env::args().any(|a| a == "--playback-only");
    eframe::run_native(
        "SingCoach",
        eframe::NativeOptions {
            viewport: eframe::egui::ViewportBuilder::default()
                .with_inner_size([960.0, 760.0])
                .with_min_inner_size([720.0, 640.0]),
            ..Default::default()
        },
        Box::new(move |cc| Ok(Box::new(ui::Coach::new(cc, playback_only)))),
    )
}

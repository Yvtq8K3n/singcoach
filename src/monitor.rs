//! Presentation components adapted from the user's singcoach-codex reference.
//! Reference display copies never alter session timestamps or training evidence.
use crate::dsp::{self, Frame};
use eframe::egui::{self, Color32, Stroke};
use std::collections::VecDeque;
pub const BLUE: Color32 = Color32::LIGHT_BLUE;
pub const GREEN: Color32 = Color32::LIGHT_GREEN;
pub const PURPLE: Color32 = Color32::from_rgb(210, 170, 255);
fn valid_midi(hz: f32) -> Option<f32> {
    (hz.is_finite() && hz > 0.0).then(|| dsp::midi(hz))
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum View {
    Compact,
    Full,
    Spectrum,
}
pub fn tabs(ui: &mut egui::Ui, view: &mut View) {
    let bar = ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        for (value, label) in [
            (View::Compact, "Compact"),
            (View::Full, "Full scale"),
            (View::Spectrum, "Spectrogram"),
        ] {
            let response = ui.add_sized(
                [120.0, 36.0],
                egui::Button::new(egui::RichText::new(label).strong())
                    .selected(*view == value)
                    .corner_radius(egui::CornerRadius {
                        nw: 6,
                        ne: 6,
                        sw: 0,
                        se: 0,
                    }),
            );
            if response.clicked() {
                *view = value;
            }
            if response.has_focus() {
                let views = [View::Compact, View::Full, View::Spectrum];
                let index = views.iter().position(|v| v == view).unwrap_or(0);
                if ui.input(|i| i.key_pressed(egui::Key::ArrowRight)) {
                    *view = views[(index + 1) % 3];
                }
                if ui.input(|i| i.key_pressed(egui::Key::ArrowLeft)) {
                    *view = views[(index + 2) % 3];
                }
            }
            if *view == value {
                ui.painter().line_segment(
                    [response.rect.left_bottom(), response.rect.right_bottom()],
                    Stroke::new(3.0_f32, GREEN),
                );
            }
        }
    });
    ui.painter().line_segment(
        [
            egui::pos2(bar.response.rect.left(), bar.response.rect.bottom() + 1.0),
            egui::pos2(ui.max_rect().right(), bar.response.rect.bottom() + 1.0),
        ],
        Stroke::new(1.0_f32, Color32::from_gray(60)),
    );
}
pub fn play_button(ui: &mut egui::Ui, playing: bool) -> egui::Response {
    let hint = if playing {
        "Stop reference"
    } else {
        "Play reference"
    };
    let response = ui
        .add_sized([36.0, 36.0], egui::Button::new("").corner_radius(8))
        .on_hover_text(hint);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, hint));
    let center = response.rect.center();
    let ink = ui.visuals().text_color();
    if playing {
        ui.painter().rect_filled(
            egui::Rect::from_center_size(center, egui::vec2(12.0, 12.0)),
            1.0,
            ink,
        );
    } else {
        ui.painter().add(egui::Shape::convex_polygon(
            vec![
                center + egui::vec2(-4.0, -7.0),
                center + egui::vec2(-4.0, 7.0),
                center + egui::vec2(7.0, 0.0),
            ],
            ink,
            Stroke::NONE,
        ));
    }
    response
}
/// Pin the newest clip at the viewport center, with prior clips to its left.
pub fn pinned_pitch(clips: &[&[Frame]], end: f64) -> VecDeque<Frame> {
    let mut output = VecDeque::new();
    let count = clips.len();
    for (index, clip) in clips.iter().enumerate() {
        let duration = clip.last().map_or(1.0, |f| f.time + 0.05);
        let offset = end - 1.5 - duration * 0.5 - (count - 1 - index) as f64 * 1.1;
        for f in clip
            .iter()
            .filter(|f| f.time + offset >= end - 3.0 && f.time + offset <= end)
        {
            let mut copy = (*f).clone();
            copy.time += offset;
            copy.spectrum = None;
            output.push_back(copy);
        }
        output.push_back(Frame {
            time: offset + duration,
            hz: None,
            confidence: 0.0,
            rms: 0.0,
            clipped: false,
            spectrum: None,
        });
    }
    output
}
/// Fit every saved guide into the map, preserving clip order and breaks.
/// This overview scales reference time only; microphone evidence stays live.
pub fn overview_pitch(clips: &[&[Frame]], end: f64) -> VecDeque<Frame> {
    let mut output = VecDeque::new();
    let slot = 3.0 / clips.len().max(1) as f64;
    for (index, clip) in clips.iter().enumerate() {
        let start = clip.first().map_or(0.0, |f| f.time);
        let duration = clip.last().map_or(0.05, |f| f.time - start + 0.05);
        let offset = end - 3.0 + index as f64 * slot;
        for f in *clip {
            let mut copy = f.clone();
            copy.time = offset + (f.time - start) / duration * slot * 0.92;
            copy.spectrum = None;
            output.push_back(copy);
        }
        output.push_back(Frame {
            time: offset + slot * 0.96,
            hz: None,
            confidence: 0.0,
            rms: 0.0,
            clipped: false,
            spectrum: None,
        });
    }
    output
}
pub fn spectral_color(db: f32, gain: f32) -> Color32 {
    let level = if db <= -120.0 {
        0.0
    } else {
        ((db + gain + 120.0) / 110.0).clamp(0.0, 1.0)
    };
    let stops = [
        [17.0, 23.0, 32.0],
        [35.0, 50.0, 120.0],
        [135.0, 50.0, 160.0],
        [240.0, 110.0, 60.0],
        [255.0, 240.0, 150.0],
    ];
    let position = level * 4.0;
    let i = (position as usize).min(3);
    let f = position - i as f32;
    let rgb: [u8; 3] =
        std::array::from_fn(|c| (stops[i][c] * (1.0 - f) + stops[i + 1][c] * f) as u8);
    Color32::from_rgb(rgb[0], rgb[1], rgb[2])
}
#[derive(Clone, Copy)]
pub struct PitchTarget {
    pub note: i32,
    pub tolerance_cents: f32,
}
impl From<i32> for PitchTarget {
    fn from(note: i32) -> Self {
        Self {
            note,
            tolerance_cents: 15.0,
        }
    }
}
/// A semitone axis keeps equal musical intervals equally spaced.
/// An indicative target marker, never pitch evidence or synthesized audio.
pub struct PitchGuide {
    pub note: i32,
    pub start: f64,
    pub end: f64,
}
#[cfg(test)]
pub fn pitch_monitor(
    ui: &mut egui::Ui,
    trace: &VecDeque<Frame>,
    examples: &VecDeque<Frame>,
    target: Option<PitchTarget>,
    full_scale: bool,
    height: f32,
    end: f64,
) {
    pitch_monitor_guided(ui, trace, examples, (target, &[]), full_scale, height, end);
}
pub fn pitch_monitor_guided(
    ui: &mut egui::Ui,
    trace: &VecDeque<Frame>,
    examples: &VecDeque<Frame>,
    guidance: (Option<PitchTarget>, &[PitchGuide]),
    full_scale: bool,
    height: f32,
    end: f64,
) {
    let (target, indicators) = guidance;
    let trace_color = Color32::LIGHT_BLUE;
    let voice_note = trace
        .iter()
        .rev()
        .take_while(|frame| end - frame.time <= 3.0)
        .filter(|frame| frame.confidence >= 0.85 && !frame.clipped)
        .find_map(|frame| frame.hz.and_then(valid_midi));
    let show_target = target.is_some();
    let tolerance = target.map_or(0.15, |t| t.tolerance_cents / 100.0);
    let target = target
        .map(|t| t.note)
        .unwrap_or_else(|| voice_note.map_or(60, |note| note.round() as i32));
    let mut notes: Vec<i32> = (target - 2..=target + 2).collect();
    if let Some(note) = voice_note {
        let voice = note.round() as i32;
        notes.extend(voice - 2..=voice + 2);
    }
    notes.extend(examples.iter().filter_map(|frame| {
        frame
            .hz
            .filter(|_| frame.confidence >= 0.85)
            .and_then(valid_midi)
            .map(|note| note.round() as i32)
    }));
    notes.extend(indicators.iter().map(|guide| guide.note));
    notes.sort_unstable();
    notes.dedup();
    if !full_scale && notes.len() < 11 {
        // Fill the compact grid around the target instead of reserving blank rows.
        notes.extend(target - 5..=target + 5);
        notes.sort_unstable();
        notes.dedup();
    }
    if full_scale {
        notes = (36..=84).collect();
    }
    let mut lanes = Vec::new();
    let mut gaps = Vec::new();
    let mut slot = 0;
    for (index, &note) in notes.iter().enumerate() {
        if index > 0 && note - notes[index - 1] > 1 {
            gaps.push((slot, note - notes[index - 1] - 1));
            slot += 1;
        }
        lanes.push((note, slot));
        slot += 1;
    }
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), height),
        egui::Sense::hover(),
    );
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 6.0, Color32::from_rgb(17, 23, 32));
    let plot = egui::Rect::from_min_max(
        egui::pos2(rect.left() + 112.0, rect.top() + 12.0),
        egui::pos2(rect.right() - 12.0, rect.bottom() - 24.0),
    );
    if plot.width() <= 0.0 {
        return;
    }
    let step = plot.height() / slot as f32;
    let y = |note: f32| {
        let &(nearest, lane) = lanes
            .iter()
            .min_by(|(a, _), (b, _)| {
                ((*a as f32 - note).abs()).total_cmp(&((*b as f32 - note).abs()))
            })
            .expect("target lanes are always present");
        plot.bottom() - (lane as f32 + 0.5 + note - nearest as f32) * step
    };
    let visible = |note: f32| lanes.iter().any(|(n, _)| (note - *n as f32).abs() <= 0.5);
    for &(note, _) in &lanes {
        let center = y(note as f32);
        let lane = egui::Rect::from_min_max(
            egui::pos2(rect.left(), center - step / 2.0),
            egui::pos2(plot.right(), center + step / 2.0),
        );
        let sharp = matches!(note.rem_euclid(12), 1 | 3 | 6 | 8 | 10);
        let color = if show_target && note == target {
            Color32::from_rgb(30, 64, 48)
        } else if sharp {
            Color32::from_rgb(20, 27, 37)
        } else {
            Color32::from_rgb(28, 36, 47)
        };
        painter.rect_filled(lane, 0.0, color);
        let ink = if show_target && note == target {
            Color32::LIGHT_GREEN
        } else {
            Color32::from_gray(165)
        };
        let voice_lane = voice_note.map(|voice| voice.round() as i32);
        if !full_scale || step >= 14.0 {
            let label = if show_target && note == target {
                format!("Target · {}", dsp::name(note))
            } else {
                dsp::name(note)
            };
            painter.text(
                egui::pos2(rect.left() + 10.0, center),
                egui::Align2::LEFT_CENTER,
                label,
                egui::FontId::monospace(12.0),
                ink,
            );
        } else {
            // Keep close voice/target labels in different columns. Octave labels
            // yield to the target instead of overlapping at small chart heights.
            if (show_target && note == target)
                || (note.rem_euclid(12) == 0
                    && (!show_target || (center - y(target as f32)).abs() >= 14.0))
            {
                painter.text(
                    egui::pos2(rect.left() + 10.0, center),
                    egui::Align2::LEFT_CENTER,
                    if show_target && note == target {
                        format!("Target {}", dsp::name(note))
                    } else {
                        dsp::name(note)
                    },
                    egui::FontId::monospace(12.0),
                    ink,
                );
            }
            if voice_lane == Some(note) {
                painter.text(
                    egui::pos2(plot.left() - 8.0, center),
                    egui::Align2::RIGHT_CENTER,
                    dsp::name(note),
                    egui::FontId::monospace(12.0),
                    trace_color,
                );
            }
        }
        painter.line_segment(
            [
                egui::pos2(plot.left(), center),
                egui::pos2(plot.right(), center),
            ],
            Stroke::new(
                1.0_f32,
                if show_target && note == target {
                    ink
                } else {
                    Color32::from_gray(48)
                },
            ),
        );
    }
    if full_scale {
        if let Some(pointer) = response.hover_pos().filter(|point| plot.contains(*point)) {
            let note = (36.0 + (plot.bottom() - pointer.y) / step - 0.5)
                .round()
                .clamp(36.0, 84.0) as i32;
            response.on_hover_text(format!(
                "{} · {:.2} Hz",
                dsp::name(note),
                dsp::hz(note as f32)
            ));
        }
    }
    for &(gap_slot, omitted) in &gaps {
        let gap_y = plot.bottom() - (gap_slot as f32 + 0.5) * step;
        let distance = voice_note.map_or(0.0, |note| note - target as f32);
        let arrow = if distance < 0.0 { "↓" } else { "↑" };
        let interval = if (distance.abs() - 12.0).abs() < 0.5 {
            "1 oct".into()
        } else {
            format!("{:.1} st", distance.abs())
        };
        painter.text(
            egui::pos2(rect.left() + 10.0, gap_y),
            egui::Align2::LEFT_CENTER,
            format!("{arrow} {interval}  ⋯"),
            egui::FontId::proportional(12.0),
            Color32::GRAY,
        );
        let gap_rect = egui::Rect::from_min_max(
            egui::pos2(rect.left(), gap_y - step / 2.0),
            egui::pos2(plot.left(), gap_y + step / 2.0),
        );
        ui.interact(
            gap_rect,
            ui.id().with("pitch_axis_gap"),
            egui::Sense::hover(),
        )
        .on_hover_text(format!(
            "{omitted} notes omitted. Voice is {:.1} semitones {} target.",
            distance.abs(),
            if distance < 0.0 { "below" } else { "above" }
        ));
    }
    if show_target {
        let band = egui::Rect::from_min_max(
            egui::pos2(plot.left(), y(target as f32 + tolerance)),
            egui::pos2(plot.right(), y(target as f32 - tolerance)),
        );
        painter.rect_filled(
            band,
            0.0,
            Color32::from_rgba_unmultiplied(100, 230, 150, 35),
        );
    }
    // Faint guide lines are drawn directly, independently of pitch evidence.
    for guide in indicators {
        let left =
            (plot.right() - ((end - guide.start) / 3.0) as f32 * plot.width()).max(plot.left());
        let right =
            (plot.right() - ((end - guide.end) / 3.0) as f32 * plot.width()).min(plot.right());
        if right > left {
            painter.with_clip_rect(plot).line_segment(
                [
                    egui::pos2(left, y(guide.note as f32)),
                    egui::pos2(right, y(guide.note as f32)),
                ],
                Stroke::new(1.5_f32, PURPLE.gamma_multiply(0.65)),
            );
        }
    }
    for seconds in 0..=3 {
        let x = plot.right() - seconds as f32 / 3.0 * plot.width();
        painter.line_segment(
            [egui::pos2(x, plot.top()), egui::pos2(x, plot.bottom())],
            Stroke::new(1.0_f32, Color32::from_gray(45)),
        );
        painter.text(
            egui::pos2(x, rect.bottom() - 5.0),
            egui::Align2::CENTER_BOTTOM,
            if seconds == 0 {
                "now".into()
            } else {
                format!("−{seconds}s")
            },
            egui::FontId::monospace(11.0),
            Color32::GRAY,
        );
    }
    if let Some(note) = trace
        .back()
        .filter(|f| f.confidence >= 0.85 && !f.clipped)
        .and_then(|f| f.hz)
        .and_then(valid_midi)
    {
        let lane_y = y(note.round());
        if lane_y >= plot.top() && lane_y <= plot.bottom() {
            painter.text(
                egui::pos2(rect.left() + 54.0, lane_y),
                egui::Align2::RIGHT_CENTER,
                "•",
                egui::FontId::proportional(16.0),
                trace_color,
            );
        }
        painter.with_clip_rect(plot).circle_filled(
            egui::pos2(plot.right() - 4.0, y(note)),
            4.0,
            trace_color,
        );
    }
    let voice_painter = painter.with_clip_rect(plot);
    for (history, trace_color) in [(examples, PURPLE), (trace, Color32::LIGHT_BLUE)] {
        let mut previous: Option<(egui::Pos2, f64)> = None;
        for frame in history {
            let note = frame
                .hz
                .filter(|_| frame.confidence >= 0.85 && !frame.clipped)
                .and_then(valid_midi)
                .filter(|note| visible(*note));
            if let Some(note) = note {
                let point = egui::pos2(
                    plot.right() - ((end - frame.time) / 3.0) as f32 * plot.width(),
                    y(note),
                );
                if let Some((prev, time)) = previous {
                    if frame.time - time < 0.025 && (prev.y - point.y).abs() < step * 2.0 {
                        voice_painter
                            .line_segment([prev, point], Stroke::new(2.0_f32, trace_color));
                    }
                }
                voice_painter.circle_filled(point, 1.5, trace_color);
                previous = Some((point, frame.time));
            } else {
                previous = None;
            }
        }
    }
}

pub fn spectrum(
    ui: &mut egui::Ui,
    frames: &[&Frame],
    window: std::ops::Range<f64>,
    height: f32,
    bounds: (f32, f32),
    gain: f32,
    reference: bool,
) {
    let (low, high) = bounds;
    let seconds = window.end - window.start;
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), height),
        egui::Sense::hover(),
    );
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 6.0, spectral_color(-120.0, gain));
    let plot = egui::Rect::from_min_max(
        rect.min + egui::vec2(72.0, 12.0),
        rect.max - egui::vec2(12.0, 28.0),
    );
    if !plot.is_positive() {
        return;
    }
    let p = painter.with_clip_rect(plot);
    for f in frames {
        if let Some(bins) = &f.spectrum {
            for (i, db) in bins.iter().enumerate() {
                let freq = 65.0 * (4000.0_f32 / 65.0).powf(i as f32 / 63.0);
                if freq < low {
                    continue;
                }
                if freq > high {
                    break;
                }
                let y = plot.bottom() - (freq / low).ln() / (high / low).ln() * plot.height();
                let h =
                    (plot.height() / (high / low).ln() * (4000.0_f32 / 65.0).ln() / 63.0).max(1.0);
                let x = plot.left() + ((f.time - window.start) / seconds) as f32 * plot.width();
                // Spectra are sampled every 100 ms; do not bridge missing observations.
                let w = (0.1 / seconds) as f32 * plot.width();
                p.rect_filled(
                    egui::Rect::from_min_size(egui::pos2(x, y - h), egui::vec2(w, h)),
                    0.0,
                    spectral_color(*db, gain),
                );
            }
        }
    }
    for tick in 0..=8 {
        let fraction = tick as f32 / 8.0;
        let hz = low * (high / low).powf(fraction);
        let y = plot.bottom() - fraction * plot.height();
        painter.text(
            egui::pos2(rect.left() + 8.0, y),
            egui::Align2::LEFT_CENTER,
            format!("{hz:.0} Hz"),
            egui::FontId::monospace(11.0),
            Color32::LIGHT_GRAY,
        );
    }
    for tick in 0..=10 {
        let fraction = tick as f32 / 10.0;
        let x = plot.right() - fraction * plot.width();
        painter.line_segment(
            [egui::pos2(x, plot.top()), egui::pos2(x, plot.bottom())],
            Stroke::new(1.0_f32, Color32::from_white_alpha(20)),
        );
        if tick % 2 == 0 {
            painter.text(
                egui::pos2(x, rect.bottom() - 5.0),
                egui::Align2::CENTER_BOTTOM,
                if tick == 0 {
                    if reference {
                        "end".into()
                    } else {
                        "now".into()
                    }
                } else {
                    format!("−{:.2}s", fraction as f64 * seconds)
                },
                egui::FontId::monospace(11.0),
                Color32::GRAY,
            );
        }
    }
    if let Some(pointer) = response.hover_pos().filter(|p| plot.contains(*p)) {
        let time = window.start + ((pointer.x - plot.left()) / plot.width()) as f64 * seconds;
        let hz = low * (high / low).powf((plot.bottom() - pointer.y) / plot.height());
        let bin = ((hz / 65.0).ln() / (4000.0_f32 / 65.0).ln() * 63.0).round() as usize;
        if let Some(bins) = frames
            .iter()
            .filter(|f| f.time <= time && time - f.time <= 0.1)
            .min_by(|a, b| (a.time - time).abs().total_cmp(&(b.time - time).abs()))
            .and_then(|f| f.spectrum.as_ref())
        {
            response.on_hover_text(format!(
                "{hz:.0} Hz · {:.1} dBFS · {:.2} seconds before this window ends",
                bins[bin.min(63)],
                window.end - time
            ));
        }
    }
    if frames.is_empty() {
        painter.text(
            plot.center(),
            egui::Align2::CENTER_CENTER,
            if reference {
                "Play the example to pin it here"
            } else {
                "Start the microphone to see your sound"
            },
            egui::FontId::proportional(13.0),
            Color32::LIGHT_GRAY,
        );
    }
    ui.horizontal(|ui| {
        for db in [-120.0, -65.0, -10.0] {
            let (swatch, _) = ui.allocate_exact_size(egui::vec2(14.0, 10.0), egui::Sense::hover());
            ui.painter()
                .rect_filled(swatch, 1.0, spectral_color(db, gain));
            ui.small(format!("{db:.0}"));
        }
        ui.small("dBFS")
            .on_hover_text("Relative to digital full scale; not room loudness.");
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    fn frame(time: f64, note: i32) -> Frame {
        Frame {
            time,
            hz: Some(dsp::hz(note as f32)),
            confidence: 1.0,
            rms: 0.1,
            clipped: false,
            spectrum: None,
        }
    }
    #[test]
    fn full_overview_keeps_every_guide_in_view_with_breaks() {
        let clips: Vec<Vec<Frame>> = (36..=84)
            .map(|note| vec![frame(0.0, note), frame(1.0, note)])
            .collect();
        let refs: Vec<_> = clips.iter().map(Vec::as_slice).collect();
        let overview = overview_pitch(&refs, 100.0);
        assert!(overview.iter().all(|f| (97.0..=100.0).contains(&f.time)));
        assert!(overview
            .iter()
            .zip(overview.iter().skip(1))
            .all(|(a, b)| a.time <= b.time));
        assert_eq!(overview.iter().filter(|f| f.hz.is_some()).count(), 98);
        assert_eq!(overview.iter().filter(|f| f.hz.is_none()).count(), 49);
        for note in 36..=84 {
            assert!(overview.iter().any(|f| f
                .hz
                .is_some_and(|hz| (dsp::midi(hz) - note as f32).abs() < 0.01)));
        }
        assert!(overview_pitch(&[], 100.0).is_empty());
    }
    #[test]
    fn pinned_guides_keep_source_times_and_break_between_clips() {
        let first: Vec<_> = (0..96).map(|i| frame(i as f64 * 0.01, 48)).collect();
        let second: Vec<_> = (0..96).map(|i| frame(i as f64 * 0.01, 50)).collect();
        let pinned = pinned_pitch(&[&first, &second], 100.0);
        assert_eq!(first[0].time, 0.0);
        assert_eq!(second[0].time, 0.0);
        let a = pinned
            .iter()
            .find(|f| f.hz.is_some_and(|h| (dsp::midi(h) - 48.0).abs() < 0.1))
            .unwrap();
        let b = pinned
            .iter()
            .find(|f| f.hz.is_some_and(|h| (dsp::midi(h) - 50.0).abs() < 0.1))
            .unwrap();
        assert!(a.time < b.time);
        assert!(pinned
            .iter()
            .any(|f| f.hz.is_none() && f.time > a.time && f.time < b.time));
        assert!(pinned
            .iter()
            .filter(|f| f.hz.is_some())
            .all(|f| (97.0..=100.0).contains(&f.time)));
        let later = pinned_pitch(&[&second], 200.0);
        assert!(
            (later.front().unwrap().time
                - pinned_pitch(&[&second], 100.0).front().unwrap().time
                - 100.0)
                .abs()
                < 0.001
        );
    }
    #[test]
    fn digital_silence_stays_dark_at_any_gain() {
        assert_eq!(spectral_color(-120.0, 0.0), spectral_color(-120.0, 30.0));
        assert_ne!(spectral_color(-90.0, 0.0), spectral_color(-90.0, 30.0));
    }
    #[test]
    fn compact_grid_fills_the_plot_and_centers_a_nearby_voice() {
        let ctx = egui::Context::default();
        let output = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                pitch_monitor(
                    ui,
                    &VecDeque::from([frame(1.0, 48)]),
                    &VecDeque::new(),
                    Some(48.into()),
                    false,
                    240.0,
                    1.0,
                );
            });
        });
        let label_y = |label: &str| {
            output
                .shapes
                .iter()
                .find_map(|s| match &s.shape {
                    egui::epaint::Shape::Text(t) if t.galley.text() == label => Some(t.pos.y),
                    _ => None,
                })
                .unwrap()
        };
        assert!(
            label_y("F3") < 45.0,
            "Top of the grid must contain note rows"
        );
        assert!((90.0..140.0).contains(&label_y("Target · C3")));
        assert!(label_y("G2") > 190.0);
    }
    #[test]
    fn compact_chart_retains_distant_voice_and_target() {
        let ctx = egui::Context::default();
        let trace = VecDeque::from([frame(1.0, 72)]);
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(720.0, 640.0),
                )),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    pitch_monitor(
                        ui,
                        &trace,
                        &VecDeque::new(),
                        Some(48.into()),
                        false,
                        240.0,
                        1.0,
                    )
                });
            },
        );
        let labels: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::epaint::Shape::Text(t) => Some(t.galley.job.text.as_str()),
                _ => None,
            })
            .collect();
        assert!(labels.contains(&"Target · C3"));
        assert!(labels.contains(&"C5"));
        assert!(labels.iter().any(|s| s.contains("st")));
    }
}

#[cfg(test)]
mod tolerance_tests {
    use super::*;
    #[test]
    fn target_band_reflects_the_selected_range_tolerance() {
        let band_height = |tolerance_cents| {
            let ctx = egui::Context::default();
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(720.0, 640.0),
                    )),
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        pitch_monitor(
                            ui,
                            &VecDeque::new(),
                            &VecDeque::new(),
                            Some(PitchTarget {
                                note: 48,
                                tolerance_cents,
                            }),
                            false,
                            240.0,
                            0.0,
                        )
                    });
                },
            );
            output
                .shapes
                .iter()
                .find_map(|s| match &s.shape {
                    egui::epaint::Shape::Rect(r)
                        if r.fill == Color32::from_rgba_unmultiplied(100, 230, 150, 35) =>
                    {
                        Some(r.rect.height())
                    }
                    _ => None,
                })
                .unwrap()
        };
        assert!((band_height(75.0) / band_height(15.0) - 5.0).abs() < 0.01);
    }
}

pub fn melody_path(
    ui: &mut egui::Ui,
    coach: &crate::melody::Coach,
    now: f64,
    height: f32,
    view: View,
    live: Option<&Frame>,
) {
    use crate::melody::{metric, State, Status};
    let live = live.filter(|f| crate::melody::reliable(f) && now >= f.time && now - f.time <= 0.25);
    let live_note = live.map(|f| dsp::midi(f.hz.unwrap()));
    let notes = coach.notes();
    let low = if view == View::Full {
        36
    } else {
        (notes.iter().map(|n| n.midi).min().unwrap() - 2)
            .min(live_note.map_or(84, |n| n.floor() as i32 - 1))
    };
    let high = if view == View::Full {
        84
    } else {
        (notes.iter().map(|n| n.midi).max().unwrap() + 2)
            .max(live_note.map_or(36, |n| n.ceil() as i32 + 1))
    };
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), height.max(160.0)),
        egui::Sense::hover(),
    );
    let plot = egui::Rect::from_min_max(
        rect.min + egui::vec2(45.0, 10.0),
        rect.max - egui::vec2(8.0, 25.0),
    );
    let x = |t: f64| plot.left() + t as f32 / coach.duration() as f32 * plot.width();
    let y = |m: f32| plot.bottom() - (m - low as f32) / (high - low) as f32 * plot.height();
    let p = ui.painter_at(rect);
    p.rect_filled(plot, 0.0, spectral_color(-120.0, 12.0));
    let clipped = p.with_clip_rect(plot);
    if view == View::Spectrum {
        for frame in &coach.frames {
            if let Some(bins) = &frame.spectrum {
                for (i, db) in bins.iter().enumerate() {
                    let hz = 65.0 * (4000.0_f32 / 65.0).powf(i as f32 / 63.0);
                    let midi = dsp::midi(hz);
                    let next = dsp::midi(hz * (4000.0_f32 / 65.0).powf(1.0 / 63.0));
                    clipped.rect_filled(
                        egui::Rect::from_min_max(
                            egui::pos2(x(frame.time), y(next)),
                            egui::pos2(x(frame.time + 0.1), y(midi)),
                        ),
                        0.0,
                        spectral_color(*db, 12.0),
                    );
                }
            }
        }
    }
    for m in low..=high {
        p.line_segment(
            [
                egui::pos2(plot.left(), y(m as f32)),
                egui::pos2(plot.right(), y(m as f32)),
            ],
            Stroke::new(0.5_f32, Color32::DARK_GRAY),
        );
        p.text(
            egui::pos2(rect.left(), y(m as f32)),
            egui::Align2::LEFT_CENTER,
            dsp::name(m),
            egui::FontId::proportional(11.0),
            Color32::GRAY,
        );
    }
    for t in 0..=coach.duration() as usize {
        p.line_segment(
            [
                egui::pos2(x(t as f64), plot.top()),
                egui::pos2(x(t as f64), plot.bottom()),
            ],
            Stroke::new(0.5_f32, Color32::DARK_GRAY),
        );
        p.text(
            egui::pos2(x(t as f64), plot.bottom() + 6.0),
            egui::Align2::CENTER_TOP,
            format!("{t}s"),
            egui::FontId::proportional(11.0),
            Color32::GRAY,
        );
    }
    for (i, n) in notes.iter().enumerate() {
        let a = coach.result.as_ref().map(|r| &r.notes[i]);
        let (color, label) = match a.map(|a| a.status) {
            Some(Status::Good) => (Color32::DARK_GREEN, "✓"),
            Some(Status::Improve) => (Color32::from_rgb(130, 85, 15), "!"),
            Some(Status::Unscored) => (Color32::DARK_GRAY, "?"),
            None => (PURPLE.gamma_multiply(0.18), "·"),
        };
        let r = egui::Rect::from_min_max(
            egui::pos2(x(n.start), y(n.midi as f32 + 0.42)),
            egui::pos2(x(n.start + n.duration), y(n.midi as f32 - 0.42)),
        );
        p.rect_filled(r, 3.0, color.gamma_multiply(0.65));
        clipped.line_segment(
            [
                egui::pos2(x(n.start), y(n.midi as f32)),
                egui::pos2(x(n.start + n.duration), y(n.midi as f32)),
            ],
            Stroke::new(2.0_f32, PURPLE),
        );
        p.text(
            r.center(),
            egui::Align2::CENTER_CENTER,
            format!("{label} {}", dsp::name(n.midi)),
            egui::FontId::proportional(12.0),
            Color32::WHITE,
        );
        ui.interact(r, ui.id().with(("melody_note", i)), egui::Sense::hover())
            .on_hover_text(if let Some(a) = a {
                format!(
                    "Nota {} · cobertura {:.0}% · viés {} · erro absoluto {} · afinado {}",
                    i + 1,
                    a.coverage * 100.0,
                    metric(a.bias, "¢"),
                    metric(a.absolute, "¢"),
                    metric(a.in_tune.map(|r| 100.0 * r), "%")
                )
            } else {
                format!("{} · {:.1}s", dsp::name(n.midi), n.duration)
            });
    }
    for (i, pair) in coach.frames.windows(2).enumerate() {
        if i % 4 < 2 && crate::melody::connected(&pair[0], &pair[1]) {
            clipped.line_segment(
                [
                    egui::pos2(x(pair[0].time), y(dsp::midi(pair[0].hz.unwrap()))),
                    egui::pos2(x(pair[1].time), y(dsp::midi(pair[1].hz.unwrap()))),
                ],
                Stroke::new(2.0_f32, Color32::LIGHT_BLUE),
            );
        }
    }
    // Live monitoring is independent of the timed, scored attempt trace.
    if let Some(note) = live_note {
        let point = egui::pos2(plot.right() - 5.0, y(note));
        clipped.circle_filled(point, 4.0, BLUE);
        p.text(
            point - egui::vec2(9.0, 9.0),
            egui::Align2::RIGHT_BOTTOM,
            format!("Live {}", dsp::name(note.round() as i32)),
            egui::FontId::proportional(12.0),
            BLUE,
        );
    }
    let cursor = match coach.state {
        State::Recording { start } => Some(live.map_or_else(
            || coach.frames.last().map_or(0.0, |f| f.time),
            |f| f.time - start,
        )),
        State::Playback => coach.playback_start.map(|start| now - start),
        _ => None,
    };
    if let Some(elapsed) = cursor {
        let t = elapsed.clamp(0.0, coach.duration());
        p.line_segment(
            [
                egui::pos2(x(t), plot.top()),
                egui::pos2(x(t), plot.bottom()),
            ],
            Stroke::new(1.0_f32, Color32::WHITE),
        );
    }
    ui.small("Purple: piano path · Azul tracejado: voz · ✓ verde: afinado · ! âmbar: melhorar · ? cinzento: dados insuficientes");
}

//! Vector icons drawn on a 16×16 grid: crisp at any size, no image assets.

use std::f32::consts::TAU;

use eframe::egui::{Color32, CornerRadius, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, Vec2};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Play,
    Pause,
    Stop,
    Previous,
    Next,
    Repeat,
    VolumeHigh,
    VolumeLow,
    VolumeMuted,
    Fullscreen,
    NowPlaying,
    Queue,
    Music,
    Video,
    Playlist,
    Settings,
    Note,
    Alert,
}

/// Paints `icon` into the largest square that fits in `rect`.
pub fn paint(painter: &Painter, rect: Rect, icon: Icon, color: Color32) {
    let pen = Pen::new(painter, rect, color);
    match icon {
        Icon::Play => pen.fill(&[(4.5, 2.5), (13.5, 8.0), (4.5, 13.5)]),
        Icon::Pause => {
            pen.bar(3.5, 2.5, 6.5, 13.5);
            pen.bar(9.5, 2.5, 12.5, 13.5);
        }
        Icon::Stop => pen.bar(3.0, 3.0, 13.0, 13.0),
        Icon::Previous => {
            pen.bar(2.5, 3.0, 4.5, 13.0);
            pen.fill(&[(13.5, 3.0), (13.5, 13.0), (5.5, 8.0)]);
        }
        Icon::Next => {
            pen.fill(&[(2.5, 3.0), (2.5, 13.0), (10.5, 8.0)]);
            pen.bar(11.5, 3.0, 13.5, 13.0);
        }
        Icon::Repeat => {
            pen.line(&[(2.5, 9.0), (2.5, 5.0), (11.0, 5.0)]);
            pen.fill(&[(10.5, 2.5), (13.5, 5.0), (10.5, 7.5)]);
            pen.line(&[(13.5, 7.0), (13.5, 11.0), (5.0, 11.0)]);
            pen.fill(&[(5.5, 8.5), (2.5, 11.0), (5.5, 13.5)]);
        }
        Icon::VolumeHigh => {
            pen.speaker();
            pen.arc((8.5, 8.0), 3.0);
            pen.arc((8.5, 8.0), 5.5);
        }
        Icon::VolumeLow => {
            pen.speaker();
            pen.arc((8.5, 8.0), 3.0);
        }
        Icon::VolumeMuted => {
            pen.speaker();
            pen.line(&[(10.5, 5.5), (14.5, 10.5)]);
            pen.line(&[(10.5, 10.5), (14.5, 5.5)]);
        }
        Icon::Fullscreen => {
            pen.line(&[(2.0, 6.0), (2.0, 2.0), (6.0, 2.0)]);
            pen.line(&[(10.0, 2.0), (14.0, 2.0), (14.0, 6.0)]);
            pen.line(&[(14.0, 10.0), (14.0, 14.0), (10.0, 14.0)]);
            pen.line(&[(6.0, 14.0), (2.0, 14.0), (2.0, 10.0)]);
        }
        Icon::NowPlaying => {
            pen.ring((8.0, 8.0), 6.5);
            pen.fill(&[(6.25, 5.0), (11.0, 8.0), (6.25, 11.0)]);
        }
        Icon::Queue => {
            pen.line(&[(2.0, 4.0), (11.0, 4.0)]);
            pen.line(&[(2.0, 8.0), (11.0, 8.0)]);
            pen.line(&[(2.0, 12.0), (7.0, 12.0)]);
            pen.fill(&[(9.5, 9.5), (14.0, 12.0), (9.5, 14.5)]);
        }
        Icon::Music => {
            pen.dot((4.5, 12.0), 2.0);
            pen.dot((11.5, 10.5), 2.0);
            pen.line(&[(6.5, 12.0), (6.5, 3.5)]);
            pen.line(&[(13.5, 10.5), (13.5, 2.0)]);
            pen.fill(&[(6.5, 3.0), (13.5, 1.5), (13.5, 3.75), (6.5, 5.25)]);
        }
        Icon::Video => {
            pen.outline(1.5, 3.0, 14.5, 13.0);
            pen.fill(&[(6.5, 5.5), (10.5, 8.0), (6.5, 10.5)]);
        }
        Icon::Playlist => {
            for y in [4.0, 8.0, 12.0] {
                pen.dot((2.5, y), 1.0);
                pen.line(&[(5.5, y), (14.0, y)]);
            }
        }
        Icon::Settings => {
            pen.ring((8.0, 8.0), 2.25);
            pen.ring((8.0, 8.0), 5.0);
            for tooth in 0..8 {
                pen.tooth(tooth as f32 * TAU / 8.0);
            }
        }
        Icon::Note => {
            pen.dot((6.0, 11.5), 2.5);
            pen.line(&[(8.25, 11.5), (8.25, 2.5), (12.5, 4.5), (12.5, 7.0)]);
        }
        Icon::Alert => {
            pen.ring((8.0, 8.0), 6.5);
            pen.bar(7.25, 4.0, 8.75, 9.25);
            pen.dot((8.0, 11.5), 1.0);
        }
    }
}

/// Maps 16×16 grid coordinates onto the target square.
struct Pen<'a> {
    painter: &'a Painter,
    origin: Pos2,
    unit: f32,
    color: Color32,
}

impl<'a> Pen<'a> {
    fn new(painter: &'a Painter, rect: Rect, color: Color32) -> Self {
        let side = rect.width().min(rect.height());
        Self {
            painter,
            origin: rect.center() - Vec2::splat(side / 2.0),
            unit: side / 16.0,
            color,
        }
    }

    fn at(&self, (x, y): (f32, f32)) -> Pos2 {
        self.origin + Vec2::new(x, y) * self.unit
    }

    fn stroke(&self) -> Stroke {
        Stroke::new((1.5 * self.unit).max(1.0), self.color)
    }

    fn fill(&self, points: &[(f32, f32)]) {
        let points = points.iter().map(|&point| self.at(point)).collect();
        self.painter
            .add(Shape::convex_polygon(points, self.color, Stroke::NONE));
    }

    fn line(&self, points: &[(f32, f32)]) {
        let points = points.iter().map(|&point| self.at(point)).collect();
        self.painter.add(Shape::line(points, self.stroke()));
    }

    fn bar(&self, x0: f32, y0: f32, x1: f32, y1: f32) {
        let rect = Rect::from_min_max(self.at((x0, y0)), self.at((x1, y1)));
        self.painter
            .rect_filled(rect, CornerRadius::from(self.unit), self.color);
    }

    fn outline(&self, x0: f32, y0: f32, x1: f32, y1: f32) {
        let rect = Rect::from_min_max(self.at((x0, y0)), self.at((x1, y1)));
        self.painter.rect_stroke(
            rect,
            CornerRadius::from(2.0 * self.unit),
            self.stroke(),
            StrokeKind::Middle,
        );
    }

    fn dot(&self, center: (f32, f32), radius: f32) {
        self.painter
            .circle_filled(self.at(center), radius * self.unit, self.color);
    }

    fn ring(&self, center: (f32, f32), radius: f32) {
        self.painter
            .circle_stroke(self.at(center), radius * self.unit, self.stroke());
    }

    /// A ±50° arc opening to the right, for speaker sound waves.
    fn arc(&self, (cx, cy): (f32, f32), radius: f32) {
        let points = (0..=12)
            .map(|step| {
                let angle = (-50.0 + 100.0 * step as f32 / 12.0_f32).to_radians();
                self.at((cx + radius * angle.cos(), cy + radius * angle.sin()))
            })
            .collect();
        self.painter.add(Shape::line(points, self.stroke()));
    }

    fn speaker(&self) {
        self.bar(1.5, 6.0, 4.5, 10.0);
        self.fill(&[(4.0, 6.0), (8.0, 2.5), (8.0, 13.5), (4.0, 10.0)]);
    }

    fn tooth(&self, angle: f32) {
        let (sin, cos) = angle.sin_cos();
        let inner = self.at((8.0 + 5.0 * cos, 8.0 + 5.0 * sin));
        let outer = self.at((8.0 + 7.0 * cos, 8.0 + 7.0 * sin));
        self.painter
            .line_segment([inner, outer], Stroke::new(2.25 * self.unit, self.color));
    }
}

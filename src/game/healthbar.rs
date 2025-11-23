use crate::types::Stats;
use macroquad::prelude::*;

pub struct HealthBar {
    width: f32,
    height: f32,
    margin: f32,
    max_value: u32,
    anchor: HealthBarAnchor,
}

#[derive(Clone, Copy)]
pub enum HealthBarAnchor {
    TopLeft,
}

impl HealthBar {
    /// Construit une barre de vie paramétrable (dimensions, marge, ancrage).
    pub fn new(
        width: f32,
        height: f32,
        margin: f32,
        max_value: u32,
        anchor: HealthBarAnchor,
    ) -> Self {
        Self {
            width,
            height,
            margin,
            max_value,
            anchor,
        }
    }

    /// Dessine la barre de vie à partir des statistiques fournies et d'un point d'origine.
    pub fn draw_at(&self, stats: &Stats, origin_x: f32, origin_y: f32) {
        let (x, y) = match self.anchor {
            HealthBarAnchor::TopLeft => (origin_x + self.margin, origin_y + self.margin),
        };
        let bg = Color::new(0.1, 0.1, 0.1, 0.75);
        draw_rectangle(x, y, self.width, self.height, bg);
        draw_rectangle_lines(x, y, self.width, self.height, 2.0, WHITE);

        let ratio = if self.max_value == 0 {
            0.0
        } else {
            (stats.vie as f32 / self.max_value as f32).clamp(0.0, 1.0)
        };
        let filled = self.width * ratio;
        let fill_color = Color::new(0.75, 0.1, 0.15, 0.9);
        draw_rectangle(x, y, filled, self.height, fill_color);

        let label = format!("PV: {}/{}", stats.vie, self.max_value);
        let text_dims = measure_text(&label, None, 22, 1.0);
        let text_x = x + (self.width - text_dims.width) * 0.5;
        let text_y = y + self.height * 0.5 + text_dims.height * 0.35;
        draw_text(&label, text_x, text_y, 22.0, WHITE);
    }
}

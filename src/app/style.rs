use std::{
    collections::HashMap,
    fmt::Formatter,
    time::{Duration, Instant},
};

use egui::{CollapsingHeader, Color32, ComboBox, DragValue};
use galileo::{
    layer::vector_tile_layer::style::{
        StyleRule, VectorTileLineSymbol, VectorTilePointSymbol, VectorTilePolygonSymbol,
        VectorTileSymbol,
    },
    Color,
};
use itertools::Itertools;
use serde::{Deserialize, Serialize};

use super::VectorTileStyle;

const UPDATE_TIMEOUT: Duration = Duration::from_millis(100);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StyleWindow {
    is_changed: bool,
    #[serde(skip)]
    last_changed_at: Option<Instant>,
    background_color: egui::Color32,
    rules: Vec<Rule>,
    last_rule_id: u64,
}

impl StyleWindow {
    pub fn new(style: VectorTileStyle) -> Self {
        let mut last_id = 0;
        let rules = style
            .rules
            .iter()
            .map(|style_rule| {
                last_id += 1;
                Rule::new(style_rule, last_id)
            })
            .collect();

        Self {
            is_changed: false,
            last_changed_at: None,
            background_color: to_egui_color(style.background),
            rules,
            last_rule_id: last_id,
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> &mut Self {
        egui::Window::new("Layer Style")
            .resizable([false, true])
            .default_width(300.0)
            .default_height(600.0)
            .scroll([false, true])
            .show(ctx, |ui| self.ui(ctx, ui));

        self
    }

    pub fn is_changed(&self) -> bool {
        self.is_changed
    }

    pub fn style(&self) -> VectorTileStyle {
        VectorTileStyle {
            rules: self.rules.iter().map(Rule::get_rule).collect(),
            background: to_galileo_color(self.background_color),
        }
    }

    fn ui(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Background");
            if ui
                .color_edit_button_srgba(&mut self.background_color)
                .changed()
            {
                self.mark_changed(ctx);
            }
        });

        ui.separator();

        ui.label("Rules");

        let mut ui_action = None;
        for (index, rule) in self.rules.iter_mut().enumerate() {
            let action = rule.ui(ui).action();
            if action != RuleAction::None {
                ui_action = Some((index, action));
            }
        }

        if let Some((index, action)) = ui_action {
            match action {
                RuleAction::MoveUp if index > 0 => {
                    self.rules.swap(index, index - 1);
                }
                RuleAction::MoveDown if index < self.rules.len() - 1 => {
                    self.rules.swap(index, index + 1);
                }
                RuleAction::Remove => {
                    self.rules.remove(index);
                }
                _ => {}
            }

            self.last_changed_at = Some(Instant::now());
            ctx.request_repaint_after(UPDATE_TIMEOUT);
        }

        ui.horizontal(|ui| {
            ui.label("Add new rule");
            if ui.button("+").clicked() {
                let id = self.next_rule_id();
                self.rules.push(Rule::new_empty(id));
            }
        });

        self.update_changed();
    }

    fn next_rule_id(&mut self) -> u64 {
        self.last_rule_id += 1;
        self.last_rule_id
    }

    fn update_changed(&mut self) {
        let mut timed_out = false;
        if let Some(changed_at) = self.last_changed_at {
            if Instant::now() >= changed_at + UPDATE_TIMEOUT {
                timed_out = true;
            }
        }

        if timed_out {
            self.is_changed = true;
            self.last_changed_at = None;
        }
    }

    fn mark_changed(&mut self, ctx: &egui::Context) {
        self.last_changed_at = Some(Instant::now());
        ctx.request_repaint_after(UPDATE_TIMEOUT);
    }

    pub fn mark_unchanged(&mut self) {
        self.is_changed = false;
    }
}

fn to_egui_color(color: Color) -> Color32 {
    Color32::from_rgba_premultiplied(color.r(), color.g(), color.b(), color.a())
}

fn to_galileo_color(color: Color32) -> Color {
    Color::rgba(color.r(), color.g(), color.b(), color.a())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Rule {
    id: u64,
    layer_name: String,
    filter: String,
    color: Color32,
    size: f64,
    symbol_type: SymbolType,
    action: RuleAction,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
enum RuleAction {
    None,
    Modified,
    MoveUp,
    MoveDown,
    Remove,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
enum SymbolType {
    None,
    Point,
    Line,
    Polygon,
}

impl std::fmt::Display for SymbolType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            SymbolType::None => write!(f, "none"),
            SymbolType::Point => write!(f, "point"),
            SymbolType::Line => write!(f, "line"),
            SymbolType::Polygon => write!(f, "polygon"),
        }
    }
}

impl Rule {
    fn new(style_rule: &StyleRule, id: u64) -> Self {
        let filter = style_rule
            .properties
            .iter()
            .map(|(key, value)| format!("{key} == {value}"))
            .join(" && ");
        let (color, size, symbol_type) = match style_rule.symbol {
            VectorTileSymbol::Point(s) => (to_egui_color(s.color), s.size, SymbolType::Point),
            VectorTileSymbol::Line(s) => (to_egui_color(s.stroke_color), s.width, SymbolType::Line),
            VectorTileSymbol::Polygon(s) => (to_egui_color(s.fill_color), 0.0, SymbolType::Polygon),
            _ => (to_egui_color(Color::TRANSPARENT), 0.0, SymbolType::None),
        };

        Self {
            id,
            layer_name: style_rule.layer_name.clone().unwrap_or_default(),
            filter,
            color,
            size,
            symbol_type,
            action: RuleAction::None,
        }
    }

    fn new_empty(id: u64) -> Self {
        Self {
            id,
            layer_name: String::from(""),
            filter: String::from(""),
            color: Color32::from_rgba_unmultiplied(0, 0, 0, 0),
            size: 1.0,
            symbol_type: SymbolType::None,
            action: RuleAction::None,
        }
    }

    fn get_rule(&self) -> StyleRule {
        let layer_name = match self.layer_name.as_str() {
            "" => None,
            v => Some(v.to_string()),
        };
        let symbol = match self.symbol_type {
            SymbolType::None => VectorTileSymbol::None,
            SymbolType::Point => VectorTileSymbol::Point(VectorTilePointSymbol {
                size: self.size,
                color: to_galileo_color(self.color),
            }),
            SymbolType::Line => VectorTileSymbol::Line(VectorTileLineSymbol {
                width: self.size,
                stroke_color: to_galileo_color(self.color),
            }),
            SymbolType::Polygon => VectorTileSymbol::Polygon(VectorTilePolygonSymbol {
                fill_color: to_galileo_color(self.color),
            }),
        };

        StyleRule {
            layer_name,
            properties: self.parse_filter().unwrap_or_default(),
            symbol,
        }
    }

    fn parse_filter(&self) -> Option<HashMap<String, String>> {
        let split = self.filter.split("&&");
        let mut properties = HashMap::new();
        for block in split {
            let blocks: Vec<&str> = block.split("==").map(|v| v.trim()).collect();
            if blocks.len() != 2 {
                return None;
            }
            properties.insert(blocks[0].to_string(), blocks[1].to_string());
        }

        if properties.is_empty() {
            None
        } else {
            Some(properties)
        }
    }

    fn action(&self) -> RuleAction {
        self.action
    }

    fn ui(&mut self, ui: &mut egui::Ui) -> &mut Self {
        self.action = RuleAction::None;
        let mut changed = false;
        CollapsingHeader::new(self.header())
            .id_salt(self.id)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    if ui.button("Down").clicked() {
                        self.action = RuleAction::MoveDown;
                    }

                    if ui.button("Up").clicked() {
                        self.action = RuleAction::MoveUp;
                    }

                    if ui.button("Del").clicked() {
                        self.action = RuleAction::Remove;
                    }
                });

                ui.horizontal(|ui| {
                    ui.label("Layer name");
                    changed = changed || ui.text_edit_singleline(&mut self.layer_name).changed();
                });

                ui.horizontal(|ui| {
                    ui.label("Filter");
                    changed = changed || ui.text_edit_singleline(&mut self.filter).changed();
                });

                ui.horizontal(|ui| {
                    ui.label("Type");
                    let v = &mut self.symbol_type;
                    changed = changed
                        || ComboBox::new("symbol type", "")
                            .selected_text(v.to_string())
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    v,
                                    SymbolType::None,
                                    SymbolType::None.to_string(),
                                );
                                ui.selectable_value(
                                    v,
                                    SymbolType::Point,
                                    SymbolType::Point.to_string(),
                                );
                                ui.selectable_value(
                                    v,
                                    SymbolType::Line,
                                    SymbolType::Line.to_string(),
                                );
                                ui.selectable_value(
                                    v,
                                    SymbolType::Polygon,
                                    SymbolType::Polygon.to_string(),
                                );
                            })
                            .response
                            .changed();

                    if !matches!(self.symbol_type, SymbolType::None) {
                        changed = changed || ui.color_edit_button_srgba(&mut self.color).changed();
                    }

                    if matches!(self.symbol_type, SymbolType::Point | SymbolType::Line) {
                        changed = changed
                            || ui
                                .add(DragValue::new(&mut self.size).speed(0.01).range(0.0..=20.0))
                                .changed();
                    }
                });
            });

        if self.action == RuleAction::None && changed {
            self.action = RuleAction::Modified;
        }

        self
    }

    fn header(&self) -> String {
        format!("{} ({})", self.layer_name, self.filter)
    }
}

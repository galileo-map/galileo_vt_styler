use std::sync::Arc;

use eframe::Frame;
use galileo::{
    Lod, Map, MapView, TileSchema, control::{EventPropagation, MouseButton, UserEvent, UserEventHandler}, layer::{
        VectorTileLayer, vector_tile_layer::{VectorTileLayerBuilder, style::VectorTileStyle}
    }, render::text::{RustybuzzRasterizer, text_service::TextService}, tile_schema::{TileIndex, VerticalDirection}
};
use galileo_egui::{EguiMap, EguiMapState};
use galileo_types::{
    cartesian::{Point2, Rect},
    geo::Crs,
    latlon,
};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use style::StyleWindow;

mod style;

pub struct GalileoApp {
    map_state: EguiMapState,
    vt_layer: Arc<RwLock<VectorTileLayer>>,
    style_window: StyleWindow,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AppState {
    style_window: StyleWindow,
}

impl GalileoApp {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // This is also where you can customize the look and feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.

        let rasterizer = RustybuzzRasterizer::default();
        TextService::initialize(rasterizer).load_fonts("assets/fonts");

        let state: Option<AppState> = cc
            .storage
            .and_then(|storage| eframe::get_value(storage, eframe::APP_KEY));
        let style_window = match state {
            Some(v) => v.style_window,
            None => StyleWindow::new(get_layer_style().unwrap_or_default()),
        };

        let map_view = MapView::new(&latlon!(55.0, 37.0), 20_000.0);

        let api_key = "yovW4kTYgmIPv7WyXTYt";

        let layer = VectorTileLayerBuilder::new_rest(move |&index: &TileIndex| {
            format!(
                //"https://api.maptiler.com/tiles/v3-openmaptiles/{z}/{x}/{y}.pbf?key={api_key}",
                "https://api.maptiler.com/tiles/v3/{z}/{x}/{y}.pbf?key={api_key}",
                z = index.z,
                x = index.x,
                y = index.y
            )
        })
        .with_style(style_window.style())
        .with_tile_schema(tile_scheme())
        .with_file_cache_checked(".tile_cache")
        .with_attribution(
            "© MapTiler© OpenStreetMap contributors".to_string(),
            "https://www.maptiler.com/copyright/".to_string(),
        )
        .build()
        .expect("failed to create layer");

        let layer = Arc::new(RwLock::new(layer));
        let layer_copy = layer.clone();
        let map = Map::new(map_view, vec![Box::new(layer.clone())], None);

        let handler = move |ev: &UserEvent, map: &mut Map| match ev {
            UserEvent::Click(MouseButton::Left, mouse_event) => {
                let view = map.view().clone();
                if let Some(position) = map
                    .view()
                    .screen_to_map(mouse_event.screen_pointer_position)
                {
                    let features = layer_copy.read().get_features_at(&position, &view);

                    println!("Clicked at {} objects:", features.len());
                    for (layer, feature) in features {
                        println!("{layer}, {:?}", feature.properties);
                    }
                }

                EventPropagation::Stop
            }
            _ => EventPropagation::Propagate,
        };

        let ctx = cc.egui_ctx.clone();
        let render_state = cc
            .wgpu_render_state
            .clone()
            .expect("failed to get wgpu context");

        let handler: Box<dyn UserEventHandler> = Box::new(handler);
        GalileoApp {
            map_state: EguiMapState::new(
                map,
                ctx,
                render_state,
                [handler],
                galileo_egui::EguiMapOptions::default(),
            ),
            vt_layer: layer,
            style_window,
        }
    }

    fn state(&self) -> AppState {
        AppState {
            style_window: self.style_window.clone(),
        }
    }
}

fn get_layer_style() -> Option<VectorTileStyle> {
    const STYLE: &str = "../galileo/galileo/examples/data/vt_style.json";
    serde_json::from_reader(std::fs::File::open(STYLE).ok()?).ok()
}

pub fn tile_scheme() -> TileSchema {
    const ORIGIN: Point2 = Point2::new(-20037508.342787, 20037508.342787);
    const TOP_RESOLUTION: f64 = 156543.03392800014 / 4.0;

    let mut lods = vec![Lod::new(TOP_RESOLUTION, 0).unwrap()];
    for i in 1..16 {
        lods.push(Lod::new(lods[(i - 1) as usize].resolution() / 2.0, i).unwrap());
    }

    TileSchema {
        origin: ORIGIN,
        bounds: Rect::new(
            -20037508.342787,
            -20037508.342787,
            20037508.342787,
            20037508.342787,
        ),
        lods: lods.into_iter().collect(),
        tile_width: 1024,
        tile_height: 1024,
        y_direction: VerticalDirection::TopToBottom,
        crs: Crs::EPSG3857,
    }
}

impl eframe::App for GalileoApp {
    /// Called by the frame work to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, &self.state());
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut Frame) {
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                let is_web = cfg!(target_arch = "wasm32");
                if !is_web {
                    ui.menu_button("File", |ui| {
                        if ui.button("Quit").clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });
                    ui.add_space(16.0);
                }

                egui::widgets::global_theme_preference_buttons(ui);
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            EguiMap::new(&mut self.map_state).show_ui(ui);

            ui.with_layout(egui::Layout::bottom_up(egui::Align::RIGHT), |ui| {
                egui::warn_if_debug_build(ui);
            });

            if self.style_window.show(ctx).is_changed() {
                self.vt_layer
                    .write()
                    .update_style(self.style_window.style().clone());
                self.map_state.request_redraw();
                self.style_window.mark_unchanged();
            }
        });
    }
}

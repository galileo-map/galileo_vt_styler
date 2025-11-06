use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use eframe::{
    wgpu::{FilterMode, TextureView},
    Frame,
};
use egui::{load::SizedTexture, Event, Image, ImageSource, Sense, TextureId, Vec2};
use galileo::{
    control::{EventProcessor, MapController, MouseButton, RawUserEvent, UserEventHandler},
    render::WgpuRenderer,
    Map, Messenger,
};
use galileo_types::cartesian::{Point2, Size};

pub struct EguiMapState {
    map: Map,
    renderer: WgpuRenderer,
    requires_redraw: Arc<AtomicBool>,
    texture_id: TextureId,
    texture_view: TextureView,
    event_processor: EventProcessor,
}

impl EguiMapState {
    pub fn new(mut map: Map, cc: &eframe::CreationContext<'_>, handler: impl UserEventHandler + 'static) -> Self {
        let requires_redraw = Arc::new(AtomicBool::new(true));
        let messenger = MapStateMessenger {
            context: cc.egui_ctx.clone(),
            requires_redraw: requires_redraw.clone(),
        };

        map.set_messenger(Some(messenger.clone()));
        for layer in map.layers_mut().iter_mut() {
            layer.set_messenger(Box::new(messenger.clone()));
        }

        let render_state = cc.wgpu_render_state.as_ref().unwrap();

        // Set a default size so that render target can be created.
        // This size will be replaced by the UI on the first frame.
        let size = Size::new(800, 800);
        map.set_size(size.cast());

        let renderer = WgpuRenderer::new_with_device_and_texture(
            render_state.device.clone(),
            render_state.queue.clone(),
            size,
        );
        let texture = renderer
            .get_target_texture_view()
            .expect("failed to get map texture");
        let texture_id = render_state.renderer.write().register_native_texture(
            &render_state.device,
            &texture,
            FilterMode::Nearest,
        );

        let mut event_processor = EventProcessor::default();
        event_processor.add_handler(handler);
        event_processor.add_handler(MapController::default());

        EguiMapState {
            map,
            renderer,
            requires_redraw,
            texture_id,
            texture_view: texture,
            event_processor,
        }
    }

    pub fn request_redraw(&self) {
        self.map.redraw();
    }
}

#[derive(Debug, Clone)]
pub struct MapStateMessenger {
    pub requires_redraw: Arc<AtomicBool>,
    pub context: egui::Context,
}

impl Messenger for MapStateMessenger {
    fn request_redraw(&self) {
        log::trace!("Redraw requested");
        if !self.requires_redraw.swap(true, Ordering::Relaxed) {
            self.context.request_repaint();
        }
    }
}

pub struct EguiMap<'a> {
    map_state: &'a mut EguiMapState,
}

impl<'a> EguiMap<'a> {
    pub fn new(map_state: &'a mut EguiMapState) -> Self {
        Self { map_state }
    }

    pub fn render(&mut self, ui: &mut egui::Ui, frame: &mut Frame) {
        let available_size = ui.available_size();
        let map_size = self.map_state.renderer.size().cast::<f32>();

        let (rect, response) = ui.allocate_exact_size(available_size, Sense::click_and_drag());

        if self.map_state.event_processor.is_dragging() || response.contains_pointer() {
            let events = ui.input(|input_state| input_state.events.clone());
            self.process_events(&events);
        }

        self.map_state.map.animate();

        if available_size[0] != map_size.width() || available_size[1] != map_size.height() {
            self.resize_map(available_size, frame);
        }

        if self
            .map_state
            .requires_redraw
            .swap(false, Ordering::Relaxed)
        {
            self.draw();
        }

        Image::new(ImageSource::Texture(SizedTexture::new(
            self.map_state.texture_id,
            Vec2::new(map_size.width(), map_size.height()),
        )))
        .paint_at(ui, rect);
    }

    #[allow(dead_code)]
    fn resize_map(&mut self, size: Vec2, frame: &mut Frame) {
        log::trace!("Resizing map to size: {size:?}");

        let size = Size::new(size.x as f64, size.y as f64);
        self.map_state.map.set_size(size);

        let size = Size::new(size.width() as u32, size.height() as u32);
        self.map_state.renderer.resize(size);

        // After renderer is resized, a new texture is created, so we need to update its id that we
        // use in UI.
        let texture = self
            .map_state
            .renderer
            .get_target_texture_view()
            .expect("failed to get map texture");
        let render_state = frame.wgpu_render_state().unwrap();
        let texture_id = render_state.renderer.write().register_native_texture(
            &render_state.device,
            &texture,
            FilterMode::Nearest,
        );

        self.map_state.texture_id = texture_id;
        self.map_state.texture_view = texture;

        self.map_state.map.redraw();
    }

    fn draw(&mut self) {
        log::trace!("Redrawing the map");
        self.map_state.map.load_layers();
        self.map_state
            .renderer
            .render_to_texture_view(&self.map_state.map, &self.map_state.texture_view);
    }

    fn process_events(&mut self, events: &[Event]) {
        for event in events {
            if let Some(raw_event) = Self::convert_event(event) {
                self.map_state
                    .event_processor
                    .handle(raw_event, &mut self.map_state.map);
            }
        }
    }

    fn convert_event(event: &Event) -> Option<RawUserEvent> {
        match event {
            Event::PointerButton {
                button, pressed, ..
            } => {
                let button = match button {
                    egui::PointerButton::Primary => MouseButton::Left,
                    egui::PointerButton::Secondary => MouseButton::Right,
                    egui::PointerButton::Middle => MouseButton::Middle,
                    _ => MouseButton::Other,
                };

                Some(match pressed {
                    true => RawUserEvent::ButtonPressed(button),
                    false => RawUserEvent::ButtonReleased(button),
                })
            }
            Event::PointerMoved(position) => {
                let scale = 1.0;
                let pointer_position =
                    Point2::new(position.x as f64 / scale, position.y as f64 / scale);
                Some(RawUserEvent::PointerMoved(pointer_position))
            }
            Event::MouseWheel { delta, .. } => {
                let zoom = delta[1] as f64;
                if zoom.abs() < 0.0001 {
                    return None;
                }

                Some(RawUserEvent::Scroll(zoom))
            }

            _ => None,
        }
    }
}

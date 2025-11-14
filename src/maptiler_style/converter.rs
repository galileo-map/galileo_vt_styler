//! MapTiler to Galileo Style Converter
//!
//! This module converts MapTiler/Mapbox GL styles to Galileo VectorTileStyle format.
//!
//! # Limitations and Unsupported Features
//!
//! Due to differences between MapTiler and Galileo style formats, the following features
//! are **NOT supported** and will be ignored during conversion:
//!
//! ## Layer Types
//! - `raster` - Raster tile layers are not converted
//! - `hillshade` - Hillshade layers are not converted
//! - `heatmap` - Heatmap layers are not converted
//! - `background` - Background layers are extracted for background color only, not as rules
//!
//! ## Paint Properties
//! - **Expressions** - Only constant values are supported. Complex expressions like:
//!   - `["interpolate", ...]` - Zoom-based interpolation
//!   - `["match", ...]` - Conditional styling (except in filters)
//!   - `["get", ...]` - Property references in paint
//!   - `["case", ...]` - Conditional expressions
//!   - `["step", ...]` - Step functions
//!   - Any data-driven styling based on feature properties
//! - **Stops** - Zoom-based styling with stops arrays
//! - **Functions** - Any function-based property values
//!
//! ## Filter Expressions
//! - Complex logical operators:
//!   - `["any", ...]` - OR logic (only `["all", ...]` with simple equality is supported)
//!   - `["none", ...]` - NOT logic
//!   - `["!has", ...]` - Negative property checks
//!   - `["!=", ...]` - Inequality comparisons
//! - Comparison operators:
//!   - `["<", ...]`, `["<=", ...]`, `[">", ...]`, `[">=", ...]`
//! - String operators:
//!   - `["in", "$type", ...]` - Geometry type filters (except simple property "in")
//! - Nested expressions and complex filter combinations
//!
//! ## Zoom Levels
//! - `minzoom` - Minimum zoom level for layer visibility
//! - `maxzoom` - Maximum zoom level for layer visibility
//!
//! ## Layout Properties
//! - All layout properties are ignored, including:
//!   - `visibility` - Layer visibility
//!   - `line-cap`, `line-join` - Line styling
//!   - `symbol-placement` - Symbol positioning
//!   - `text-field`, `text-font`, `text-size` - Text styling
//!   - `icon-image`, `icon-size` - Icon styling
//!   - All other layout properties
//!
//! ## Advanced Paint Properties
//! - `fill-pattern` - Pattern fills
//! - `line-pattern` - Pattern strokes
//! - `line-dasharray` - Dashed lines (partially visible in output but not styled)
//! - `line-gradient` - Gradient strokes
//! - `fill-extrusion-height` - 3D extrusion heights
//! - `text-halo-*` - Text halo properties
//! - `icon-halo-*` - Icon halo properties
//! - Blend modes and composite operations
//!
//! ## Sources
//! - Source definitions are not converted (only layer styles are converted)
//! - Tile URLs and attribution are ignored
//!
//! ## Metadata
//! - Layer groups and organization
//! - Copyright and attribution information
//! - Custom metadata fields
//!
//! # What IS Supported
//!
//! - **Layer types**: `fill`, `line`, `circle`, `symbol` (converted to Point), `fill-extrusion` (as Polygon)
//! - **Constant colors**: Hex (#RGB, #RRGGBB), RGB, RGBA, HSL, HSLA
//! - **Simple numeric properties**: `fill-opacity`, `line-width`, `circle-radius`, etc.
//! - **Simple equality filters**: `["==", "property", "value"]`
//! - **IN filters**: `["in", "property", "value1", "value2", ...]` (converted to multiple rules)
//! - **Combined filters**: `["all", ...]` with simple equality filters only
//! - **Background color**: Extracted from background layer

use galileo::{
    layer::vector_tile_layer::style::{
        PropertyFilter, PropertyFilterOperator, StyleRule, VectorTileLabelSymbol,
        VectorTileLineSymbol, VectorTilePolygonSymbol, VectorTileStyle, VectorTileSymbol,
    },
    render::text::{FontWeight, TextStyle},
    Color,
};
use serde_json::Value;

use super::{Layer, LayerType, Style};

/// Convert a MapTiler style to a Galileo VectorTileStyle
pub fn convert_maptiler_to_galileo(maptiler_style: &Style) -> VectorTileStyle {
    let mut rules = Vec::new();

    for layer in &maptiler_style.layers {
        // Convert each layer to one or more style rules
        let layer_rules = convert_layer(layer);
        rules.extend(layer_rules);
    }

    // Extract background color if present
    let background = extract_background_color(maptiler_style);

    VectorTileStyle { rules, background }
}

/// Convert a single MapTiler layer to one or more Galileo style rules
fn convert_layer(layer: &Layer) -> Option<StyleRule> {
    // Skip layers without source-layer (like background)
    let layer_name = match &layer.source_layer {
        Some(name) => name.clone(),
        None => return None,
    };

    // Determine the symbol type from layer type
    let symbol = match layer.layer_type {
        LayerType::Fill | LayerType::FillExtrusion => extract_polygon_symbol(&layer.paint),
        LayerType::Line => extract_line_symbol(&layer.paint),
        LayerType::Circle | LayerType::Symbol => extract_point_symbol(&layer.paint, &layer.layout)?,
        _ => return None, // Skip unsupported types
    };

    // Only create rules if we have a valid symbol
    if matches!(symbol, VectorTileSymbol::None) {
        return None;
    }

    // Parse filters and create rules
    if let Some(filter) = &layer.filter {
        let galileo_filters = parse_filter_to_rules(filter);
        if galileo_filters.is_none() {
            println!("Failed to convert filter of layer {layer_name}: {filter}");
        }

        Some(StyleRule {
            layer_name: Some(layer_name),
            properties: galileo_filters?,
            symbol,
        })
    } else {
        // No filter - create a single rule
        Some(StyleRule {
            layer_name: Some(layer_name),
            properties: vec![],
            symbol,
        })
    }
}

/// Parse MapTiler filter expressions into Galileo style rules
fn parse_filter_to_rules(filter: &Value) -> Option<Vec<PropertyFilter>> {
    let Some(filter_arr) = filter.as_array() else {
        return None;
    };

    if filter_arr.is_empty() {
        return None;
    }

    let operator = match filter_arr[0].as_str().unwrap_or("") {
        "!in" => "not in",
        "has" => "exist",
        "!has" => "not exist",
        v => v,
    };

    let value = match operator {
        "all" => {
            let mut filters = Vec::new();
            for sub_filter in &filter_arr[1..] {
                if let Some(mut sub_filters) = parse_filter_to_rules(sub_filter) {
                    filters.append(&mut sub_filters);
                } else {
                    println!("Skipped part of the filter: {sub_filter:?}");
                }
            }

            return Some(filters);
        }
        "in" | "not in" => {
            let mut values = Vec::new();
            for val in &filter_arr[2..] {
                values.push(value_to_string(val));
            }
            values.join(",")
        }
        "exist" | "not exist" => String::new(),
        _ => value_to_string(&filter_arr[2]),
    };

    if let Some(filter_operator) = PropertyFilterOperator::from_str(operator, &value) {
        let property = filter_arr[1].as_str().unwrap_or("").to_string();

        if property.starts_with('$') {
            // Skip special properties like geometry types
            return None;
        }

        Some(vec![PropertyFilter {
            property_name: property,
            operator: filter_operator,
        }])
    } else {
        eprintln!(
            "Failed to parse filter operator: {} with value: {}",
            operator, value
        );
        None
    }
}

/// Convert a JSON value to a string representation
fn value_to_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Array(arr) => {
            let parts: Vec<String> = arr.iter().map(value_to_string).collect();
            parts.join(",")
        }
        _ => String::new(),
    }
}

/// Extract polygon symbol from paint properties
fn extract_polygon_symbol(paint: &Option<Value>) -> VectorTileSymbol {
    let paint = match paint {
        Some(p) => p,
        None => return VectorTileSymbol::None,
    };

    // Try to extract fill-color
    if let Some(color) = extract_color(paint, "fill-color") {
        // Apply fill-opacity if present
        let opacity = extract_number(paint, "fill-opacity").unwrap_or(1.0);
        let fill_color = apply_opacity(color, opacity);

        return VectorTileSymbol::Polygon(VectorTilePolygonSymbol { fill_color });
    }

    VectorTileSymbol::None
}

/// Extract line symbol from paint properties
fn extract_line_symbol(paint: &Option<Value>) -> VectorTileSymbol {
    let paint = match paint {
        Some(p) => p,
        None => return VectorTileSymbol::None,
    };

    // Try to extract line-color and line-width
    let stroke_color = extract_color(paint, "line-color").unwrap_or(Color::BLACK);
    let width = extract_number(paint, "line-width").unwrap_or(1.0);

    // Apply line-opacity if present
    let opacity = extract_number(paint, "line-opacity").unwrap_or(1.0);
    let stroke_color = apply_opacity(stroke_color, opacity);

    VectorTileSymbol::Line(VectorTileLineSymbol {
        width,
        stroke_color,
    })
}

/// Extract point symbol from paint properties
fn extract_point_symbol(paint: &Option<Value>, layout: &Option<Value>) -> Option<VectorTileSymbol> {
    let paint = match paint {
        Some(p) => p,
        None => return None,
    };
    let layout = match layout {
        Some(l) => l,
        None => return None,
    };

    let text_field = layout.get("text-field")?.as_str()?.to_string();

    let Some(font_size) = extract_number(layout, "text-size") else {
        eprintln!(
            "Failed to parse text-size from layout: {:?}",
            layout.get("text-size")
        );
        return None;
    };

    let Some(font_color) = extract_color(paint, "text-color") else {
        eprintln!(
            "Failed to parse text-color from paint: {:?}",
            paint.get("text-color")
        );
        return None;
    };

    let Some(outline_width) = extract_number(paint, "text-halo-width") else {
        eprintln!(
            "Failed to parse text-halo-width from paint: {:?}",
            paint.get("text-halo-width")
        );
        return None;
    };

    let Some(outline_color) = extract_color(paint, "text-halo-color") else {
        eprintln!(
            "Failed to parse text-halo-color from paint: {:?}",
            paint.get("text-halo-color")
        );
        return None;
    };

    Some(VectorTileSymbol::Label(VectorTileLabelSymbol {
        pattern: text_field,
        text_style: TextStyle {
            font_family: vec![
                "Noto Sans".to_string(),
                "Noto Sans Arabic".to_string(),
                "Noto Sans Hebrew".to_string(),
                "Noto Sans SC".to_string(),
                "Noto Sans KR".to_string(),
                "Noto Sans JP".to_string(),
            ],
            font_size: font_size as f32,
            font_color,
            horizontal_alignment: Default::default(),
            vertical_alignment: Default::default(),
            weight: FontWeight::BOLD,
            style: Default::default(),
            outline_width: outline_width as f32 * 2.0,
            outline_color,
        },
    }))
}

/// Extract a color from paint properties
fn extract_color(paint: &Value, property: &str) -> Option<Color> {
    let value = paint.get(property)?;

    match value {
        Value::String(color_str) => parse_color(color_str),
        Value::Object(color_obj) => {
            let steps = color_obj.get("stops")?;
            let Some(steps_arr) = steps.as_array() else {
                return None;
            };

            parse_color(steps_arr[0].as_array()?[1].as_str()?)
        }
        _ => None,
    }
}

/// Extract a numeric value from paint properties
fn extract_number(paint: &Value, property: &str) -> Option<f64> {
    let value = paint.get(property)?;

    match value {
        Value::Array(values) => {
            if values.is_empty() {
                return None;
            }

            if value[0].as_str()? == "interpolate" && values.len() > 4 {
                // 5th value in the interpolate expression is the first actual property value
                values[4].as_f64()
            } else {
                None
            }
        }
        Value::Object(obj) => {
            let steps = obj.get("stops")?;
            let Some(steps_arr) = steps.as_array() else {
                return None;
            };

            steps_arr[0].as_array()?[1].as_f64()
        }
        Value::Number(n) => n.as_f64(),
        _ => None,
    }
}

/// Parse a CSS color string to a Galileo Color
fn parse_color(color_str: &str) -> Option<Color> {
    let color_str = color_str.trim();

    // Handle hex colors: #RGB or #RRGGBB
    if let Some(hex) = color_str.strip_prefix('#') {
        return parse_hex_color(hex);
    }

    // Handle hsl colors: hsl(h, s%, l%)
    if color_str.starts_with("hsl(") {
        return parse_hsl_color(color_str);
    }

    // Handle hsla colors: hsla(h, s%, l%, a)
    if color_str.starts_with("hsla(") {
        return parse_hsla_color(color_str);
    }

    // Handle rgb colors: rgb(r, g, b)
    if color_str.starts_with("rgb(") {
        return parse_rgb_color(color_str);
    }

    // Handle rgba colors: rgba(r, g, b, a)
    if color_str.starts_with("rgba(") {
        return parse_rgba_color(color_str);
    }

    None
}

/// Parse hex color (#RGB or #RRGGBB)
fn parse_hex_color(hex: &str) -> Option<Color> {
    let hex = hex.trim();

    if hex.len() == 3 {
        // #RGB format
        let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
        let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
        let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
        Some(Color::rgba(r, g, b, 255))
    } else if hex.len() == 6 {
        // #RRGGBB format
        let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
        Some(Color::rgba(r, g, b, 255))
    } else {
        None
    }
}

/// Parse HSL color: hsl(h, s%, l%)
fn parse_hsl_color(color_str: &str) -> Option<Color> {
    let inner = color_str.strip_prefix("hsl(")?.strip_suffix(')')?;
    let parts: Vec<&str> = inner.split(',').map(|s| s.trim()).collect();

    if parts.len() != 3 {
        return None;
    }

    let h = parts[0].parse::<f64>().ok()?;
    let s = parts[1].strip_suffix('%')?.parse::<f64>().ok()? / 100.0;
    let l = parts[2].strip_suffix('%')?.parse::<f64>().ok()? / 100.0;

    Some(hsl_to_rgb(h, s, l))
}

/// Parse HSLA color: hsla(h, s%, l%, a)
fn parse_hsla_color(color_str: &str) -> Option<Color> {
    let inner = color_str.strip_prefix("hsla(")?.strip_suffix(')')?;
    let parts: Vec<&str> = inner.split(',').map(|s| s.trim()).collect();

    if parts.len() != 4 {
        return None;
    }

    let h = parts[0].parse::<f64>().ok()?;
    let s = parts[1].strip_suffix('%')?.parse::<f64>().ok()? / 100.0;
    let l = parts[2].strip_suffix('%')?.parse::<f64>().ok()? / 100.0;
    let a = parts[3].parse::<f64>().ok()?;

    let rgb = hsl_to_rgb(h, s, l);
    Some(Color::rgba(rgb.r(), rgb.g(), rgb.b(), (a * 255.0) as u8))
}

/// Parse RGB color: rgb(r, g, b)
fn parse_rgb_color(color_str: &str) -> Option<Color> {
    let inner = color_str.strip_prefix("rgb(")?.strip_suffix(')')?;
    let parts: Vec<&str> = inner.split(',').map(|s| s.trim()).collect();

    if parts.len() != 3 {
        return None;
    }

    let r = parts[0].parse::<u8>().ok()?;
    let g = parts[1].parse::<u8>().ok()?;
    let b = parts[2].parse::<u8>().ok()?;

    Some(Color::rgba(r, g, b, 255))
}

/// Parse RGBA color: rgba(r, g, b, a)
fn parse_rgba_color(color_str: &str) -> Option<Color> {
    let inner = color_str.strip_prefix("rgba(")?.strip_suffix(')')?;
    let parts: Vec<&str> = inner.split(',').map(|s| s.trim()).collect();

    if parts.len() != 4 {
        return None;
    }

    let r = parts[0].parse::<u8>().ok()?;
    let g = parts[1].parse::<u8>().ok()?;
    let b = parts[2].parse::<u8>().ok()?;
    let a = parts[3].parse::<f64>().ok()?;

    Some(Color::rgba(r, g, b, (a * 255.0) as u8))
}

/// Convert HSL to RGB color
fn hsl_to_rgb(h: f64, s: f64, l: f64) -> Color {
    let h = h / 360.0;

    let r;
    let g;
    let b;

    if s == 0.0 {
        r = l;
        g = l;
        b = l;
    } else {
        let q = if l < 0.5 {
            l * (1.0 + s)
        } else {
            l + s - l * s
        };
        let p = 2.0 * l - q;

        r = hue_to_rgb(p, q, h + 1.0 / 3.0);
        g = hue_to_rgb(p, q, h);
        b = hue_to_rgb(p, q, h - 1.0 / 3.0);
    }

    Color::rgba(
        (r * 255.0).round() as u8,
        (g * 255.0).round() as u8,
        (b * 255.0).round() as u8,
        255,
    )
}

fn hue_to_rgb(p: f64, q: f64, mut t: f64) -> f64 {
    if t < 0.0 {
        t += 1.0;
    }
    if t > 1.0 {
        t -= 1.0;
    }
    if t < 1.0 / 6.0 {
        return p + (q - p) * 6.0 * t;
    }
    if t < 1.0 / 2.0 {
        return q;
    }
    if t < 2.0 / 3.0 {
        return p + (q - p) * (2.0 / 3.0 - t) * 6.0;
    }
    p
}

/// Apply opacity to a color
fn apply_opacity(color: Color, opacity: f64) -> Color {
    let alpha = (opacity * 255.0).round() as u8;
    Color::rgba(color.r(), color.g(), color.b(), alpha)
}

/// Extract background color from MapTiler style
fn extract_background_color(style: &Style) -> Color {
    // Look for a background layer
    for layer in &style.layers {
        if matches!(layer.layer_type, LayerType::Background) {
            if let Some(paint) = &layer.paint {
                if let Some(color) = extract_color(paint, "background-color") {
                    return color;
                }
            }
        }
    }

    // Default background color
    Color::rgba(240, 240, 240, 255)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hex_color() {
        let color = parse_color("#ff0000").unwrap();
        assert_eq!(color.r(), 255);
        assert_eq!(color.g(), 0);
        assert_eq!(color.b(), 0);
    }

    #[test]
    fn test_parse_hsl_color() {
        let color = parse_color("hsl(0, 100%, 50%)").unwrap();
        assert_eq!(color.r(), 255);
        assert_eq!(color.g(), 0);
        assert_eq!(color.b(), 0);
    }

    #[test]
    fn test_parse_rgb_color() {
        let color = parse_color("rgb(255, 128, 0)").unwrap();
        assert_eq!(color.r(), 255);
        assert_eq!(color.g(), 128);
        assert_eq!(color.b(), 0);
    }

    #[test]
    fn test_convert_maptiler_style() {
        // Read the test maptiler.json file
        let json_content = std::fs::read_to_string("src/maptiler_style/tests/maptiler.json")
            .expect("Failed to read maptiler.json");

        let maptiler_style: Style =
            serde_json::from_str(&json_content).expect("Failed to parse maptiler.json");

        // Convert to Galileo style
        let galileo_style = convert_maptiler_to_galileo(&maptiler_style);

        // Verify we have some rules
        assert!(
            !galileo_style.rules.is_empty(),
            "Should have converted some rules"
        );

        println!(
            "Converted {} layers to {} rules",
            maptiler_style.layers.len(),
            galileo_style.rules.len()
        );

        // Verify symbol types
        let has_polygon = galileo_style
            .rules
            .iter()
            .any(|r| matches!(r.symbol, VectorTileSymbol::Polygon(_)));
        let has_line = galileo_style
            .rules
            .iter()
            .any(|r| matches!(r.symbol, VectorTileSymbol::Line(_)));

        assert!(has_polygon, "Should have polygon symbols");
        assert!(has_line, "Should have line symbols");

        let background = parse_color("hsl(47,79%,94%)").unwrap();
        assert_eq!(galileo_style.background, background);
    }
}

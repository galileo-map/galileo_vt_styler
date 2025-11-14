use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub mod converter;

pub use converter::convert_maptiler_to_galileo;

/// MapTiler Style root structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Style {
    pub version: u8,
    pub id: String,
    pub name: String,
    pub sources: HashMap<String, Source>,
    pub layers: Vec<Layer>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Metadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub glyphs: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sprite: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bearing: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pitch: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub center: Option<Vec<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zoom: Option<f64>,
}

/// Source definition
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Source {
    #[serde(rename = "vector")]
    Vector {
        #[serde(skip_serializing_if = "Option::is_none")]
        url: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        tiles: Option<Vec<String>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        attribution: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        minzoom: Option<u8>,
        #[serde(skip_serializing_if = "Option::is_none")]
        maxzoom: Option<u8>,
    },
    #[serde(rename = "raster")]
    Raster {
        #[serde(skip_serializing_if = "Option::is_none")]
        url: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        tiles: Option<Vec<String>>,
        #[serde(skip_serializing_if = "Option::is_none", rename = "tileSize")]
        tile_size: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        attribution: Option<String>,
    },
    #[serde(rename = "geojson")]
    GeoJSON { data: serde_json::Value },
}

/// Layer definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Layer {
    pub id: String,
    #[serde(rename = "type")]
    pub layer_type: LayerType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "source-layer")]
    pub source_layer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minzoom: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maxzoom: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layout: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paint: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

/// Layer type enumeration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LayerType {
    Fill,
    Line,
    Symbol,
    Circle,
    Heatmap,
    #[serde(rename = "fill-extrusion")]
    FillExtrusion,
    Raster,
    Hillshade,
    Background,
}

/// Metadata structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maptiler: Option<MapTilerMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "spaceColor")]
    pub space_color: Option<String>,
}

/// MapTiler-specific metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapTilerMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub copyright: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub groups: Option<Vec<LayerGroup>>,
}

/// Layer group definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerGroup {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    pub layers: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_maptiler_json() {
        let json_content = include_str!("tests/maptiler.json");

        // Parse the JSON into the Style structure
        let style: Style = serde_json::from_str(&json_content)
            .expect("Failed to parse maptiler.json into Style structure");

        // Basic assertions to verify the structure
        assert_eq!(style.version, 8, "Style version should be 8");
        assert_eq!(style.id, "streets-v2", "Style id should be 'streets-v2'");
        assert_eq!(style.name, "Streets", "Style name should be 'Streets'");

        // Verify sources exist
        assert!(!style.sources.is_empty(), "Sources should not be empty");
        assert!(
            style.sources.contains_key("maptiler_attribution"),
            "Should contain 'maptiler_attribution' source"
        );
        assert!(
            style.sources.contains_key("maptiler_planet"),
            "Should contain 'maptiler_planet' source"
        );

        // Verify layers exist
        assert!(!style.layers.is_empty(), "Layers should not be empty");

        // Find and verify the Background layer
        let background_layer = style
            .layers
            .iter()
            .find(|l| l.id == "Background")
            .expect("Should have a 'Background' layer");
        assert!(
            matches!(background_layer.layer_type, LayerType::Background),
            "Background layer should be of type Background"
        );

        // Verify metadata exists
        assert!(style.metadata.is_some(), "Metadata should exist");
        let metadata = style.metadata.as_ref().unwrap();
        assert!(
            metadata.maptiler.is_some(),
            "MapTiler metadata should exist"
        );

        // Verify glyphs and sprite URLs
        assert!(style.glyphs.is_some(), "Glyphs URL should exist");
        assert!(style.sprite.is_some(), "Sprite URL should exist");

        // Print some statistics
        println!("Successfully parsed MapTiler style:");
        println!("  - Version: {}", style.version);
        println!("  - ID: {}", style.id);
        println!("  - Name: {}", style.name);
        println!("  - Sources: {}", style.sources.len());
        println!("  - Layers: {}", style.layers.len());

        // Verify layer type distribution
        let mut layer_types = HashMap::new();
        for layer in &style.layers {
            *layer_types
                .entry(format!("{:?}", layer.layer_type))
                .or_insert(0) += 1;
        }
        println!("  - Layer types:");
        for (layer_type, count) in layer_types {
            println!("    - {}: {}", layer_type, count);
        }
    }

    #[test]
    fn test_parse_sources() {
        let json_content = include_str!("tests/maptiler.json");

        let style: Style =
            serde_json::from_str(&json_content).expect("Failed to parse maptiler.json");

        // Check maptiler_attribution source
        let attribution_source = style
            .sources
            .get("maptiler_attribution")
            .expect("maptiler_attribution source should exist");

        match attribution_source {
            Source::Vector { attribution, .. } => {
                assert!(attribution.is_some(), "Attribution should be present");
            }
            _ => panic!("maptiler_attribution should be a Vector source"),
        }

        // Check maptiler_planet source
        let planet_source = style
            .sources
            .get("maptiler_planet")
            .expect("maptiler_planet source should exist");

        match planet_source {
            Source::Vector { url, .. } => {
                assert!(url.is_some(), "URL should be present");
                let url_str = url.as_ref().unwrap();
                assert!(
                    url_str.contains("maptiler.com"),
                    "URL should point to MapTiler"
                );
            }
            _ => panic!("maptiler_planet should be a Vector source"),
        }
    }

    #[test]
    fn test_layer_properties() {
        let json_content = include_str!("tests/maptiler.json");

        let style: Style =
            serde_json::from_str(&json_content).expect("Failed to parse maptiler.json");

        // Test a fill layer
        let meadow_layer = style
            .layers
            .iter()
            .find(|l| l.id == "Meadow")
            .expect("Should have 'Meadow' layer");

        assert!(matches!(meadow_layer.layer_type, LayerType::Fill));
        assert_eq!(meadow_layer.source.as_deref(), Some("maptiler_planet"));
        assert_eq!(
            meadow_layer.source_layer.as_deref(),
            Some("globallandcover")
        );
        assert_eq!(meadow_layer.maxzoom, Some(8));
        assert!(meadow_layer.paint.is_some());
        assert!(meadow_layer.layout.is_some());
        assert!(meadow_layer.filter.is_some());

        // Test a line layer
        let river_layer = style
            .layers
            .iter()
            .find(|l| l.id == "River")
            .expect("Should have 'River' layer");

        assert!(matches!(river_layer.layer_type, LayerType::Line));

        // Test a symbol layer
        let road_labels = style
            .layers
            .iter()
            .find(|l| l.id == "Road labels")
            .expect("Should have 'Road labels' layer");

        assert!(matches!(road_labels.layer_type, LayerType::Symbol));
    }
}

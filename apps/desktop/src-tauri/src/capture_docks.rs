//! Per-dock placement. A user position wins over automatic capture placement.
use crate::screen::Region;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Anchor { pub x: f64, pub y: f64 }
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Dock {
    pub zoom: f64,
    pub compact: bool,
    pub horizontal: bool,
    pub anchor: Option<Anchor>,
    #[serde(skip)] pub size: Option<(f64, f64)>,
    #[serde(skip)] pub drag_origin: Option<(i32, i32)>,
}
impl Default for Dock {
    fn default() -> Self { Self { zoom: 1., compact: false, horizontal: false, anchor: None, size: None, drag_origin: None } }
}
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Docks { pub tools: Dock, pub hud: Dock }
impl Docks {
    pub fn get(&self, tools: bool) -> &Dock { if tools { &self.tools } else { &self.hud } }
    pub fn get_mut(&mut self, tools: bool) -> &mut Dock { if tools { &mut self.tools } else { &mut self.hud } }
    pub fn sanitize(&mut self) {
        for dock in [&mut self.tools, &mut self.hud] {
            if !dock.zoom.is_finite() { dock.zoom = 1.; }
            dock.zoom = dock.zoom.clamp(0.75, 1.5);
            dock.anchor = dock.anchor.filter(|p| p.x.is_finite() && p.y.is_finite())
                .map(|p| Anchor { x: p.x.clamp(0., 1.), y: p.y.clamp(0., 1.) });
        }
    }
}
pub fn fit(mut dock: Region, monitor: Region) -> Region {
    let gap = 4_i32;
    dock.width = dock.width.min(monitor.width.saturating_sub(8).max(1));
    dock.height = dock.height.min(monitor.height.saturating_sub(8).max(1));
    dock.x = dock.x.clamp(monitor.x + gap, (monitor.x + monitor.width as i32 - dock.width as i32 - gap).max(monitor.x + gap));
    dock.y = dock.y.clamp(monitor.y + gap, (monitor.y + monitor.height as i32 - dock.height as i32 - gap).max(monitor.y + gap));
    dock
}
pub fn placed(mut automatic: Region, monitor: Region, dock: &Dock, dpi: f64) -> Region {
    if let Some((width, height)) = dock.size {
        automatic.width = (width * dpi * dock.zoom).ceil().max(1.) as u32;
        automatic.height = (height * dpi * dock.zoom).ceil().max(1.) as u32;
    }
    automatic = fit(automatic, monitor);
    if let Some(anchor) = dock.anchor {
        automatic.x = monitor.x + 4 + ((monitor.width.saturating_sub(automatic.width + 8)) as f64 * anchor.x).round() as i32;
        automatic.y = monitor.y + 4 + ((monitor.height.saturating_sub(automatic.height + 8)) as f64 * anchor.y).round() as i32;
    }
    fit(automatic, monitor)
}
pub fn pin(dock: &mut Dock, bounds: Region, monitor: Region) {
    dock.anchor = Some(Anchor {
        x: (bounds.x - monitor.x - 4).max(0) as f64 / monitor.width.saturating_sub(bounds.width + 8).max(1) as f64,
        y: (bounds.y - monitor.y - 4).max(0) as f64 / monitor.height.saturating_sub(bounds.height + 8).max(1) as f64,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dragged_docks_stay_pinned_after_resize_and_dpi_changes() {
        for dpi in [1., 1.25, 1.5, 2.] {
            let monitor = Region { x: -1920, y: -200, width: 1920, height: 1080, ..Default::default() };
            let original = Region { x: -1300, y: 120, width: 300, height: 56, ..monitor };
            let mut dock = Dock::default(); pin(&mut dock, original, monitor);
            assert_eq!(placed(original, monitor, &dock, 1.).x, original.x);
            dock.size = Some((420., 60.)); dock.zoom = 1.5;
            let b = placed(original, monitor, &dock, dpi);
            assert!(b.x >= monitor.x && b.y >= monitor.y);
            assert!(b.x + b.width as i32 <= monitor.x + monitor.width as i32);
            assert!(b.y + b.height as i32 <= monitor.y + monitor.height as i32);
        }
    }
    #[test]
    fn invalid_saved_geometry_is_sanitized_and_docks_are_independent() {
        let mut docks = Docks::default(); docks.tools.zoom = f64::NAN;
        docks.tools.anchor = Some(Anchor { x: f64::INFINITY, y: -4. });
        docks.hud.zoom = 20.; docks.sanitize();
        assert_eq!(docks.tools.zoom, 1.); assert!(docks.tools.anchor.is_none());
        assert_eq!(docks.hud.zoom, 1.5); assert!(!docks.hud.compact);
        let saved = serde_json::to_string(&docks).unwrap();
        assert!(!saved.contains("drag_origin") && !saved.contains("size"));
    }
}

//! Pure visual-diagnostics schema and rule checks.
//!
//! This crate is deliberately free of Bevy types. Bevy adapters in the game or
//! labs convert rendered state into these serializable snapshots, then the rule
//! checks turn "a human can see this is wrong" into text findings an agent can
//! inspect.

use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: u32 = 1;
pub const DEFAULT_SIGNAL_MIN_LUMINANCE: f32 = 2.0;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct DiagnosticRun {
    pub schema_version: u32,
    pub run_id: String,
    pub scenarios: Vec<String>,
    pub snapshots: Vec<DiagnosticSnapshotSummary>,
    pub findings: Vec<DiagnosticFinding>,
}

impl DiagnosticRun {
    pub fn new(run_id: impl Into<String>, scenarios: Vec<String>) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            run_id: run_id.into(),
            scenarios,
            snapshots: Vec::new(),
            findings: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct DiagnosticSnapshotSummary {
    pub scenario: String,
    pub image_path: String,
    pub json_path: String,
    pub finding_count: usize,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct DiagnosticSnapshot {
    pub schema_version: u32,
    pub run_id: String,
    pub scenario: String,
    pub frame_index: u32,
    pub place: Option<PlaceSnapshot>,
    pub geometry: GeometrySnapshot,
    pub thresholds: Vec<ThresholdSnapshot>,
    pub lights: Vec<LightSnapshot>,
    pub materials: Vec<MaterialSnapshot>,
    pub tac_map: Option<TacMapSnapshot>,
    pub monitors: Vec<MonitorSnapshot>,
    pub findings: Vec<DiagnosticFinding>,
}

impl DiagnosticSnapshot {
    pub fn new(run_id: impl Into<String>, scenario: impl Into<String>, frame_index: u32) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            run_id: run_id.into(),
            scenario: scenario.into(),
            frame_index,
            place: None,
            geometry: GeometrySnapshot::default(),
            thresholds: Vec::new(),
            lights: Vec::new(),
            materials: Vec::new(),
            tac_map: None,
            monitors: Vec::new(),
            findings: Vec::new(),
        }
    }

    pub fn run_default_checks(&mut self) {
        self.findings.clear();
        self.findings.extend(check_geometry(&self.geometry));
        self.findings.extend(check_thresholds(&self.thresholds));
        self.findings.extend(check_lights(&self.lights));
        self.findings.extend(check_materials(&self.materials));
        if let Some(tac_map) = &self.tac_map {
            self.findings.extend(check_tac_map(tac_map));
        }
        self.findings.extend(check_monitors(&self.monitors));
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct PlaceSnapshot {
    pub label: String,
    pub player_position: [f32; 3],
    pub player_yaw: f32,
    pub player_pitch: f32,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct GeometrySnapshot {
    pub footprints: Vec<FootprintSnapshot>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct FootprintSnapshot {
    pub subject: String,
    pub center: [f32; 2],
    pub half: [f32; 2],
    /// If true, this footprint is allowed to overlap others. Use this for the
    /// current place when checking preview-vs-preview collisions.
    pub allow_overlap: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ThresholdSnapshot {
    pub subject: String,
    pub label: String,
    pub status: String,
    pub target_room: u32,
    pub center: [f32; 3],
    pub width: f32,
    pub passage: bool,
    pub locked: bool,
    pub tethered: bool,
    pub frame_count: usize,
    pub leaf_count: usize,
    pub frame_light_count: usize,
    pub matching_status_light_count: usize,
    pub label_count: usize,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct LightSnapshot {
    pub subject: String,
    pub intensity: f32,
    pub range: f32,
    pub color_rgb: [f32; 3],
    pub expected_min_intensity: f32,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct MaterialSnapshot {
    pub subject: String,
    pub signal: bool,
    pub base_rgb: [f32; 3],
    pub emissive_rgb: [f32; 3],
    pub emissive_luminance: f32,
    pub min_luminance: f32,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TacMapSnapshot {
    pub visible: bool,
    pub expected_elements: usize,
    pub rendered_elements: usize,
    pub expected_rooms: usize,
    pub rendered_rooms: usize,
    pub expected_routes: usize,
    pub rendered_routes: usize,
    pub expected_keystones: usize,
    pub rendered_keystones: usize,
    pub expected_rivals: usize,
    pub rendered_rivals: usize,
    pub player_marker_count: usize,
    pub player_model: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct MonitorSnapshot {
    pub subject: String,
    pub room: u32,
    pub active: bool,
    pub visible: bool,
    pub label: String,
    pub label_segment_count: usize,
    pub base_rgb: [f32; 3],
    pub emissive_rgb: [f32; 3],
    pub emissive_luminance: f32,
    pub min_luminance: f32,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum FindingSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct DiagnosticFinding {
    pub severity: FindingSeverity,
    pub code: String,
    pub subject: String,
    pub message: String,
}

impl DiagnosticFinding {
    pub fn new(
        severity: FindingSeverity,
        code: impl Into<String>,
        subject: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity,
            code: code.into(),
            subject: subject.into(),
            message: message.into(),
        }
    }
}

pub fn luminance(rgb: [f32; 3]) -> f32 {
    0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2]
}

pub fn aabb_overlap_2d(a: &FootprintSnapshot, b: &FootprintSnapshot) -> bool {
    let dx = (a.center[0] - b.center[0]).abs();
    let dy = (a.center[1] - b.center[1]).abs();
    dx < a.half[0] + b.half[0] && dy < a.half[1] + b.half[1]
}

pub fn check_geometry(geometry: &GeometrySnapshot) -> Vec<DiagnosticFinding> {
    let mut findings = Vec::new();
    for i in 0..geometry.footprints.len() {
        for j in i + 1..geometry.footprints.len() {
            let a = &geometry.footprints[i];
            let b = &geometry.footprints[j];
            if a.allow_overlap || b.allow_overlap {
                continue;
            }
            if aabb_overlap_2d(a, b) {
                findings.push(DiagnosticFinding::new(
                    FindingSeverity::Error,
                    "geometry.footprint_overlap",
                    format!("{} <-> {}", a.subject, b.subject),
                    "footprints overlap without an explicit allowance",
                ));
            }
        }
    }
    findings
}

pub fn check_thresholds(thresholds: &[ThresholdSnapshot]) -> Vec<DiagnosticFinding> {
    let mut findings = Vec::new();
    for threshold in thresholds {
        if threshold.label.trim().is_empty() || threshold.label_count == 0 {
            findings.push(DiagnosticFinding::new(
                FindingSeverity::Warning,
                "threshold.label_missing",
                &threshold.subject,
                "threshold has no debug label",
            ));
        }
        if threshold.frame_count == 0 {
            findings.push(DiagnosticFinding::new(
                FindingSeverity::Error,
                "threshold.frame_missing",
                &threshold.subject,
                "visible threshold has no rendered frame",
            ));
        }
        if threshold.passage && threshold.leaf_count > 0 {
            findings.push(DiagnosticFinding::new(
                FindingSeverity::Error,
                "threshold.passage_has_leaf",
                &threshold.subject,
                "passage threshold is blocked by a rendered door leaf",
            ));
        }
        if !threshold.passage && threshold.leaf_count == 0 {
            findings.push(DiagnosticFinding::new(
                FindingSeverity::Warning,
                "threshold.sealed_without_leaf",
                &threshold.subject,
                "sealed or locked threshold has no visible door leaf",
            ));
        }
        if threshold.tethered && threshold.matching_status_light_count == 0 {
            findings.push(DiagnosticFinding::new(
                FindingSeverity::Error,
                "threshold.tether_light_missing",
                &threshold.subject,
                "tethered threshold has no control-colored frame light",
            ));
        }
        if threshold.frame_light_count == 0 {
            findings.push(DiagnosticFinding::new(
                FindingSeverity::Warning,
                "threshold.frame_light_missing",
                &threshold.subject,
                "threshold frame has no point light",
            ));
        }
    }
    findings
}

pub fn check_lights(lights: &[LightSnapshot]) -> Vec<DiagnosticFinding> {
    lights
        .iter()
        .filter(|light| light.intensity < light.expected_min_intensity || light.range <= 0.1)
        .map(|light| {
            DiagnosticFinding::new(
                FindingSeverity::Error,
                "light.not_emitting",
                &light.subject,
                format!(
                    "light intensity/range too low: intensity {:.2}, range {:.2}",
                    light.intensity, light.range
                ),
            )
        })
        .collect()
}

pub fn check_materials(materials: &[MaterialSnapshot]) -> Vec<DiagnosticFinding> {
    materials
        .iter()
        .filter(|material| material.signal && material.emissive_luminance < material.min_luminance)
        .map(|material| {
            DiagnosticFinding::new(
                FindingSeverity::Error,
                "material.signal_too_dim",
                &material.subject,
                format!(
                    "signal emissive luminance {:.2} is below {:.2}",
                    material.emissive_luminance, material.min_luminance
                ),
            )
        })
        .collect()
}

pub fn check_tac_map(map: &TacMapSnapshot) -> Vec<DiagnosticFinding> {
    let mut findings = Vec::new();
    if !map.visible {
        return findings;
    }
    if map.expected_elements != map.rendered_elements {
        findings.push(DiagnosticFinding::new(
            FindingSeverity::Error,
            "tacmap.element_count_mismatch",
            "tac-map",
            format!(
                "expected {} elements, rendered {}",
                map.expected_elements, map.rendered_elements
            ),
        ));
    }
    if map.rendered_rooms != map.expected_rooms {
        findings.push(DiagnosticFinding::new(
            FindingSeverity::Error,
            "tacmap.room_count_mismatch",
            "tac-map",
            format!(
                "expected {} rooms, rendered {}",
                map.expected_rooms, map.rendered_rooms
            ),
        ));
    }
    if map.rendered_routes != map.expected_routes {
        findings.push(DiagnosticFinding::new(
            FindingSeverity::Error,
            "tacmap.route_count_mismatch",
            "tac-map",
            format!(
                "expected {} route bars, rendered {}",
                map.expected_routes, map.rendered_routes
            ),
        ));
    }
    if map.rendered_keystones != map.expected_keystones {
        findings.push(DiagnosticFinding::new(
            FindingSeverity::Error,
            "tacmap.keystone_count_mismatch",
            "tac-map",
            format!(
                "expected {} keystone pips, rendered {}",
                map.expected_keystones, map.rendered_keystones
            ),
        ));
    }
    if map.rendered_rivals != map.expected_rivals {
        findings.push(DiagnosticFinding::new(
            FindingSeverity::Error,
            "tacmap.rival_count_mismatch",
            "tac-map",
            format!(
                "expected {} rival pips, rendered {}",
                map.expected_rivals, map.rendered_rivals
            ),
        ));
    }
    if map.player_marker_count != 1 {
        findings.push(DiagnosticFinding::new(
            FindingSeverity::Error,
            "tacmap.player_marker_count",
            "tac-map",
            format!(
                "expected exactly one player marker, found {}",
                map.player_marker_count
            ),
        ));
    }
    findings
}

pub fn check_monitors(monitors: &[MonitorSnapshot]) -> Vec<DiagnosticFinding> {
    let mut findings = Vec::new();
    for monitor in monitors {
        if monitor.label.trim().is_empty() {
            findings.push(DiagnosticFinding::new(
                FindingSeverity::Warning,
                "monitor.label_missing",
                &monitor.subject,
                "monitor has no room label/name",
            ));
        }
        if monitor.label_segment_count == 0 {
            findings.push(DiagnosticFinding::new(
                FindingSeverity::Warning,
                "monitor.label_geometry_missing",
                &monitor.subject,
                "monitor has no rendered room label geometry",
            ));
        }
        if monitor.active && monitor.emissive_luminance < monitor.min_luminance {
            findings.push(DiagnosticFinding::new(
                FindingSeverity::Error,
                "monitor.active_black",
                &monitor.subject,
                format!(
                    "active monitor emissive luminance {:.2} is below {:.2}",
                    monitor.emissive_luminance, monitor.min_luminance
                ),
            ));
        }
        if monitor.active && !monitor.visible {
            findings.push(DiagnosticFinding::new(
                FindingSeverity::Error,
                "monitor.active_hidden",
                &monitor.subject,
                "active monitor is hidden",
            ));
        }
    }
    findings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geometry_overlap_becomes_text_finding() {
        let geometry = GeometrySnapshot {
            footprints: vec![
                FootprintSnapshot {
                    subject: "room-a".to_string(),
                    center: [0.0, 0.0],
                    half: [2.0, 2.0],
                    allow_overlap: false,
                },
                FootprintSnapshot {
                    subject: "room-b".to_string(),
                    center: [1.0, 1.0],
                    half: [2.0, 2.0],
                    allow_overlap: false,
                },
            ],
        };
        let findings = check_geometry(&geometry);
        assert_eq!(findings[0].code, "geometry.footprint_overlap");
    }

    #[test]
    fn passage_leaf_and_missing_tether_light_fail() {
        let findings = check_thresholds(&[ThresholdSnapshot {
            subject: "R0:S1".to_string(),
            label: "R0:S1".to_string(),
            status: "tethered_passage".to_string(),
            target_room: 1,
            center: [0.0, 0.0, 0.0],
            width: 4.5,
            passage: true,
            locked: false,
            tethered: true,
            frame_count: 3,
            leaf_count: 1,
            frame_light_count: 1,
            matching_status_light_count: 0,
            label_count: 1,
        }]);
        assert!(
            findings
                .iter()
                .any(|f| f.code == "threshold.passage_has_leaf")
        );
        assert!(
            findings
                .iter()
                .any(|f| f.code == "threshold.tether_light_missing")
        );
    }

    #[test]
    fn active_black_monitor_fails() {
        let findings = check_monitors(&[MonitorSnapshot {
            subject: "guardian monitor room 4".to_string(),
            room: 4,
            active: true,
            visible: true,
            label: "Guardian Monitor Room 4".to_string(),
            label_segment_count: 8,
            base_rgb: [0.0, 0.0, 0.0],
            emissive_rgb: [0.0, 0.0, 0.0],
            emissive_luminance: 0.0,
            min_luminance: DEFAULT_SIGNAL_MIN_LUMINANCE,
        }]);
        assert_eq!(findings[0].code, "monitor.active_black");
    }

    #[test]
    fn tac_map_counts_are_checked() {
        let findings = check_tac_map(&TacMapSnapshot {
            visible: true,
            expected_elements: 20,
            rendered_elements: 19,
            expected_rooms: 9,
            rendered_rooms: 9,
            expected_routes: 8,
            rendered_routes: 8,
            expected_keystones: 3,
            rendered_keystones: 3,
            expected_rivals: 3,
            rendered_rivals: 3,
            player_marker_count: 1,
            player_model: "room 0".to_string(),
        });
        assert_eq!(findings[0].code, "tacmap.element_count_mismatch");
    }

    #[test]
    fn snapshot_runs_all_default_checks() {
        let mut snapshot = DiagnosticSnapshot::new("run", "test", 0);
        snapshot.lights.push(LightSnapshot {
            subject: "dead light".to_string(),
            intensity: 0.0,
            range: 0.0,
            color_rgb: [1.0, 1.0, 1.0],
            expected_min_intensity: 1.0,
        });
        snapshot.run_default_checks();
        assert!(
            snapshot
                .findings
                .iter()
                .any(|f| f.code == "light.not_emitting")
        );
    }
}

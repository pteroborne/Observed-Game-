use observed_hex::{HexFace, PortClass, face_edge};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{ContractDiagnostic, DiagnosticLocation, QuantizedBox, QuantizedPoint};
use crate::QUANTIZED_UNITS_PER_METER;
use crate::source::ModuleCellRef;

const INTERFACE_HASH_DOMAIN: &[u8] = b"observed2.module-interface";
const INTERFACE_HASH_VERSION: u16 = 1;
/// Common exact projection scale for the 64 m² and 65 m² quantized-hex edge
/// metrics in TrenchBroom units, and for one 128-unit level.
pub const FACE_LOCAL_SCALE: i64 = 1_064_960;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct QuantizedDirection {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FaceLocalPoint {
    pub u: i64,
    pub v: i64,
    pub inward: i64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FaceLocalDirection {
    pub u: i64,
    pub v: i64,
    pub inward: i64,
}

impl FaceLocalDirection {
    fn reduced(self) -> Result<Self, ContractDiagnostic> {
        let divisor = gcd(
            gcd(self.u.unsigned_abs(), self.v.unsigned_abs()),
            self.inward.unsigned_abs(),
        );
        if divisor == 0 {
            return Err(ContractDiagnostic::whole(
                "zero_interface_tangent",
                "interface tangent must be nonzero",
            ));
        }
        let divisor = i128::from(divisor);
        let divide = |value: i64| {
            i64::try_from(i128::from(value) / divisor).map_err(|_| {
                ContractDiagnostic::whole(
                    "interface_tangent_overflow",
                    "reduced interface tangent exceeds i64",
                )
            })
        };
        Ok(Self {
            u: divide(self.u)?,
            v: divide(self.v)?,
            inward: divide(self.inward)?,
        })
    }

    fn validate(self) -> Result<(), ContractDiagnostic> {
        if self.reduced()? != self {
            return Err(ContractDiagnostic::whole(
                "noncanonical_interface_tangent",
                format!("interface tangent {self:?} must be GCD-reduced"),
            ));
        }
        Ok(())
    }
}

fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

/// Exact face-local projection for one lateral hex edge in TrenchBroom units.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LateralFaceFrame {
    origin: [i32; 2],
    tangent: [i32; 2],
    inward: [i32; 2],
    metric_squared: i64,
    level_z: i32,
}

impl LateralFaceFrame {
    pub fn try_new(face: HexFace, level_z: i32) -> Result<Self, ContractDiagnostic> {
        if face.is_vertical() {
            return Err(ContractDiagnostic::whole(
                "lateral_frame_for_vertical_face",
                format!("{face:?} does not have a lateral edge"),
            ));
        }
        let [a, b] = face_edge(face).map(|(x, world_z)| {
            [
                x * QUANTIZED_UNITS_PER_METER,
                -world_z * QUANTIZED_UNITS_PER_METER,
            ]
        });
        let origin = [(a[0] + b[0]) / 2, (a[1] + b[1]) / 2];
        let mut tangent = [b[0] - a[0], b[1] - a[1]];
        if tangent[0] < 0 || (tangent[0] == 0 && tangent[1] < 0) {
            tangent = [-tangent[0], -tangent[1]];
        }
        let mut inward = [-tangent[1], tangent[0]];
        if dot2([-origin[0], -origin[1]], inward) < 0 {
            inward = [-inward[0], -inward[1]];
        }
        let metric_squared = dot2(tangent, tangent);
        if FACE_LOCAL_SCALE % metric_squared != 0 {
            return Err(ContractDiagnostic::whole(
                "inexact_face_metric",
                format!("face {face:?} metric {metric_squared} does not divide the scale"),
            ));
        }
        Ok(Self {
            origin,
            tangent,
            inward,
            metric_squared,
            level_z,
        })
    }

    pub fn project_point(
        self,
        point: QuantizedPoint,
    ) -> Result<FaceLocalPoint, ContractDiagnostic> {
        let plan = [
            i64::from(point.x) - i64::from(self.origin[0]),
            i64::from(point.y) - i64::from(self.origin[1]),
        ];
        Ok(FaceLocalPoint {
            u: scale_dot(plan, self.tangent, self.metric_squared)?,
            v: scale_vertical(i64::from(point.z) - i64::from(self.level_z))?,
            inward: scale_dot(plan, self.inward, self.metric_squared)?,
        })
    }

    pub fn project_direction(
        self,
        direction: QuantizedDirection,
    ) -> Result<FaceLocalDirection, ContractDiagnostic> {
        FaceLocalDirection {
            u: scale_dot(
                [i64::from(direction.x), i64::from(direction.y)],
                self.tangent,
                self.metric_squared,
            )?,
            v: scale_vertical(i64::from(direction.z))?,
            inward: scale_dot(
                [i64::from(direction.x), i64::from(direction.y)],
                self.inward,
                self.metric_squared,
            )?,
        }
        .reduced()
    }

    pub fn project_box(self, bounds: QuantizedBox) -> Result<FaceLocalBox, ContractDiagnostic> {
        project_box_corners(bounds, |point| self.project_point(point))
    }
}

/// Exact Up/Down connection frame. Plane axes retain editor X/Y; inward is
/// mirrored so equal geometry on either side of the shared threshold matches.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerticalFaceFrame {
    face: HexFace,
    boundary_z: i32,
}

impl VerticalFaceFrame {
    pub fn try_new(face: HexFace, boundary_z: i32) -> Result<Self, ContractDiagnostic> {
        if !face.is_vertical() {
            return Err(ContractDiagnostic::whole(
                "vertical_frame_for_lateral_face",
                format!("{face:?} is not vertical"),
            ));
        }
        Ok(Self { face, boundary_z })
    }

    pub fn project_point(
        self,
        point: QuantizedPoint,
    ) -> Result<FaceLocalPoint, ContractDiagnostic> {
        let inward = match self.face {
            HexFace::Up => i64::from(self.boundary_z) - i64::from(point.z),
            HexFace::Down => i64::from(point.z) - i64::from(self.boundary_z),
            _ => unreachable!("constructor accepts vertical faces only"),
        };
        Ok(FaceLocalPoint {
            u: scale_vertical(i64::from(point.x))?,
            v: scale_vertical(i64::from(point.y))?,
            inward: scale_vertical(inward)?,
        })
    }

    pub fn project_direction(
        self,
        direction: QuantizedDirection,
    ) -> Result<FaceLocalDirection, ContractDiagnostic> {
        let inward = match self.face {
            HexFace::Up => -i64::from(direction.z),
            HexFace::Down => i64::from(direction.z),
            _ => unreachable!("constructor accepts vertical faces only"),
        };
        FaceLocalDirection {
            u: scale_vertical(i64::from(direction.x))?,
            v: scale_vertical(i64::from(direction.y))?,
            inward: scale_vertical(inward)?,
        }
        .reduced()
    }

    pub fn project_box(self, bounds: QuantizedBox) -> Result<FaceLocalBox, ContractDiagnostic> {
        project_box_corners(bounds, |point| self.project_point(point))
    }
}

fn dot2(a: [i32; 2], b: [i32; 2]) -> i64 {
    i64::from(a[0]) * i64::from(b[0]) + i64::from(a[1]) * i64::from(b[1])
}

fn scale_dot(
    value: [i64; 2],
    axis: [i32; 2],
    metric_squared: i64,
) -> Result<i64, ContractDiagnostic> {
    let dot =
        i128::from(value[0]) * i128::from(axis[0]) + i128::from(value[1]) * i128::from(axis[1]);
    let scaled = dot * i128::from(FACE_LOCAL_SCALE / metric_squared);
    i64::try_from(scaled).map_err(|_| {
        ContractDiagnostic::whole(
            "face_projection_overflow",
            "face-local projection exceeds i64",
        )
    })
}

fn scale_vertical(value: i64) -> Result<i64, ContractDiagnostic> {
    let level_units = i64::from(8 * QUANTIZED_UNITS_PER_METER);
    let scaled = i128::from(value) * i128::from(FACE_LOCAL_SCALE / level_units);
    i64::try_from(scaled).map_err(|_| {
        ContractDiagnostic::whole(
            "face_projection_overflow",
            "face-local projection exceeds i64",
        )
    })
}

fn project_box_corners(
    bounds: QuantizedBox,
    project: impl Fn(QuantizedPoint) -> Result<FaceLocalPoint, ContractDiagnostic>,
) -> Result<FaceLocalBox, ContractDiagnostic> {
    bounds.validate()?;
    let mut points = Vec::with_capacity(8);
    for x in [bounds.min.x, bounds.max.x] {
        for y in [bounds.min.y, bounds.max.y] {
            for z in [bounds.min.z, bounds.max.z] {
                points.push(project(QuantizedPoint { x, y, z })?);
            }
        }
    }
    Ok(FaceLocalBox {
        min: FaceLocalPoint {
            u: points
                .iter()
                .map(|point| point.u)
                .min()
                .expect("eight corners"),
            v: points
                .iter()
                .map(|point| point.v)
                .min()
                .expect("eight corners"),
            inward: points
                .iter()
                .map(|point| point.inward)
                .min()
                .expect("eight corners"),
        },
        max: FaceLocalPoint {
            u: points
                .iter()
                .map(|point| point.u)
                .max()
                .expect("eight corners"),
            v: points
                .iter()
                .map(|point| point.v)
                .max()
                .expect("eight corners"),
            inward: points
                .iter()
                .map(|point| point.inward)
                .max()
                .expect("eight corners"),
        },
    })
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct QuantizedPose {
    pub position: FaceLocalPoint,
    /// Direction in the same normalized face-local frame.
    pub tangent: FaceLocalDirection,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct QuantizedAperture {
    pub min_u: i64,
    pub min_v: i64,
    pub max_u: i64,
    pub max_v: i64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FaceLocalBox {
    pub min: FaceLocalPoint,
    pub max: FaceLocalPoint,
}

impl FaceLocalBox {
    fn validate(self) -> Result<(), ContractDiagnostic> {
        if self.min.u >= self.max.u
            || self.min.v >= self.max.v
            || self.min.inward >= self.max.inward
        {
            return Err(ContractDiagnostic::whole(
                "invalid_interface_clearance",
                format!("face-local box requires min < max: {self:?}"),
            ));
        }
        Ok(())
    }
}

impl QuantizedAperture {
    fn validate(self) -> Result<(), ContractDiagnostic> {
        if self.min_u >= self.max_u || self.min_v >= self.max_v {
            return Err(ContractDiagnostic::whole(
                "invalid_interface_aperture",
                format!("aperture requires min < max: {self:?}"),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InterfaceProfile {
    #[serde(with = "face_serde")]
    pub face: HexFace,
    #[serde(with = "class_serde")]
    pub class: PortClass,
    pub landing: QuantizedPose,
    pub aperture: QuantizedAperture,
    /// Bounds in the normalized face-local connection frame.
    pub clearance: FaceLocalBox,
    pub guide_terminal: QuantizedPose,
}

impl InterfaceProfile {
    pub fn validate(&self) -> Result<(), ContractDiagnostic> {
        if !self.class.valid_on(self.face) {
            return Err(ContractDiagnostic::whole(
                "interface_class_invalid_on_face",
                format!("port class {:?} is invalid on {:?}", self.class, self.face),
            ));
        }
        self.landing.tangent.validate()?;
        self.guide_terminal.tangent.validate()?;
        self.aperture.validate()?;
        self.clearance.validate()
    }

    /// Hash physical compatibility in a normalized connection frame.
    /// Opposite faces intentionally share an axis tag; the compiler mirrors
    /// authored values into this frame before constructing the profile.
    #[must_use]
    pub fn fingerprint(&self) -> InterfaceFingerprint {
        let mut hash = Sha256::new();
        hash.update(INTERFACE_HASH_DOMAIN);
        hash.update(INTERFACE_HASH_VERSION.to_le_bytes());
        hash.update([u8::from(self.face.is_vertical())]);
        hash.update([class_tag(self.class)]);
        hash_pose(&mut hash, self.landing);
        for value in [
            self.aperture.min_u,
            self.aperture.min_v,
            self.aperture.max_u,
            self.aperture.max_v,
        ] {
            hash.update(value.to_le_bytes());
        }
        hash_box(&mut hash, self.clearance);
        hash_pose(&mut hash, self.guide_terminal);
        InterfaceFingerprint(hash.finalize().into())
    }
}

fn hash_pose(hash: &mut Sha256, pose: QuantizedPose) {
    for value in [
        pose.position.u,
        pose.position.v,
        pose.position.inward,
        pose.tangent.u,
        pose.tangent.v,
        pose.tangent.inward,
    ] {
        hash.update(value.to_le_bytes());
    }
}

fn hash_box(hash: &mut Sha256, bounds: FaceLocalBox) {
    for value in [
        bounds.min.u,
        bounds.min.v,
        bounds.min.inward,
        bounds.max.u,
        bounds.max.v,
        bounds.max.inward,
    ] {
        hash.update(value.to_le_bytes());
    }
}

fn class_tag(class: PortClass) -> u8 {
    match class {
        PortClass::Sealed => 0,
        PortClass::Door => 1,
        PortClass::RampOpen => 2,
        PortClass::ShaftOpen => 3,
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ModuleInterface {
    pub port: String,
    pub cell: ModuleCellRef,
    pub profile: InterfaceProfile,
}

impl ModuleInterface {
    pub fn validate(&self) -> Result<(), ContractDiagnostic> {
        if self.port.trim().is_empty() {
            return Err(ContractDiagnostic {
                code: "empty_interface_port".to_string(),
                location: DiagnosticLocation::Face(self.cell, self.profile.face),
                message: "interface port name must be nonempty".to_string(),
            });
        }
        self.profile.validate()
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct InterfaceFingerprint(pub [u8; 32]);

mod face_serde {
    use observed_hex::HexFace;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(face: &HexFace, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(match face {
            HexFace::East => "east",
            HexFace::SouthEast => "south_east",
            HexFace::SouthWest => "south_west",
            HexFace::West => "west",
            HexFace::NorthWest => "north_west",
            HexFace::NorthEast => "north_east",
            HexFace::Up => "up",
            HexFace::Down => "down",
        })
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<HexFace, D::Error> {
        let value = String::deserialize(deserializer)?;
        match value.as_str() {
            "east" => Ok(HexFace::East),
            "south_east" => Ok(HexFace::SouthEast),
            "south_west" => Ok(HexFace::SouthWest),
            "west" => Ok(HexFace::West),
            "north_west" => Ok(HexFace::NorthWest),
            "north_east" => Ok(HexFace::NorthEast),
            "up" => Ok(HexFace::Up),
            "down" => Ok(HexFace::Down),
            _ => Err(serde::de::Error::unknown_variant(
                &value,
                &[
                    "east",
                    "south_east",
                    "south_west",
                    "west",
                    "north_west",
                    "north_east",
                    "up",
                    "down",
                ],
            )),
        }
    }
}

mod class_serde {
    use observed_hex::PortClass;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(class: &PortClass, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(match class {
            PortClass::Sealed => "sealed",
            PortClass::Door => "door",
            PortClass::RampOpen => "ramp_open",
            PortClass::ShaftOpen => "shaft_open",
        })
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<PortClass, D::Error> {
        let value = String::deserialize(deserializer)?;
        match value.as_str() {
            "sealed" => Ok(PortClass::Sealed),
            "door" => Ok(PortClass::Door),
            "ramp_open" => Ok(PortClass::RampOpen),
            "shaft_open" => Ok(PortClass::ShaftOpen),
            _ => Err(serde::de::Error::unknown_variant(
                &value,
                &["sealed", "door", "ramp_open", "shaft_open"],
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn local_point(u: i64, v: i64, inward: i64) -> FaceLocalPoint {
        FaceLocalPoint { u, v, inward }
    }

    fn profile(face: HexFace) -> InterfaceProfile {
        InterfaceProfile {
            face,
            class: if face.is_vertical() {
                PortClass::ShaftOpen
            } else {
                PortClass::Door
            },
            landing: QuantizedPose {
                position: local_point(1, 2, 3),
                tangent: FaceLocalDirection {
                    u: 1,
                    v: 0,
                    inward: 0,
                },
            },
            aperture: QuantizedAperture {
                min_u: -2,
                min_v: 0,
                max_u: 2,
                max_v: 4,
            },
            clearance: FaceLocalBox {
                min: local_point(-2, 0, 0),
                max: local_point(2, 3, 4),
            },
            guide_terminal: QuantizedPose {
                position: local_point(1, 2, 3),
                tangent: FaceLocalDirection {
                    u: 1,
                    v: 0,
                    inward: 0,
                },
            },
        }
    }

    #[test]
    fn opposite_faces_match_after_connection_frame_normalization() {
        assert_eq!(
            profile(HexFace::Up).fingerprint(),
            profile(HexFace::Down).fingerprint()
        );
        assert_eq!(
            profile(HexFace::East).fingerprint(),
            profile(HexFace::West).fingerprint()
        );
        assert_ne!(
            profile(HexFace::East).fingerprint(),
            profile(HexFace::Up).fingerprint()
        );
    }

    #[test]
    fn invalid_bounds_and_tangents_fail_closed() {
        assert!(
            FaceLocalDirection {
                u: 0,
                v: 0,
                inward: 0,
            }
            .validate()
            .is_err()
        );
        assert!(
            FaceLocalDirection {
                u: 2,
                v: 0,
                inward: 0,
            }
            .validate()
            .is_err()
        );
        let mut invalid = profile(HexFace::East);
        invalid.aperture.max_u = invalid.aperture.min_u;
        assert!(invalid.validate().is_err());

        let mut hostile = profile(HexFace::East);
        hostile.landing.tangent = FaceLocalDirection {
            u: i64::MIN,
            v: 0,
            inward: 0,
        };
        let serialized = ron::to_string(&hostile).expect("serialize hostile value");
        let parsed: InterfaceProfile = ron::from_str(&serialized).expect("parse hostile value");
        assert!(
            parsed.validate().is_err(),
            "i64::MIN must diagnose rather than panic while reducing"
        );
    }

    #[test]
    fn extreme_editor_coordinates_use_checked_widened_subtraction() {
        let lateral = LateralFaceFrame::try_new(HexFace::East, i32::MIN).unwrap();
        assert!(
            lateral
                .project_point(QuantizedPoint {
                    x: 0,
                    y: 0,
                    z: i32::MAX,
                })
                .is_ok()
        );
        let vertical = VerticalFaceFrame::try_new(HexFace::Up, i32::MAX).unwrap();
        assert!(
            vertical
                .project_point(QuantizedPoint {
                    x: 0,
                    y: 0,
                    z: i32::MIN,
                })
                .is_ok()
        );
    }

    #[test]
    fn every_physical_interface_field_participates_in_the_fingerprint() {
        let base = profile(HexFace::East);
        let expected = base.fingerprint();
        let mut variants = Vec::new();

        let mut changed = base.clone();
        changed.class = PortClass::RampOpen;
        variants.push(changed);
        let mut changed = base.clone();
        changed.landing.position.u += 1;
        variants.push(changed);
        let mut changed = base.clone();
        changed.landing.tangent = FaceLocalDirection {
            u: 0,
            v: 1,
            inward: 0,
        };
        variants.push(changed);
        let mut changed = base.clone();
        changed.aperture.max_u += 1;
        variants.push(changed);
        let mut changed = base.clone();
        changed.clearance.max.inward += 1;
        variants.push(changed);
        let mut changed = base;
        changed.guide_terminal.position.v += 1;
        variants.push(changed);

        for changed in variants {
            assert_ne!(changed.fingerprint(), expected);
        }
    }

    #[test]
    fn all_lateral_opposites_project_asymmetric_tb_points_exactly() {
        for face in [HexFace::East, HexFace::SouthEast, HexFace::SouthWest] {
            let opposite = face.opposite();
            let a = LateralFaceFrame::try_new(face, 0).unwrap();
            let b = LateralFaceFrame::try_new(opposite, 0).unwrap();
            let point_in = |frame: LateralFaceFrame| QuantizedPoint {
                x: frame.origin[0] + frame.tangent[0] * 2 + frame.inward[0] * 3,
                y: frame.origin[1] + frame.tangent[1] * 2 + frame.inward[1] * 3,
                z: 37,
            };
            let point_a = a.project_point(point_in(a)).unwrap();
            let point_b = b.project_point(point_in(b)).unwrap();
            assert_eq!(point_a, point_b, "{face:?}/{opposite:?}");
            let direction_in = |frame: LateralFaceFrame| QuantizedDirection {
                x: frame.tangent[0] * 2 + frame.inward[0] * 3,
                y: frame.tangent[1] * 2 + frame.inward[1] * 3,
                z: 5,
            };
            let direction_a = a.project_direction(direction_in(a)).unwrap();
            let direction_b = b.project_direction(direction_in(b)).unwrap();
            assert_eq!(direction_a, direction_b, "{face:?}/{opposite:?} direction");

            let mut profile_a = profile(face);
            profile_a.landing = QuantizedPose {
                position: point_a,
                tangent: direction_a,
            };
            profile_a.guide_terminal = profile_a.landing;
            let mut profile_b = profile(opposite);
            profile_b.landing = QuantizedPose {
                position: point_b,
                tangent: direction_b,
            };
            profile_b.guide_terminal = profile_b.landing;
            assert_eq!(profile_a.fingerprint(), profile_b.fingerprint());
        }
    }

    #[test]
    fn up_and_down_frames_mirror_depth_at_the_shared_plane() {
        let up = VerticalFaceFrame::try_new(HexFace::Up, 128).unwrap();
        let down = VerticalFaceFrame::try_new(HexFace::Down, 0).unwrap();
        let up_point = up
            .project_point(QuantizedPoint {
                x: 17,
                y: -23,
                z: 118,
            })
            .unwrap();
        let down_point = down
            .project_point(QuantizedPoint {
                x: 17,
                y: -23,
                z: 10,
            })
            .unwrap();
        assert_eq!(up_point, down_point);

        let mut up_profile = profile(HexFace::Up);
        up_profile.landing.position = up_point;
        up_profile.guide_terminal.position = up_point;
        let mut down_profile = profile(HexFace::Down);
        down_profile.landing.position = down_point;
        down_profile.guide_terminal.position = down_point;
        assert_eq!(up_profile.fingerprint(), down_profile.fingerprint());
    }
}

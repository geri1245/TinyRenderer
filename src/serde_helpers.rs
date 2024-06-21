use glam::{Quat, Vec3};
use serde::{ser::SerializeStruct, Serializer};

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
#[serde(remote = "Vec3")]
pub struct SerdeVec3Proxy {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct QuatAsArray {
    pub values: [f32; 4],
}

impl SerdeVec3Proxy {
    pub fn from_vec3(vec: &Vec3) -> Self {
        Self {
            x: vec.x,
            y: vec.y,
            z: vec.z,
        }
    }
}

impl From<Vec3> for SerdeVec3Proxy {
    fn from(vec: Vec3) -> Self {
        Self {
            x: vec.x,
            y: vec.y,
            z: vec.z,
        }
    }
}

impl Into<Vec3> for SerdeVec3Proxy {
    fn into(self) -> Vec3 {
        Vec3 {
            x: self.x,
            y: self.y,
            z: self.z,
        }
    }
}

// #[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
// pub struct SerializableSceneComponent {
//     pub position: SerdeVec3Proxy,
//     pub scale: SerdeVec3Proxy,
//     pub rotation: [f32; 4],
// }

pub fn serialize_quat<S>(quat: &Quat, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let quat_as_array = QuatAsArray {
        values: quat.to_array(),
    };

    let mut struct_serializer = serializer.serialize_struct("QuatAsArray", 1)?;
    struct_serializer.serialize_field("values", &quat_as_array.values)?;
    struct_serializer.end()
}

// pub fn deserialize_quat<'de, D>(deserializer: D) -> Result<Quat, D::Error>
// where
//     D: Deserializer<'de>,
// {
//   deserializer.deserialize_seq(visitor)
// }

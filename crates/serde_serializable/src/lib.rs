pub trait Serializable<SerialziedData> {
    fn serialize(&self) -> SerialziedData;
    fn deserialize(data: &SerialziedData) -> Self;
}

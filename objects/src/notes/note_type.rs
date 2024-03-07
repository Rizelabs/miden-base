use crate::{
    utils::{
        format,
        serde::{ByteReader, ByteWriter, Deserializable, DeserializationError, Serializable},
    },
    Felt, NoteError,
};

// NOTE TYPE
// ================================================================================================

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum NoteType {
    /// Notes with this type have only their hash published to the network.
    OffChain,

    /// Notes with type are shared with the network encrypted.
    Encrypted,

    /// Notes with this type are fully shared with the network.
    Public,
}

impl From<NoteType> for Felt {
    fn from(id: NoteType) -> Self {
        Felt::new(id as u64)
    }
}

impl TryFrom<Felt> for NoteType {
    type Error = NoteError;

    fn try_from(value: Felt) -> Result<Self, Self::Error> {
        match value.as_int() {
            0_u64 => Ok(NoteType::OffChain),
            1_u64 => Ok(NoteType::Encrypted),
            2_u64 => Ok(NoteType::Public),
            v => Err(NoteError::NoteTypeInvalid(v)),
        }
    }
}

// SERIALIZATION
// ================================================================================================

impl Serializable for NoteType {
    fn write_into<W: ByteWriter>(&self, target: &mut W) {
        (*self as u8).write_into(target)
    }
}

impl Deserializable for NoteType {
    fn read_from<R: ByteReader>(source: &mut R) -> Result<Self, DeserializationError> {
        let discriminat = u8::read_from(source)?;

        let note_type = match discriminat {
            0_u8 => NoteType::OffChain,
            1_u8 => NoteType::Encrypted,
            2_u8 => NoteType::Public,
            v => {
                return Err(DeserializationError::InvalidValue(format!(
                    "Value {} is not a valid NoteType",
                    v
                )))
            },
        };

        Ok(note_type)
    }
}

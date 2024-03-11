use super::{
    AccountError, AccountStorageDelta, ByteReader, ByteWriter, Deserializable,
    DeserializationError, Digest, Felt, Hasher, Serializable, Word,
};
use crate::{
    crypto::merkle::{LeafIndex, NodeIndex, SimpleSmt, Smt},
    utils::{collections::*, string::*, vec},
};

mod slot;
pub use slot::StorageSlotType;

// CONSTANTS
// ================================================================================================

/// Depth of the storage tree.
pub const STORAGE_TREE_DEPTH: u8 = 8;

// TYPE ALIASES
// ================================================================================================

/// A type that represents a single storage slot item. The tuple contains the slot index of the item
/// and the entry of the item.
pub type SlotItem = (u8, StorageSlot);

/// A type that represents a single storage slot entry. The tuple contains the type of the slot and
/// the value of the slot - the value can be a raw value or a commitment to the underlying data
/// structure.
pub type StorageSlot = (StorageSlotType, Word);

// ACCOUNT STORAGE
// ================================================================================================

/// Account storage consists of 256 index-addressable storage slots.
///
/// Each slot has a type which defines the size and the structure of the slot. Currently, the
/// following types are supported:
/// - Scalar: a sequence of up to 256 words.
/// - Array: a sparse array of up to 2^n values where n > 1 and n <= 64 and each value contains up
///   to 256 words.
/// - Map: a key-value map where keys are words and values contain up to 256 words.
///
/// Storage slots are stored in a simple Sparse Merkle Tree of depth 8. Slot 255 is always reserved
/// and contains information about slot types of all other slots.
///
/// Optionally, a user can make use of storage maps. Storage maps are represented by a SMT and
/// they can hold more data as there is in plain usage of the storage slots. The root of the SMT
/// consumes one storage slot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountStorage {
    slots: SimpleSmt<STORAGE_TREE_DEPTH>,
    layout: Vec<StorageSlotType>,
    maps: Option<Vec<Smt>>,
}

impl AccountStorage {
    // CONSTANTS
    // --------------------------------------------------------------------------------------------

    /// Depth of the storage tree.
    pub const STORAGE_TREE_DEPTH: u8 = STORAGE_TREE_DEPTH;

    /// Total number of storage slots.
    pub const NUM_STORAGE_SLOTS: usize = 256;

    /// The storage slot at which the layout commitment is stored.
    pub const SLOT_LAYOUT_COMMITMENT_INDEX: u8 = 255;

    // CONSTRUCTOR
    // --------------------------------------------------------------------------------------------
    /// Returns a new instance of account storage initialized with the provided items.
    pub fn new(
        items: Vec<SlotItem>,
        maps: Option<Vec<Smt>>,
    ) -> Result<AccountStorage, AccountError> {
        // initialize storage layout
        let mut layout = vec![StorageSlotType::default(); Self::NUM_STORAGE_SLOTS];

        // set the slot type for the layout commitment
        layout[Self::SLOT_LAYOUT_COMMITMENT_INDEX as usize] =
            StorageSlotType::Value { value_arity: 64 };

        // process entries to extract type data
        let mut entires = items
            .into_iter()
            .map(|x| {
                if x.0 == Self::SLOT_LAYOUT_COMMITMENT_INDEX {
                    return Err(AccountError::StorageSlotIsReserved(x.0));
                }

                let (slot_type, slot_value) = x.1;
                layout[x.0 as usize] = slot_type;
                Ok((x.0 as u64, slot_value))
            })
            .collect::<Result<Vec<_>, AccountError>>()?;

        // add layout commitment entry
        entires.push((
            Self::SLOT_LAYOUT_COMMITMENT_INDEX as u64,
            *Hasher::hash_elements(&layout.iter().map(Felt::from).collect::<Vec<_>>()),
        ));

        // construct storage slots smt and populate the types vector.
        let slots = SimpleSmt::<STORAGE_TREE_DEPTH>::with_leaves(entires)
            .map_err(AccountError::DuplicateStorageItems)?;

        // check if the provided Vec<Smt> is longer than 255 and return an error if so
        if let Some(ref m) = maps {
            if m.len() > 255 {
                return Err(AccountError::StorageMapToManyMaps {
                    max: (Self::NUM_STORAGE_SLOTS - 1),
                    actual: (m.len()),
                });
            }
        }

        Ok(Self { slots, layout, maps })
    }

    // PUBLIC ACCESSORS
    // --------------------------------------------------------------------------------------------

    /// Returns a commitment to this storage.
    pub fn root(&self) -> Digest {
        self.slots.root()
    }

    /// Returns an item from the storage at the specified index.
    ///
    /// If the item is not present in the storage, [ZERO; 4] is returned.
    pub fn get_item(&self, index: u8) -> Digest {
        let item_index = NodeIndex::new(Self::STORAGE_TREE_DEPTH, index as u64)
            .expect("index is u8 - index within range");
        self.slots.get_node(item_index).expect("index is u8 - index within range")
    }

    /// Returns a reference to the Sparse Merkle Tree that backs the storage slots.
    pub fn slots(&self) -> &SimpleSmt<STORAGE_TREE_DEPTH> {
        &self.slots
    }

    /// Returns layout info for this storage.
    pub fn layout(&self) -> &[StorageSlotType] {
        &self.layout
    }

    /// Returns a commitment to the storage layout.
    pub fn layout_commitment(&self) -> Digest {
        Hasher::hash_elements(&self.layout.iter().map(Felt::from).collect::<Vec<_>>())
    }

    /// Returns the storage maps for this storage.
    pub fn maps(&self) -> Option<&[Smt]> {
        self.maps.as_ref().map(|v| &v[..])
    }

    // DATA MUTATORS
    // --------------------------------------------------------------------------------------------

    /// Applies the provided delta to this account storage.
    ///
    /// This method assumes that the delta has been validated by the calling method and so, no
    /// additional validation of delta is performed.
    ///
    /// # Errors
    /// Returns an error if:
    /// - The delta implies an update to a reserved account slot.
    /// - The updates violate storage layout constraints.
    pub(super) fn apply_delta(&mut self, delta: &AccountStorageDelta) -> Result<(), AccountError> {
        for &slot_idx in delta.cleared_items.iter() {
            self.set_item(slot_idx, Word::default())?;
        }

        for &(slot_idx, slot_value) in delta.updated_items.iter() {
            self.set_item(slot_idx, slot_value)?;
        }

        Ok(())
    }

    /// Sets an item from the storage at the specified index.
    ///
    /// # Errors
    /// Returns an error if:
    /// - The index specifies a reserved storage slot.
    /// - The update violates storage layout constraints.
    pub fn set_item(&mut self, index: u8, value: Word) -> Result<Word, AccountError> {
        // layout commitment slot cannot be updated
        if index == Self::SLOT_LAYOUT_COMMITMENT_INDEX {
            return Err(AccountError::StorageSlotIsReserved(index));
        }

        // only value slots of basic arity can currently be updated
        match self.layout[index as usize] {
            StorageSlotType::Value { value_arity } => {
                if value_arity > 0 {
                    return Err(AccountError::StorageSlotInvalidValueArity {
                        slot: index,
                        expected: 0,
                        actual: value_arity,
                    });
                }
            },
            slot_type => Err(AccountError::StorageSlotNotValueSlot(index, slot_type))?,
        }

        // update the slot and return
        let index = LeafIndex::new(index as u64).expect("index is u8 - index within range");
        let slot_value = self.slots.insert(index, value);
        Ok(slot_value)
    }
}

// SERIALIZATION
// ================================================================================================

impl Serializable for AccountStorage {
    fn write_into<W: ByteWriter>(&self, target: &mut W) {
        // serialize layout info; we don't serialize default type info as we'll assume that any
        // slot type that wasn't serialized was a default slot type. also we skip the last slot
        // type as it is a constant.
        let complex_types = self.layout[..255]
            .iter()
            .enumerate()
            .filter(|(_, slot_type)| !slot_type.is_default())
            .collect::<Vec<_>>();

        target.write_u8(complex_types.len() as u8);
        for (idx, slot_type) in complex_types {
            target.write_u8(idx as u8);
            target.write_u16(slot_type.into());
        }

        // serialize slot values; we serialize only non-empty values and also skip slot 255 as info
        // for this slot was already serialized as a part of serializing slot type info above
        let filled_slots = self
            .slots
            .leaves()
            .filter(|(idx, &value)| {
                // TODO: consider checking empty values for complex types as well
                value != SimpleSmt::<STORAGE_TREE_DEPTH>::EMPTY_VALUE
                    && *idx as u8 != AccountStorage::SLOT_LAYOUT_COMMITMENT_INDEX
            })
            .collect::<Vec<_>>();

        target.write_u8(filled_slots.len() as u8);
        for (idx, &value) in filled_slots {
            target.write_u8(idx as u8);
            target.write(value);
        }

        // // Serialize the optional maps field
        // match &self.maps {
        //     Some(maps) => {
        //         // Write the length of the vector to indicate how many trees we're serializing
        //         target.write_u8(maps.len() as u8);

        //         // Serialize each SparseMerkleTree in the vector
        //         for smt in maps {
        //             // Serialize the individual SparseMerkleTree here
        //             smt.write_into(target);
        //         }
        //     }
        //     None => {
        //         // Write a length of 0 to indicate that there are no trees
        //         target.write_u8(0);
        //     }
        // }
    }
}

impl Deserializable for AccountStorage {
    fn read_from<R: ByteReader>(source: &mut R) -> Result<Self, DeserializationError> {
        // read complex types
        let mut complex_types = BTreeMap::new();
        let num_complex_types = source.read_u8()?;
        for _ in 0..num_complex_types {
            let idx = source.read_u8()?;
            let slot_type: StorageSlotType =
                source.read_u16()?.try_into().map_err(DeserializationError::InvalidValue)?;
            complex_types.insert(idx, slot_type);
        }

        // read filled slots and build a vector of slot items
        let mut items: Vec<SlotItem> = Vec::new();
        let num_filled_slots = source.read_u8()?;
        for _ in 0..num_filled_slots {
            let idx = source.read_u8()?;
            let slot_value: Word = source.read()?;
            let slot_type = complex_types.remove(&idx).unwrap_or_default();
            items.push((idx, (slot_type, slot_value)));
        }

        // ToDo: add correct serialization and deserialization for SMTs
        Self::new(items, None).map_err(|err| DeserializationError::InvalidValue(err.to_string()))
    }
}

// TESTS
// ================================================================================================

#[cfg(test)]
mod tests {
    use super::{AccountStorage, Deserializable, Serializable, StorageSlotType};
    use crate::{ONE, ZERO};

    #[test]
    fn account_storage_serialization() {
        // empty storage
        let storage = AccountStorage::new(Vec::new(), None).unwrap();
        let bytes = storage.to_bytes();
        assert_eq!(storage, AccountStorage::read_from_bytes(&bytes).unwrap());

        // storage with values for default types
        let storage = AccountStorage::new(
            vec![
                (0, (StorageSlotType::default(), [ONE, ONE, ONE, ONE])),
                (2, (StorageSlotType::default(), [ONE, ONE, ONE, ZERO])),
            ],
            None,
        )
        .unwrap();
        let bytes = storage.to_bytes();
        assert_eq!(storage, AccountStorage::read_from_bytes(&bytes).unwrap());

        // storage with a mix of types
        let storage = AccountStorage::new(
            vec![
                (0, (StorageSlotType::Value { value_arity: 1 }, [ONE, ONE, ONE, ONE])),
                (1, (StorageSlotType::Value { value_arity: 0 }, [ONE, ONE, ONE, ZERO])),
                (2, (StorageSlotType::Map { value_arity: 2 }, [ONE, ONE, ZERO, ZERO])),
                (
                    3,
                    (StorageSlotType::Array { depth: 4, value_arity: 3 }, [ONE, ZERO, ZERO, ZERO]),
                ),
            ],
            None,
        )
        .unwrap();
        let bytes = storage.to_bytes();
        assert_eq!(storage, AccountStorage::read_from_bytes(&bytes).unwrap());
    }
}

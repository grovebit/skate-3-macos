//! Unpacked controller input words, base-disc 8292797C..82927AC0.
//! These are 16 full words per owner, distinct from packed voice output controls.
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WordReference {
    pub owner: u32,
    pub slot: usize,
}
impl WordReference {
    pub fn decode(id: u32) -> Option<Self> {
        if !matches!(id & 0xe0000000, 0x40000000 | 0x60000000) {
            return None;
        }
        Some(Self {
            owner: id & 0xe0fffff0,
            slot: (id & 15) as usize,
        })
    }
    pub fn key(self) -> u32 {
        self.owner | self.slot as u32
    }
}

#[derive(Default)]
pub struct WordInputs(HashMap<u32, [u32; 16]>);
impl WordInputs {
    /// Explicit owner attachment creates the native zeroed buffer. Reattaching
    /// an existing owner preserves its published words, as the resolver does.
    pub fn attach(&mut self, id: u32) -> Option<&mut [u32; 16]> {
        let reference = WordReference::decode(id)?;
        Some(self.0.entry(reference.owner).or_insert([0; 16]))
    }
    pub fn set(&mut self, id: u32, value: u32) -> bool {
        let Some(reference) = WordReference::decode(id) else {
            return false;
        };
        let Some(words) = self.0.get_mut(&reference.owner) else {
            return false;
        };
        words[reference.slot] = value;
        true
    }
    /// The owner's whole 16-word buffer, as the modulation stage reads it.
    pub fn words(&self, id: u32) -> Option<&[u32; 16]> {
        self.0.get(&WordReference::decode(id)?.owner)
    }
    /// Modulation consumes reset flags in place (8292A53C).
    pub fn words_mut(&mut self, id: u32) -> Option<&mut [u32; 16]> {
        self.0.get_mut(&WordReference::decode(id)?.owner)
    }
    /// Reset 828B7E60 zeroes a linked input buffer's 64 bytes.
    pub fn clear(&mut self, id: u32) {
        if let Some(words) = WordReference::decode(id).and_then(|r| self.0.get_mut(&r.owner)) {
            *words = [0; 16];
        }
    }
    /// Missing owners remain missing; this does not silently attach them.
    pub fn get(&self, id: u32) -> Option<u32> {
        let reference = WordReference::decode(id)?;
        Some(self.0.get(&reference.owner)?[reference.slot])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn curve_flags_share_words_but_families_instances_and_kinds_stay_separate() {
        let mut inputs = WordInputs::default();
        assert_eq!(inputs.get(0x49000025), None);
        assert!(!inputs.set(0x49000025, 123));
        inputs.attach(0x40000020).unwrap();
        assert_eq!(inputs.get(0x49000025), Some(0));
        assert!(inputs.set(0x49000025, 123));
        assert_eq!(inputs.get(0x48000025), Some(123));
        assert_eq!(inputs.attach(0x5f00002f).unwrap()[5], 123);
        for id in [0x40000825, 0x40010025, 0x60000025, 0xa0000025] {
            assert_eq!(inputs.get(id), None);
        }
        assert_eq!(
            WordReference::decode(0x49038d59),
            Some(WordReference {
                owner: 0x40038d50,
                slot: 9
            })
        );
    }
}

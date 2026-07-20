use crate::resources::ResourceKind;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ItemKind {
    Resource(ResourceKind),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ItemStack {
    pub kind: ItemKind,
    pub quantity: u32,
}

impl ItemStack {
    pub fn new(kind: ItemKind, quantity: u32) -> Self {
        Self { kind, quantity }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Error)]
pub enum InventoryError {
    #[error("inventory is full")]
    Full,
    #[error("slot is empty at index {0}")]
    EmptySlot(usize),
    #[error("item kind mismatch")]
    KindMismatch,
    #[error("would exceed max stack size of {0}")]
    StackOverflow(u32),
    #[error("not enough items: requested {requested}, available {available}")]
    InsufficientItems { requested: u32, available: u32 },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Inventory {
    pub slots: Vec<Option<ItemStack>>,
    pub capacity: usize,
}

impl Inventory {
    pub fn new(capacity: usize) -> Self {
        Self {
            slots: vec![None; capacity],
            capacity,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.slots.iter().all(|s| s.is_none())
    }

    pub fn is_full(&self) -> bool {
        self.slots.iter().all(|s| s.is_some())
    }

    pub fn count_item_kind(&self, kind: &ItemKind) -> u32 {
        self.slots
            .iter()
            .filter_map(|s| s.as_ref())
            .filter(|stack| &stack.kind == kind)
            .map(|stack| stack.quantity)
            .sum()
    }

    pub fn first_slot_with(&self, kind: &ItemKind) -> Option<usize> {
        self.slots
            .iter()
            .position(|s| s.as_ref().is_some_and(|stack| &stack.kind == kind))
    }

    pub fn add(&mut self, kind: ItemKind, quantity: u32) -> Result<u32, InventoryError> {
        let max_stack = Self::max_stack_size();
        let mut remaining = quantity;

        if let Some(idx) = self.first_slot_with(&kind) {
            let slot = &mut self.slots[idx];
            if let Some(stack) = slot {
                let room = max_stack.saturating_sub(stack.quantity);
                let to_add = remaining.min(room);
                stack.quantity += to_add;
                remaining -= to_add;
                if remaining == 0 {
                    return Ok(0);
                }
            }
        }

        while remaining > 0 {
            let empty_idx = self.slots.iter().position(|s| s.is_none());
            match empty_idx {
                None => return Err(InventoryError::Full),
                Some(idx) => {
                    let to_add = remaining.min(max_stack);
                    self.slots[idx] = Some(ItemStack::new(kind, to_add));
                    remaining -= to_add;
                }
            }
        }

        Ok(remaining)
    }

    pub fn remove(&mut self, index: usize, quantity: u32) -> Result<ItemStack, InventoryError> {
        if index >= self.slots.len() {
            return Err(InventoryError::EmptySlot(index));
        }
        let slot = self.slots[index]
            .as_mut()
            .ok_or(InventoryError::EmptySlot(index))?;
        if slot.quantity < quantity {
            return Err(InventoryError::InsufficientItems {
                requested: quantity,
                available: slot.quantity,
            });
        }
        slot.quantity -= quantity;
        let removed = ItemStack::new(slot.kind, quantity);
        if slot.quantity == 0 {
            self.slots[index] = None;
        }
        Ok(removed)
    }

    pub fn max_stack_size() -> u32 {
        crate::constants::MAX_STACK_SIZE
    }
}

impl Default for Inventory {
    fn default() -> Self {
        Self::new(crate::constants::INVENTORY_CAPACITY)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn berry() -> ItemKind {
        ItemKind::Resource(ResourceKind::Berry)
    }

    fn wood() -> ItemKind {
        ItemKind::Resource(ResourceKind::Wood)
    }

    #[test]
    fn new_inventory_has_correct_capacity() {
        let inv = Inventory::new(10);
        assert_eq!(inv.capacity, 10);
        assert_eq!(inv.slots.len(), 10);
        assert!(inv.is_empty());
    }

    #[test]
    fn new_inventory_not_full() {
        let inv = Inventory::new(24);
        assert!(!inv.is_full());
    }

    #[test]
    fn add_single_item_occupies_one_slot() {
        let mut inv = Inventory::new(10);
        inv.add(berry(), 5).unwrap();
        assert_eq!(inv.count_item_kind(&berry()), 5);
        assert!(!inv.is_empty());
    }

    #[test]
    fn add_stacks_same_kind_in_one_slot() {
        let mut inv = Inventory::new(10);
        inv.add(berry(), 30).unwrap();
        inv.add(berry(), 40).unwrap();
        let total = inv.count_item_kind(&berry());
        assert_eq!(total, 70);
        let filled: Vec<_> = inv.slots.iter().filter(|s| s.is_some()).collect();
        assert_eq!(filled.len(), 1);
    }

    #[test]
    fn add_overflow_stacks_split_across_slots() {
        let mut inv = Inventory::new(10);
        let max = Inventory::max_stack_size();
        inv.add(berry(), max + 1).unwrap();
        let total = inv.count_item_kind(&berry());
        assert_eq!(total, max + 1);
        let filled: Vec<_> = inv.slots.iter().filter(|s| s.is_some()).collect();
        assert_eq!(filled.len(), 2);
    }

    #[test]
    fn add_full_inventory_returns_error() {
        let mut inv = Inventory::new(1);
        inv.add(berry(), 1).unwrap();
        let result = inv.add(wood(), 1);
        assert!(matches!(result, Err(InventoryError::Full)));
    }

    #[test]
    fn remove_item_decrements_quantity() {
        let mut inv = Inventory::new(10);
        inv.add(berry(), 10).unwrap();
        let removed = inv.remove(0, 3).unwrap();
        assert_eq!(removed.quantity, 3);
        assert_eq!(removed.kind, berry());
        assert_eq!(inv.count_item_kind(&berry()), 7);
    }

    #[test]
    fn remove_exact_amount_empties_slot() {
        let mut inv = Inventory::new(10);
        inv.add(berry(), 5).unwrap();
        inv.remove(0, 5).unwrap();
        assert!(inv.slots[0].is_none());
        assert!(inv.is_empty());
    }

    #[test]
    fn remove_too_many_returns_error() {
        let mut inv = Inventory::new(10);
        inv.add(berry(), 3).unwrap();
        let result = inv.remove(0, 5);
        assert!(matches!(
            result,
            Err(InventoryError::InsufficientItems { .. })
        ));
    }

    #[test]
    fn remove_from_empty_slot_returns_error() {
        let mut inv = Inventory::new(10);
        let result = inv.remove(0, 1);
        assert!(matches!(result, Err(InventoryError::EmptySlot(0))));
    }

    #[test]
    fn remove_wrong_index_returns_error() {
        let mut inv = Inventory::new(5);
        let result = inv.remove(10, 1);
        assert!(result.is_err());
    }

    #[test]
    fn count_item_kind_aggregates() {
        let mut inv = Inventory::new(10);
        let max = Inventory::max_stack_size();
        inv.add(berry(), max).unwrap();
        inv.add(berry(), max).unwrap();
        assert_eq!(inv.count_item_kind(&berry()), max * 2);
    }

    #[test]
    fn default_inventory_uses_constant() {
        let inv = Inventory::default();
        assert_eq!(inv.capacity, crate::constants::INVENTORY_CAPACITY);
    }

    #[test]
    fn item_kind_serde_roundtrip() {
        let kind = berry();
        let json = serde_json::to_string(&kind).unwrap();
        let deserialized: ItemKind = serde_json::from_str(&json).unwrap();
        assert_eq!(kind, deserialized);
    }

    #[test]
    fn inventory_serde_roundtrip() {
        let mut inv = Inventory::new(4);
        inv.add(berry(), 5).unwrap();
        inv.add(wood(), 3).unwrap();
        let json = serde_json::to_string(&inv).unwrap();
        let deserialized: Inventory = serde_json::from_str(&json).unwrap();
        assert_eq!(inv, deserialized);
    }

    #[test]
    fn inventory_error_display() {
        let err = InventoryError::Full;
        assert_eq!(err.to_string(), "inventory is full");
    }
}

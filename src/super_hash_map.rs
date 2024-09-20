use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug)]
pub struct SuperHashMap<T> {
    items: Vec<T>,
    empty_spots: HashSet<usize>,
    id_to_place_in_vec: HashMap<u32, usize>,
}

impl<T> SuperHashMap<T> {
    pub fn new() -> Self {
        SuperHashMap {
            items: Vec::new(),
            empty_spots: HashSet::new(),
            id_to_place_in_vec: HashMap::new(),
        }
    }

    pub fn insert(&mut self, id: u32, value: T) {
        let mut empty_spot_used = None;
        if let Some(index) = self.empty_spots.iter().next() {
            self.items[*index] = value;
            self.id_to_place_in_vec.insert(id, *index);
            empty_spot_used = Some(*index);
        } else {
            self.items.push(value);
            self.id_to_place_in_vec.insert(id, self.items.len() - 1);
        }

        empty_spot_used.map(|index| {
            self.empty_spots.remove(&index);
        });
    }

    pub fn len(&self) -> usize {
        self.items.len() - self.empty_spots.len()
    }

    pub fn remove(&mut self, id: u32) {
        let index = self.id_to_place_in_vec.remove(&id).unwrap();
        self.empty_spots.insert(index);
    }

    fn get_index_of_item_in_vec(&self, id: u32) -> Option<usize> {
        self.id_to_place_in_vec.get(&id).map(|id| *id)
    }

    pub fn get(&self, id: u32) -> Option<&T> {
        self.get_index_of_item_in_vec(id)
            .map(|index| &self.items[index])
    }

    pub fn get_mut(&mut self, id: u32) -> Option<&mut T> {
        self.get_index_of_item_in_vec(id)
            .map(|index| &mut self.items[index])
    }
}

impl<'a, T> IntoIterator for &'a SuperHashMap<T> {
    type Item = &'a T;

    type IntoIter = SuperHashMapIterator<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        SuperHashMapIterator {
            values: &self.items,
            index: 0,
            skipped_items: &self.empty_spots,
        }
    }
}

#[derive(Debug)]
pub struct SuperHashMapIterator<'a, T: 'a> {
    values: &'a Vec<T>,
    index: usize,
    skipped_items: &'a HashSet<usize>,
}

impl<'a, T> Clone for SuperHashMapIterator<'a, T> {
    fn clone(&self) -> Self {
        Self {
            values: &self.values,
            index: self.index,
            skipped_items: &self.skipped_items,
        }
    }
}

impl<'a, T> Iterator for SuperHashMapIterator<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        while self.index < self.values.len() && self.skipped_items.contains(&self.index) {
            self.index += 1;
        }

        if self.index < self.values.len() {
            let ret_val = &self.values[self.index];
            self.index += 1;
            Some(ret_val)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adding_items() {
        let original = vec![4, 6, 2, 9];
        let mut map = SuperHashMap::new();
        for value in &original {
            map.insert(*value, *value);
        }

        let expected_element_count = original.len();
        let mut actual_element_count = 0;

        for item in map.into_iter() {
            actual_element_count += 1;
            assert!(original.contains(item));
        }
        assert_eq!(expected_element_count, actual_element_count);
    }

    #[test]
    fn removing_items() {
        let mut map = SuperHashMap::new();
        map.insert(4, 12);
        map.insert(12, 453);
        map.remove(4);

        assert_eq!(map.len(), 1);
        let mut iter = map.into_iter();
        assert_eq!(iter.next(), Some(&453));
        assert_eq!(iter.next(), None);
    }
}

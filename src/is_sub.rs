pub trait IsSub {
    fn is_sub(&self, second: &Self) -> bool;
}

impl<T: PartialEq> IsSub for Vec<T> {
    fn is_sub(&self, second: &Self) -> bool {
        if second.is_empty() {
            return false;
        }
        let mut index: usize = 0;
        for element in self {
            if &second[index] == element {
                index += 1;
            } else {
                index = 0;
            }
            if second.len() == index {
                return true;
            }
        }
        false
    }
}

pub type Index = usize;

#[derive(Clone, Debug)]
pub struct Arena<T> {
    blocks: Vec<T>,
    free: Vec<Index>,
}

impl<T> Arena<T> {
    pub fn new(capacity: usize) -> Self {
        Arena {
            blocks: Vec::with_capacity(capacity),
            free: Vec::with_capacity(capacity),
        }
    }

    pub fn alloc(&mut self, value: T) -> Index {
        if let Some(index) = self.free.pop() {
            self.blocks[index] = value;
            return index;
        }
        self.blocks.push(value);
        self.blocks.len() - 1
    }

    pub fn get(&self, index: Index) -> &T {
        &self.blocks[index]
    }

    pub fn get_mut(&mut self, index: Index) -> &mut T {
        &mut self.blocks[index]
    }

    pub fn free(&mut self, index: Index) {
        self.free.push(index)
    }
}

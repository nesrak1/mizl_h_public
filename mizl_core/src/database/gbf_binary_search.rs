pub enum BinarySearchMatch {
    Found(i32),   // the index where the item was found
    Missing(i32), // the index where the item could be inserted
}

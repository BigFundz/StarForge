pub fn is_trending(downloads: u64, rating: f32) -> bool {
    downloads > 500 && rating >= 4.5
}

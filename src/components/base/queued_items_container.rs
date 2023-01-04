#[async_trait]
pub trait QueuedItemsContainer {
    type Item;
    async fn next(&self) -> Option<Self::Item>;
}

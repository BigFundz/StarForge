use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct MarketplaceHealth {
    pub total_templates: usize,
    pub spam_count: usize,
    pub average_quality_score: f32,
}

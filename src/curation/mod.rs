pub mod categories;
pub mod health;
pub mod quality;
pub mod spam;
pub mod trends;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TemplateMetadata {
    pub id: String,
    pub name: String,
    pub description: String,
    pub download_count: u64,
    pub rating: f32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CurationReport {
    pub quality_score: u8,
    pub is_spam: bool,
    pub is_trending: bool,
    pub category: String,
}

pub struct CurationEngine;

impl CurationEngine {
    pub fn evaluate(template: &TemplateMetadata) -> CurationReport {
        let is_spam = spam::check_spam(&template.description);
        let quality_score = quality::calculate_score(template);
        let is_trending = trends::is_trending(template.download_count, template.rating);
        let category = categories::assign_category(&template.name, &template.description);

        CurationReport {
            quality_score,
            is_spam,
            is_trending,
            category,
        }
    }
}

use super::TemplateMetadata;

pub fn calculate_score(template: &TemplateMetadata) -> u8 {
    let mut score: u8 = 0;
    if template.description.len() > 50 {
        score += 30;
    }
    if template.rating >= 4.0 {
        score += 40;
    }
    if template.download_count > 100 {
        score += 30;
    }
    score
}

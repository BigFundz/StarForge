pub fn assign_category(name: &str, description: &str) -> String {
    let combined = format!("{} {}", name, description).to_lowercase();
    if combined.contains("smart contract") || combined.contains("stellar") {
        "Blockchain".to_string()
    } else if combined.contains("api") || combined.contains("server") {
        "Backend".to_string()
    } else {
        "General".to_string()
    }
}

pub fn check_spam(description: &str) -> bool {
    let lowercase = description.to_lowercase();
    lowercase.contains("free money") || lowercase.contains("http://bit.ly")
}

#[test]
fn test_transfer() {
    let result = contract.transfer("alice", "bob", 100);
    assert!(result.is_ok());
}
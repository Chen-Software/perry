use perry_container_compose::types::ComposeSpec;
use perry_container_compose::compose::resolve_startup_order;
use std::fs;
use std::path::Path;

#[test]
fn test_startup_order_from_fixture() {
    let path = if Path::new("tests/fixtures/simple-two-service.yaml").exists() {
        "tests/fixtures/simple-two-service.yaml"
    } else {
        "crates/perry-container-compose/tests/fixtures/simple-two-service.yaml"
    };
    let yaml = fs::read_to_string(path).unwrap();
    let spec = ComposeSpec::parse_str(&yaml).unwrap();
    let order = resolve_startup_order(&spec).unwrap();
    assert_eq!(order, vec!["db", "web"]);
}

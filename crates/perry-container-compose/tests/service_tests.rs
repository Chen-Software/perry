use perry_container_compose::service::*;
use perry_container_compose::types::ComposeService;

// Feature: perry-container | Layer: unit | Req: 6.13 | Property: -
#[test]
fn test_service_container_name_precedence() {
    let mut svc = ComposeService::default();
    svc.container_name = Some("custom-name".into());

    // Explicit name should win
    let name = service_container_name(&svc, "web");
    assert_eq!(name, "custom-name");

    // Default should use generated name (starting with service name)
    let svc2 = ComposeService::default();
    let name2 = service_container_name(&svc2, "web");
    assert!(name2.starts_with("web_"));
    let parts: Vec<&str> = name2.split('_').collect();
    assert_eq!(parts.len(), 2);
}

use perry_container_compose::service::generate_name;
use perry_container_compose::types::ComposeService;

#[test]
fn test_generate_name_format() {
    let name = generate_name(&ComposeService { image: Some("nginx".into()), ..Default::default() }, "web");
    let parts: Vec<&str> = name.split('-').collect();
    assert_eq!(parts.len(), 3);
    assert_eq!(parts[0], "web");
}

#[test]
fn test_generate_name_stable_per_yaml() {
    let svc = ComposeService { image: Some("nginx".into()), ..Default::default() };
    let name1 = generate_name(&svc, "web");
    let name2 = generate_name(&svc, "web");
    let parts1: Vec<&str> = name1.split('-').collect();
    let parts2: Vec<&str> = name2.split('-').collect();
    assert_eq!(parts1[1], parts2[1]);
}

#[test]
fn test_generate_name_different_per_yaml() {
    let name1 = generate_name(&ComposeService { image: Some("nginx".into()), ..Default::default() }, "web1");
    let name2 = generate_name(&ComposeService { image: Some("redis".into()), ..Default::default() }, "web2");
    assert_ne!(name1, name2);
}

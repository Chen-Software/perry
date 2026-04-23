use perry_container_compose::service::Service;

#[test]
fn test_generate_name_format() {
    let name = Service::generate_name("image: nginx", "web");
    // Format: {md5_8chars}_{random_u32}
    let parts: Vec<&str> = name.split('_').collect();
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0].len(), 8);
}

#[test]
fn test_generate_name_stable_per_yaml() {
    let name1 = Service::generate_name("image: nginx", "web");
    let name2 = Service::generate_name("image: nginx", "web");
    // Prefix is md5 hash, so same input → same prefix
    assert_eq!(name1.split('_').next().unwrap(), name2.split('_').next().unwrap());
}

#[test]
fn test_generate_name_different_per_yaml() {
    let name1 = Service::generate_name("image: nginx", "web");
    let name2 = Service::generate_name("image: redis", "db");
    assert_ne!(name1.split('_').next().unwrap(), name2.split('_').next().unwrap());
}

use perry_container_compose::service::generate_name;

#[test]
fn test_generate_name_format() {
    let name = generate_name("nginx", "web");
    // Format: {service_name}_{md5_8chars}_{random_hex}
    let parts: Vec<&str> = name.split('_').collect();
    assert_eq!(parts.len(), 3);
    assert_eq!(parts[0], "web");
    assert_eq!(parts[1].len(), 8);
    assert_eq!(parts[2].len(), 8);
}

#[test]
fn test_generate_name_stable_per_image() {
    let name1 = generate_name("nginx", "web");
    let name2 = generate_name("nginx", "web");
    // md5 hash part should be same
    assert_eq!(name1.split('_').nth(1).unwrap(), name2.split('_').nth(1).unwrap());
}

#[test]
fn test_generate_name_different_per_image() {
    let name1 = generate_name("nginx", "web");
    let name2 = generate_name("redis", "web");
    assert_ne!(name1.split('_').nth(1).unwrap(), name2.split('_').nth(1).unwrap());
}

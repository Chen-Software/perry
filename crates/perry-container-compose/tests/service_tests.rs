use perry_container_compose::service::generate_name;

#[test]
fn test_generate_name_format() {
    let name = generate_name("nginx", "web");
    // Format: {safe_name}_{short_hash}{random_suffix_hex}
    assert!(name.starts_with("web_"));
    assert_eq!(name.len(), "web_".len() + 8 + 8);
}

#[test]
fn test_generate_name_stable_per_yaml() {
    let name1 = generate_name("nginx", "web");
    let name2 = generate_name("nginx", "web");
    // Prefix is safe name + md5 hash, so same input → same prefix
    assert_eq!(&name1[..name1.len()-8], &name2[..name2.len()-8]);
}

#[test]
fn test_generate_name_different_per_yaml() {
    let name1 = generate_name("nginx", "web");
    let name2 = generate_name("redis", "web");
    assert_ne!(name1, name2);
}

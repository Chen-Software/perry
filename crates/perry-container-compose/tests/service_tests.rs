use perry_container_compose::service::generate_name;

#[test]
fn test_generate_name_format() {
    let name = generate_name("nginx");
    // Format: {md5_8chars}-{random_hex}
    let parts: Vec<&str> = name.split('-').collect();
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0].len(), 8);
    assert_eq!(parts[1].len(), 8);
}

#[test]
fn test_generate_name_stable_per_image() {
    let name1 = generate_name("nginx");
    let name2 = generate_name("nginx");
    // md5 hash part should be same
    assert_eq!(name1.split('-').next().unwrap(), name2.split('-').next().unwrap());
}

#[test]
fn test_generate_name_different_per_image() {
    let name1 = generate_name("nginx");
    let name2 = generate_name("redis");
    assert_ne!(name1.split('-').next().unwrap(), name2.split('-').next().unwrap());
}

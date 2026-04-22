use perry_container_compose::types::*;
use perry_container_compose::compose::resolve_startup_order;
use perry_container_compose::error::ComposeError;

// Feature: perry-container | Layer: unit | Req: 6.4 | Property: 3
#[test]
fn test_resolve_startup_order_happy_path() {
    let mut services = indexmap::IndexMap::new();

    // a depends on b
    let mut sa = ComposeService::default();
    sa.depends_on = Some(DependsOnSpec::List(vec!["b".into()]));
    services.insert("a".into(), sa);

    // b depends on c
    let mut sb = ComposeService::default();
    sb.depends_on = Some(DependsOnSpec::List(vec!["c".into()]));
    services.insert("b".into(), sb);

    // c depends on nothing
    services.insert("c".into(), ComposeService::default());

    let spec = ComposeSpec { services, ..Default::default() };
    let order = resolve_startup_order(&spec).expect("Should resolve order");

    assert_eq!(order, vec!["c", "b", "a"]);
}

// Feature: perry-container | Layer: unit | Req: 6.5 | Property: 4
#[test]
fn test_resolve_startup_order_cycle() {
    let mut services = indexmap::IndexMap::new();

    // a depends on b
    let mut sa = ComposeService::default();
    sa.depends_on = Some(DependsOnSpec::List(vec!["b".into()]));
    services.insert("a".into(), sa);

    // b depends on a
    let mut sb = ComposeService::default();
    sb.depends_on = Some(DependsOnSpec::List(vec!["a".into()]));
    services.insert("b".into(), sb);

    let spec = ComposeSpec { services, ..Default::default() };
    let res = resolve_startup_order(&spec);

    match res {
        Err(ComposeError::DependencyCycle { services }) => {
            assert!(services.contains(&"a".into()));
            assert!(services.contains(&"b".into()));
        }
        _ => panic!("Expected DependencyCycle error"),
    }
}

// Feature: perry-container | Layer: unit | Req: 6.4 | Property: 3
#[test]
fn test_resolve_startup_order_isolated_nodes() {
    let mut services = indexmap::IndexMap::new();
    services.insert("a".into(), ComposeService::default());
    services.insert("b".into(), ComposeService::default());

    let spec = ComposeSpec { services, ..Default::default() };
    let order = resolve_startup_order(&spec).expect("Should resolve order");

    // Alphabetical for determinism
    assert_eq!(order, vec!["a", "b"]);
}

use perry_container_compose as compose;
use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::test]
async fn test_startup_order_alphabetical_tie_break() {
    let mut services = indexmap::IndexMap::new();
    services.insert("b".to_string(), compose::ComposeService {
        image: Some("img".to_string()),
        build: None, command: None, entrypoint: None, container_name: None,
        environment: None, ports: None, volumes: None, networks: Some(vec![]),
        depends_on: None, extensions: indexmap::IndexMap::new(),
    });
    services.insert("a".to_string(), compose::ComposeService {
        image: Some("img".to_string()),
        build: None, command: None, entrypoint: None, container_name: None,
        environment: None, ports: None, volumes: None, networks: Some(vec![]),
        depends_on: None, extensions: indexmap::IndexMap::new(),
    });

    let spec = compose::ComposeSpec {
        name: Some("test".to_string()),
        version: None,
        services,
        networks: None, volumes: None, secrets: None, configs: None,
        extensions: indexmap::IndexMap::new(),
    };

    let engine = compose::ComposeEngine::new(
        Arc::new(compose::CliBackend { protocol: compose::DockerProtocol }),
        spec,
        None,
    );

    let order = engine.resolve_startup_order().expect("Should resolve");
    assert_eq!(order, vec!["a", "b"]);
}

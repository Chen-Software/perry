// Feature: perry-container | Layer: unit | Req: 6.1 | Property: -

#[cfg(test)]
mod tests {
    use perry_stdlib::container::compose::*;
    use perry_container_compose::types::ComposeSpec;
    use perry_container_compose::backend::detect_backend;
    use std::sync::Arc;

    // Feature: perry-container | Layer: unit | Req: 6.1 | Property: -
    #[tokio::test]
    async fn test_compose_wrapper_project_name_derivation() {
        let mut spec = ComposeSpec::default();
        spec.name = Some("custom-stack".into());

        let backend_res = detect_backend().await;
        if let Ok(backend) = backend_res {
            let wrapper = ComposeWrapper::new(spec, Arc::new(backend));
            assert_eq!(wrapper.engine.project_name, "custom-stack");
        }
    }

    // Feature: perry-container | Layer: unit | Req: 11.2 | Property: -
    #[tokio::test]
    async fn test_compose_wrapper_default_name() {
        let spec = ComposeSpec::default();
        let backend_res = detect_backend().await;
        if let Ok(backend) = backend_res {
            let wrapper = ComposeWrapper::new(spec, Arc::new(backend));
            assert_eq!(wrapper.engine.project_name, "perry-stack");
        }
    }
}

/*
| Requirement | Test name | Layer |
|-------------|-----------|-------|
| 6.1         | test_compose_wrapper_project_name_derivation | unit |
| 11.2        | test_compose_wrapper_default_name | unit |
*/

// Deferred Requirements: none

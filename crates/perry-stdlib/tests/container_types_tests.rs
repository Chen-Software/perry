// Feature: perry-container | Layer: unit | Req: 2.7 | Property: -

#[cfg(test)]
mod tests {
    use perry_stdlib::container::types::*;
    use perry_container_compose::types::ContainerHandle;
    use perry_container_compose::error::ComposeError;

    #[test]
    fn test_container_handle_registry() {
        let handle = ContainerHandle { id: "123".into(), name: Some("test".into()) };
        let id = register_container_handle(handle.clone());

        let retrieved = get_container_handle_typed(id).expect("Should find handle");
        assert_eq!(retrieved.id, "123");

        let taken = take_container_handle_typed(id).expect("Should take handle");
        assert_eq!(taken.id, "123");
        assert!(get_container_handle_typed(id).is_none());
    }

    #[test]
    fn test_compose_error_to_container_error_conversion() {
        let ce = ComposeError::NotFound("lost".into());
        let err: ContainerError = ce.into();
        match err {
            ContainerError::NotFound(s) => assert_eq!(s, "lost"),
            _ => panic!("Wrong variant"),
        }

        let ce = ComposeError::ValidationError { message: "invalid".into() };
        let err: ContainerError = ce.into();
        match err {
            ContainerError::InvalidConfig(s) => assert_eq!(s, "invalid"),
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_container_error_display() {
        let err = ContainerError::VerificationFailed {
            image: "bad-img".into(),
            reason: "no sig".into()
        };
        let s = format!("{}", err);
        assert!(s.contains("bad-img"));
        assert!(s.contains("no sig"));
    }
}

/*
| Requirement | Test name | Layer |
|-------------|-----------|-------|
| 2.7         | test_container_handle_registry | unit |
| 11.1        | test_container_handle_registry | unit |
| 16.11       | test_compose_error_to_container_error_conversion | unit |
| none        | test_container_error_display | unit |
*/

// Deferred Requirements: none

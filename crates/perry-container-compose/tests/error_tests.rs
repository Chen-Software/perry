// Feature: perry-container | Layer: unit | Req: 2.6 | Property: 11

#[cfg(test)]
mod tests {
    use perry_container_compose::error::*;
    use std::io;

    #[test]
    fn test_compose_error_to_js_variants() {
        let errs = vec![
            (ComposeError::NotFound("foo".into()), 404),
            (ComposeError::BackendError { code: 125, message: "err".into() }, 125),
            (ComposeError::DependencyCycle { services: vec![] }, 422),
            (ComposeError::ValidationError { message: "bad".into() }, 400),
            (ComposeError::VerificationFailed { image: "i".into(), reason: "r".into() }, 403),
            (ComposeError::NoBackendFound { probed: vec![] }, 503),
            (ComposeError::BackendNotAvailable { name: "n".into(), reason: "r".into() }, 503),
            (ComposeError::IoError(io::Error::new(io::ErrorKind::Other, "oh no")), 500),
        ];

        for (err, expected_code) in errs {
            let js = compose_error_to_js(&err);
            let v: serde_json::Value = serde_json::from_str(&js).unwrap();
            assert_eq!(v["code"].as_i64().unwrap(), expected_code as i64, "Mismatch for {:?}", err);
            assert!(v["message"].as_str().is_some());
        }
    }

    #[test]
    fn test_error_display_messages() {
        let err = ComposeError::DependencyCycle { services: vec!["a".into(), "b".into()] };
        assert!(err.to_string().contains("Dependency cycle"));
        assert!(err.to_string().contains("a"));

        let err = ComposeError::ServiceStartupFailed { service: "web".into(), message: "fail".into() };
        assert!(err.to_string().contains("web"));
        assert!(err.to_string().contains("fail"));
    }
}

// Feature: perry-container | Layer: property | Req: 2.6 | Property: 11
use proptest::prelude::*;
use perry_container_compose::error::*;

#[cfg(test)]
const PROPTEST_CASES: u32 = 256;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(PROPTEST_CASES))]

    #[test]
    fn prop_error_propagation(code in -255i32..255i32, msg in "\\PC*") {
        let err = ComposeError::BackendError { code, message: msg.clone() };
        let js = compose_error_to_js(&err);
        let v: serde_json::Value = serde_json::from_str(&js).unwrap();

        prop_assert_eq!(v["code"].as_i64().unwrap(), code as i64);
        prop_assert!(v["message"].as_str().unwrap().contains(&msg));
    }
}

/*
| Requirement | Test name | Layer |
|-------------|-----------|-------|
| 2.6         | test_compose_error_to_js_variants | unit |
| 2.6         | prop_error_propagation | property |
| 12.2        | prop_error_propagation | property |
*/

// Deferred Requirements: none

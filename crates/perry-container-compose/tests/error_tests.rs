use perry_container_compose::error::*;

// Feature: perry-container | Layer: unit | Req: 12.2 | Property: 11
#[test]
fn test_compose_error_to_js_codes() {
    let err = ComposeError::NotFound("abc".into());
    let js = compose_error_to_js(&err);
    assert!(js.contains("\"code\":404"));
    assert!(js.contains("abc"));

    let err = ComposeError::ValidationError { message: "invalid".into() };
    let js = compose_error_to_js(&err);
    assert!(js.contains("\"code\":400"));

    let err = ComposeError::DependencyCycle { services: vec!["a".into(), "b".into()] };
    let js = compose_error_to_js(&err);
    assert!(js.contains("\"code\":422"));

    let err = ComposeError::VerificationFailed { image: "img".into(), reason: "bad sig".into() };
    let js = compose_error_to_js(&err);
    assert!(js.contains("\"code\":403"));
}

use perry_container_compose::yaml::*;
use perry_container_compose::types::*;
use std::collections::HashMap;
use proptest::prelude::*;

// Feature: perry-container | Layer: unit | Req: 7.8 | Property: 6
#[test]
fn test_interpolate_plain_dollar() {
    let env = HashMap::new();
    assert_eq!(interpolate_yaml("plain $ string", &env), "plain $ string");
}

// Feature: perry-container | Layer: unit | Req: 7.8 | Property: 6
#[test]
fn test_interpolate_dollar_dollar_escape() {
    let env = HashMap::new();
    assert_eq!(interpolate_yaml("$$", &env), "$");
}

// Feature: perry-container | Layer: unit | Req: 7.8 | Property: 6
#[test]
fn test_interpolate_simple_braces() {
    let mut env = HashMap::new();
    env.insert("VAR".into(), "val".into());
    assert_eq!(interpolate_yaml("${VAR}", &env), "val");
}

// Feature: perry-container | Layer: unit | Req: 7.8 | Property: 6
#[test]
fn test_interpolate_default_when_missing() {
    let env = HashMap::new();
    assert_eq!(interpolate_yaml("${VAR:-default}", &env), "default");
}

// Feature: perry-container | Layer: unit | Req: 7.8 | Property: 6
#[test]
fn test_interpolate_default_not_used_when_set() {
    let mut env = HashMap::new();
    env.insert("VAR".into(), "val".into());
    assert_eq!(interpolate_yaml("${VAR:-default}", &env), "val");
}

// Feature: perry-container | Layer: unit | Req: 7.8 | Property: 6
#[test]
fn test_interpolate_default_when_empty() {
    let mut env = HashMap::new();
    env.insert("VAR".into(), "".into());
    assert_eq!(interpolate_yaml("${VAR:-default}", &env), "default");
}

// Feature: perry-container | Layer: unit | Req: 7.8 | Property: 6
#[test]
fn test_interpolate_conditional_set() {
    let mut env = HashMap::new();
    env.insert("VAR".into(), "val".into());
    assert_eq!(interpolate_yaml("${VAR:+something}", &env), "something");
}

// Feature: perry-container | Layer: unit | Req: 7.8 | Property: 6
#[test]
fn test_interpolate_conditional_unset() {
    let env = HashMap::new();
    assert_eq!(interpolate_yaml("${VAR:+something}", &env), "");
}

// Feature: perry-container | Layer: unit | Req: 7.8 | Property: 6
#[test]
fn test_interpolate_unknown_var_empty() {
    let env = HashMap::new();
    assert_eq!(interpolate_yaml("${UNKNOWN}", &env), "");
}

// Feature: perry-container | Layer: unit | Req: 7.9 | Property: -
#[test]
fn test_parse_dotenv_basic() {
    let map = parse_dotenv("K=V");
    assert_eq!(map.get("K"), Some(&"V".to_string()));
}

// Feature: perry-container | Layer: unit | Req: 7.9 | Property: -
#[test]
fn test_parse_dotenv_inline_comment() {
    let map = parse_dotenv("K=V # comment");
    assert_eq!(map.get("K"), Some(&"V".to_string()));
}

// Feature: perry-container | Layer: unit | Req: 7.9 | Property: -
#[test]
fn test_parse_dotenv_double_quoted() {
    let map = parse_dotenv("K=\"V # not comment\"");
    assert_eq!(map.get("K"), Some(&"V # not comment".to_string()));
}

// Feature: perry-container | Layer: unit | Req: 7.9 | Property: -
#[test]
fn test_parse_dotenv_single_quoted() {
    let map = parse_dotenv("K='V # not comment'");
    assert_eq!(map.get("K"), Some(&"V # not comment".to_string()));
}

// Feature: perry-container | Layer: unit | Req: 7.9 | Property: -
#[test]
fn test_parse_dotenv_equals_in_value() {
    let map = parse_dotenv("K=V=V2");
    assert_eq!(map.get("K"), Some(&"V=V2".to_string()));
}

// Feature: perry-container | Layer: unit | Req: 7.1 | Property: -
#[test]
fn test_parse_compose_yaml_basic() {
    let yaml = "services:\n  web:\n    image: nginx";
    let spec = parse_compose_yaml(yaml, &HashMap::new()).unwrap();
    assert_eq!(spec.services.len(), 1);
    assert_eq!(spec.services.get("web").unwrap().image, Some("nginx".to_string()));
}

// Feature: perry-container | Layer: unit | Req: 7.8 | Property: -
#[test]
fn test_parse_compose_yaml_with_interpolation() {
    let yaml = "services:\n  web:\n    image: ${IMG:-nginx}";
    let spec = parse_compose_yaml(yaml, &HashMap::new()).unwrap();
    assert_eq!(spec.services.get("web").unwrap().image, Some("nginx".to_string()));
}

// Feature: perry-container | Layer: unit | Req: 7.11 | Property: -
#[test]
fn test_parse_compose_yaml_malformed_returns_error() {
    let yaml = "services: [malformed";
    let res = parse_compose_yaml(yaml, &HashMap::new());
    assert!(res.is_err());
}

// Feature: perry-container | Layer: unit | Req: 7.10 | Property: 7
#[test]
fn test_merge_last_writer_wins_services() {
    let mut s1 = ComposeSpec::default();
    let mut svc1 = ComposeService::default();
    svc1.image = Some("old".into());
    s1.services.insert("web".into(), svc1);

    let mut s2 = ComposeSpec::default();
    let mut svc2 = ComposeService::default();
    svc2.image = Some("new".into());
    s2.services.insert("web".into(), svc2);

    s1.merge(s2);
    assert_eq!(s1.services.get("web").unwrap().image, Some("new".into()));
}

// Feature: perry-container | Layer: unit | Req: 7.10 | Property: 7
#[test]
fn test_merge_last_writer_wins_networks() {
    let mut s1 = ComposeSpec::default();
    let mut nets1 = indexmap::IndexMap::new();
    nets1.insert("front".into(), Some(ComposeNetwork { driver: Some("bridge".into()), ..Default::default() }));
    s1.networks = Some(nets1);

    let mut s2 = ComposeSpec::default();
    let mut nets2 = indexmap::IndexMap::new();
    nets2.insert("front".into(), Some(ComposeNetwork { driver: Some("overlay".into()), ..Default::default() }));
    s2.networks = Some(nets2);

    s1.merge(s2);
    assert_eq!(s1.networks.as_ref().unwrap().get("front").unwrap().as_ref().unwrap().driver, Some("overlay".into()));
}

// Feature: perry-container | Layer: unit | Req: 9.1 | Property: -
#[test]
fn test_parse_and_merge_files_empty_returns_default() {
    let res = parse_and_merge_files(&[], &HashMap::new());
    assert!(res.is_ok());
    assert_eq!(res.unwrap().services.len(), 0);
}

// Feature: perry-container | Layer: unit | Req: 9.8 | Property: -
#[test]
fn test_parse_and_merge_files_missing_returns_error() {
    let res = parse_and_merge_files(&["nonexistent.yaml".into()], &HashMap::new());
    assert!(res.is_err());
}

prop_compose! {
    fn arb_env_map()(m in proptest::collection::hash_map("[A-Y_]+", "[a-z0-9]+", 0..10)) -> HashMap<String, String> {
        m
    }
}

proptest! {
    // Feature: perry-container | Layer: property | Req: 7.8 | Property: 6
    #[test]
    fn prop_env_interpolation(
        env in arb_env_map(),
        key in "[A-Y_]+"
    ) {
        let template = format!("${{{}}}", key);
        let result = interpolate_yaml(&template, &env);
        if let Some(val) = env.get(&key) {
            prop_assert_eq!(result, val.clone());
        } else {
            if let Ok(p_val) = std::env::var(&key) {
                prop_assert_eq!(result, p_val);
            } else {
                prop_assert_eq!(result, "");
            }
        }
    }
}

use perry_container_compose::yaml::*;
use perry_container_compose::types::*;
use std::collections::HashMap;
use proptest::prelude::*;

#[cfg(test)]
const PROPTEST_CASES: u32 = 256;

prop_compose! {
    fn arb_service_name()(s in "[a-z0-9_-]{1,30}") -> String { s }
}

prop_compose! {
    fn arb_image_ref()(
        repo in "[a-z0-9]{3,10}",
        tag in prop::option::of("[a-z0-9]{1,5}")
    ) -> String {
        if let Some(t) = tag { format!("{}:{}", repo, t) } else { repo }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(PROPTEST_CASES))]

    // Feature: perry-container | Layer: property | Req: 7.8 | Property: 6
    #[test]
    fn prop_interpolation_works(
        key in "[A-Z0-9_]{1,10}",
        val in "[a-z0-9]{1,10}"
    ) {
        let mut env = HashMap::new();
        env.insert(key.clone(), val.clone());
        let template = format!("image: ${{{}}}", key);
        let result = interpolate(&template, &env);
        assert_eq!(result, format!("image: {}", val));
    }

    #[test]
    fn prop_interpolation_default_works(
        key in "[A-Z0-9_]{1,10}",
        default_val in "[a-z0-9]{1,10}"
    ) {
        let env = HashMap::new();
        let template = format!("image: ${{{}:-{}}}", key, default_val);
        let result = interpolate(&template, &env);
        assert_eq!(result, format!("image: {}", default_val));
    }

    // Feature: perry-container | Layer: property | Req: 7.10 | Property: 7
    #[test]
    fn prop_compose_merge_last_writer_wins(
        name in arb_service_name(),
        img1 in arb_image_ref(),
        img2 in arb_image_ref()
    ) {
        let mut s1 = ComposeService::default(); s1.image = Some(img1);
        let mut s2 = ComposeService::default(); s2.image = Some(img2.clone());
        let mut spec1 = ComposeSpec::default(); spec1.services.insert(name.clone(), s1);
        let mut spec2 = ComposeSpec::default(); spec2.services.insert(name.clone(), s2);
        spec1.merge(spec2);
        assert_eq!(spec1.services[&name].image.as_ref().unwrap(), &img2);
    }
}

// Feature: perry-container | Layer: unit | Req: 7.1 | Property: 5
#[test]
fn test_parse_simple_yaml() {
    let yaml = "
services:
  web:
    image: nginx
";
    let spec = parse_compose_yaml(yaml, &HashMap::new()).expect("failed to parse");
    assert!(spec.services.contains_key("web"));
    assert_eq!(spec.services.get("web").unwrap().image, Some("nginx".to_string()));
}

/*
Coverage Table:
| Requirement | Test name | Layer |
|-------------|-----------|-------|
| 7.1         | test_parse_simple_yaml | unit |
| 7.8         | prop_interpolation_works | property |
| 7.10        | prop_compose_merge_last_writer_wins | property |

Deferred Requirements:
- none
*/

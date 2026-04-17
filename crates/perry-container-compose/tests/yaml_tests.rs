use perry_container_compose::yaml::*;
use std::collections::HashMap;
use proptest::prelude::*;

#[cfg(test)]
const PROPTEST_CASES: u32 = 256;

// --- Generators ---

prop_compose! {
    fn arb_env_map()(
        map in prop::collection::hash_map("[a-zA-Z_][a-zA-Z0-9_]{0,10}", ".*", 0..20)
    ) -> HashMap<String, String> {
        map
    }
}

prop_compose! {
    fn arb_env_template()(
        var in "[A-Z_]{1,10}",
        default in prop::option::of(".*"),
        is_plus in prop::bool::ANY
    ) -> String {
        if let Some(d) = default {
            if is_plus {
                format!("${{{}:+{}}}", var, d)
            } else {
                format!("${{{}:-{}}}", var, d)
            }
        } else {
            format!("${{{}}}", var)
        }
    }
}

// --- Unit Tests ---

// Feature: perry-container | Layer: unit | Req: 7.9 | Property: -
#[test]
fn test_parse_dotenv_logic() {
    let content = "KEY=VAL\n# Comment\nQ=\"val # hash\"\nE=\n T = S ";
    let env = parse_dotenv(content);
    assert_eq!(env["KEY"], "VAL");
    assert_eq!(env["Q"], "val # hash");
    assert_eq!(env["E"], "");
    assert_eq!(env["T"], "S");
}

// Feature: perry-container | Layer: unit | Req: 7.8 | Property: 6
#[test]
fn test_interpolate_dollar_dollar_literal() {
    let env = HashMap::new();
    let input = "price: $$100";
    let output = interpolate_yaml(input, &env);
    assert_eq!(output, "price: $100");
}

// --- Property Tests ---

proptest! {
    #![proptest_config(ProptestConfig::with_cases(PROPTEST_CASES))]

    // Feature: perry-container | Layer: property | Req: 7.8 | Property: 6
    #[test]
    fn prop_env_interpolation(template in arb_env_template(), env in arb_env_map()) {
        let output = interpolate_yaml(&template, &env);
        if !template.contains("$$") {
            assert!(!output.contains("${"), "Interpolation leaked in {}", template);
        }
    }
}

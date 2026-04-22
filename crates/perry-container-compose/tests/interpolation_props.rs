use proptest::prelude::*;
use perry_container_compose::yaml::interpolate_yaml;
use std::collections::HashMap;

proptest! {
    #[test]
    fn test_interpolation_literal(s in "[a-zA-Z0-9 ]*") {
        let env = HashMap::new();
        assert_eq!(interpolate_yaml(&s, &env), s);
    }

    #[test]
    fn test_interpolation_basic(key_suffix in "[A-Z_]+", value in "[a-zA-Z0-9 ]*") {
        let key = format!("PROPTEST_VAR_{}", key_suffix);
        let mut env = HashMap::new();
        env.insert(key.clone(), value.clone());
        let template = format!("prefix ${{{}}} suffix", key);
        let expected = format!("prefix {} suffix", value);
        assert_eq!(interpolate_yaml(&template, &env), expected);
    }

    #[test]
    fn test_interpolation_default(key_suffix in "[A-Z_]+", default in "[a-zA-Z0-9 ]*") {
        let key = format!("PROPTEST_VAR_UNSET_{}", key_suffix);
        let env = HashMap::new();
        let template = format!("prefix ${{{}:-{}}} suffix", key, default);
        let expected = format!("prefix {} suffix", default);
        assert_eq!(interpolate_yaml(&template, &env), expected);
    }
}

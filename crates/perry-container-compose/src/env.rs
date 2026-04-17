use regex::{Captures, Regex};
use std::collections::HashMap;

/// Interpolate ${VAR}, ${VAR:-default}, ${VAR:+value} in a string.
pub fn interpolate(text: &str, env: &HashMap<String, String>) -> String {
    let re = Regex::new(r"\$\{(?P<var>[A-Z0-9_]+)(?::(?P<op>[-+])(?P<val>[^}]*))?\}").unwrap();
    re.replace_all(text, |caps: &Captures| {
        let var = caps.name("var").unwrap().as_str();
        let op = caps.name("op").map(|m| m.as_str());
        let val = caps.name("val").map(|m| m.as_str()).unwrap_or("");

        match (env.get(var), op) {
            (Some(_v), Some("+")) => val.to_string(),
            (Some(v), _) => v.clone(),
            (None, Some("-")) => val.to_string(),
            (None, _) => String::new(),
        }
    })
    .into_owned()
}

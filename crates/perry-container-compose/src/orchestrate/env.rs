//! Environment variable interpolation and .env file support.
//!
//! Implements `${VARIABLE}`, `${VARIABLE:-default}`, and `${VARIABLE:+value}`
//! syntax commonly used in Docker Compose YAML files.

use std::collections::HashMap;

/// Parse a `.env` file into a key→value map.
///
/// Rules:
/// - Lines starting with `#` are comments
/// - Empty lines are skipped
/// - Format: `KEY=VALUE` or `KEY="VALUE"` or `KEY='VALUE'`
/// - Inline `#` comments after unquoted values are stripped
pub fn parse_dotenv(content: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();

    for line in content.lines() {
        let line = line.trim();

        // Skip comments and empty lines
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some((key, raw_val)) = line.split_once('=') {
            let key = key.trim().to_owned();
            let val = parse_value(raw_val.trim());
            map.insert(key, val);
        }
    }

    map
}

fn parse_value(raw: &str) -> String {
    if raw.is_empty() {
        return String::new();
    }

    // Double-quoted
    if raw.starts_with('"') && raw.ends_with('"') && raw.len() >= 2 {
        let inner = &raw[1..raw.len() - 1];
        return inner.replace("\\n", "\n").replace("\\\"", "\"");
    }

    // Single-quoted
    if raw.starts_with('\'') && raw.ends_with('\'') && raw.len() >= 2 {
        return raw[1..raw.len() - 1].to_owned();
    }

    // Strip inline comment
    let val = if let Some(pos) = raw.find(" #") {
        raw[..pos].trim().to_owned()
    } else {
        raw.to_owned()
    };

    val
}

/// Expand `${VAR}`, `${VAR:-default}`, `${VAR:+value}` in a string,
/// using the provided environment map.
///
/// Falls back to the process environment for variables not in `env`.
pub fn interpolate(input: &str, env: &HashMap<String, String>) -> String {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '$' {
            match chars.peek() {
                Some('{') => {
                    chars.next(); // consume '{'
                    let expr = read_until_close(&mut chars);
                    let expanded = expand_expr(&expr, env);
                    result.push_str(&expanded);
                }
                Some('$') => {
                    // $$ → literal $
                    chars.next();
                    result.push('$');
                }
                Some(&c) if c.is_alphanumeric() || c == '_' => {
                    // $VAR_NAME (no braces) — consume chars and expand
                    let name = read_plain_var(&mut chars, c);
                    let val = lookup(&name, env);
                    result.push_str(&val);
                }
                _ => {
                    result.push('$');
                }
            }
        } else {
            result.push(ch);
        }
    }

    result
}

fn read_until_close(chars: &mut std::iter::Peekable<std::str::Chars>) -> String {
    let mut expr = String::new();
    let mut depth = 1usize;
    for ch in chars.by_ref() {
        match ch {
            '{' => {
                depth += 1;
                expr.push(ch);
            }
            '}' => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
                expr.push(ch);
            }
            _ => expr.push(ch),
        }
    }
    expr
}

fn read_plain_var(
    chars: &mut std::iter::Peekable<std::str::Chars>,
    first: char,
) -> String {
    let mut name = String::new();
    name.push(first);
    chars.next(); // consume the first char that was only peeked
    while let Some(&c) = chars.peek() {
        if c.is_alphanumeric() || c == '_' {
            name.push(c);
            chars.next();
        } else {
            break;
        }
    }
    name
}

fn expand_expr(expr: &str, env: &HashMap<String, String>) -> String {
    // ${VAR:-default}
    if let Some(pos) = expr.find(":-") {
        let name = &expr[..pos];
        let default = &expr[pos + 2..];
        let val = lookup(name, env);
        if val.is_empty() {
            return default.to_owned();
        }
        return val;
    }

    // ${VAR:+value}
    if let Some(pos) = expr.find(":+") {
        let name = &expr[..pos];
        let value = &expr[pos + 2..];
        let val = lookup(name, env);
        if !val.is_empty() {
            return value.to_owned();
        }
        return String::new();
    }

    // ${VAR}
    lookup(expr, env)
}

fn lookup(name: &str, env: &HashMap<String, String>) -> String {
    if let Some(v) = env.get(name) {
        return v.clone();
    }
    // Fall back to process environment
    std::env::var(name).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_dotenv_basic() {
        let content = "FOO=bar\nBAZ=qux\n# comment\n\nEMPTY=";
        let map = parse_dotenv(content);
        assert_eq!(map["FOO"], "bar");
        assert_eq!(map["BAZ"], "qux");
        assert_eq!(map["EMPTY"], "");
    }

    #[test]
    fn test_parse_dotenv_quoted() {
        let content = r#"A="hello world"
B='single quoted'
C="with \"escape\""
"#;
        let map = parse_dotenv(content);
        assert_eq!(map["A"], "hello world");
        assert_eq!(map["B"], "single quoted");
        assert_eq!(map["C"], "with \"escape\"");
    }

    #[test]
    fn test_interpolate_simple() {
        let mut env = HashMap::new();
        env.insert("NAME".into(), "world".into());
        assert_eq!(interpolate("Hello ${NAME}!", &env), "Hello world!");
    }

    #[test]
    fn test_interpolate_default() {
        let env = HashMap::new();
        assert_eq!(interpolate("${MISSING:-fallback}", &env), "fallback");
    }

    #[test]
    fn test_interpolate_conditional() {
        let mut env = HashMap::new();
        env.insert("SET".into(), "yes".into());
        assert_eq!(interpolate("${SET:+value}", &env), "value");
        let empty: HashMap<String, String> = HashMap::new();
        assert_eq!(interpolate("${UNSET:+value}", &empty), "");
    }

    #[test]
    fn test_interpolate_dollar_dollar() {
        let env = HashMap::new();
        assert_eq!(interpolate("$$FOO", &env), "$FOO");
    }
}

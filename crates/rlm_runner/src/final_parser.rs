use regex::Regex;

/// Extract FINAL("...") / FINAL('...') / triple-quoted variants.
/// Matches the unofficial implementation: FINAL is "not a function", just a textual marker.
pub fn extract_final(response: &str) -> Option<String> {
    // Keep this small and deterministic; we only support the patterns the baseline uses.
    let patterns = [
        // Triple-quoted (DOTALL).
        "(?s)FINAL\\s*\\(\\s*\\\"\\\"\\\"(.*)\\\"\\\"\\\"",
        "(?s)FINAL\\s*\\(\\s*'''(.*)'''",
        // Single-line quoted.
        "FINAL\\s*\\(\\s*\\\"([^\\\"]*)\\\"",
        "FINAL\\s*\\(\\s*'([^']*)'",
    ];
    for pat in patterns {
        let re = match Regex::new(pat) {
            Ok(r) => r,
            Err(_) => continue,
        };
        if let Some(c) = re.captures(response) {
            return Some(c.get(1)?.as_str().trim().to_string());
        }
    }
    None
}

/// Extract FINAL_VAR(name) and return the variable name.
pub fn extract_final_var_name(response: &str) -> Option<String> {
    let re = Regex::new(r#"FINAL_VAR\s*\(\s*(\w+)\s*\)"#).ok()?;
    let cap = re.captures(response)?;
    Some(cap.get(1)?.as_str().to_string())
}

pub fn is_final(response: &str) -> bool {
    response.contains("FINAL(") || response.contains("FINAL_VAR(")
}

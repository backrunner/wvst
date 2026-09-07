use std::ops::Range;

/// Edit selected JSON string values while preserving npm's key order and formatting.
/// Validate the whole document first, then locate values structurally (not by regex).
pub(super) fn replace_versions(
    text: &str,
    targets: &[Vec<&str>],
    value: &str,
) -> Result<String, String> {
    let _: serde_json::Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let mut parser = Locator {
        text,
        offset: 0,
        targets,
        matches: Vec::new(),
    };
    parser.visit(&mut Vec::new())?;
    if parser.matches.len() != targets.len() {
        return Err("missing or duplicate JSON version fields".into());
    }
    let replacement = serde_json::to_string(value).map_err(|e| e.to_string())?;
    let mut output = text.to_string();
    for range in parser.matches.into_iter().rev() {
        output.replace_range(range, &replacement);
    }
    Ok(output)
}
struct Locator<'a> {
    text: &'a str,
    offset: usize,
    targets: &'a [Vec<&'a str>],
    matches: Vec<Range<usize>>,
}
impl Locator<'_> {
    fn skip_space(&mut self) {
        while self
            .text
            .as_bytes()
            .get(self.offset)
            .is_some_and(u8::is_ascii_whitespace)
        {
            self.offset += 1;
        }
    }
    fn string(&mut self) -> Result<String, String> {
        let start = self.offset;
        self.offset += 1;
        loop {
            match self.text.as_bytes().get(self.offset) {
                Some(b'\\') => self.offset += 2,
                Some(b'"') => {
                    self.offset += 1;
                    break;
                }
                Some(_) => self.offset += 1,
                None => return Err("unterminated JSON string".into()),
            }
        }
        serde_json::from_str(&self.text[start..self.offset]).map_err(|e| e.to_string())
    }
    fn visit(&mut self, path: &mut Vec<String>) -> Result<(), String> {
        self.skip_space();
        let start = self.offset;
        let matched = self.targets.iter().any(|target| path == target);
        if matched && self.text.as_bytes().get(start) != Some(&b'"') {
            return Err("JSON version must be a string".into());
        }
        match self.text.as_bytes().get(self.offset) {
            Some(b'{') => {
                self.offset += 1;
                loop {
                    self.skip_space();
                    if self.text.as_bytes().get(self.offset) == Some(&b'}') {
                        self.offset += 1;
                        break;
                    }
                    let key = self.string()?;
                    self.skip_space();
                    self.offset += 1; // colon, guaranteed by initial JSON validation
                    path.push(key);
                    self.visit(path)?;
                    path.pop();
                    self.skip_space();
                    if self.text.as_bytes().get(self.offset) == Some(&b',') {
                        self.offset += 1;
                    }
                }
            }
            Some(b'[') => {
                self.offset += 1;
                let mut index = 0;
                loop {
                    self.skip_space();
                    if self.text.as_bytes().get(self.offset) == Some(&b']') {
                        self.offset += 1;
                        break;
                    }
                    path.push(index.to_string());
                    self.visit(path)?;
                    path.pop();
                    index += 1;
                    self.skip_space();
                    if self.text.as_bytes().get(self.offset) == Some(&b',') {
                        self.offset += 1;
                    }
                }
            }
            Some(b'"') => {
                self.string()?;
            }
            Some(_) => {
                while self
                    .text
                    .as_bytes()
                    .get(self.offset)
                    .is_some_and(|c| !c.is_ascii_whitespace() && !b",]}".contains(c))
                {
                    self.offset += 1;
                }
            }
            None => return Err("missing JSON value".into()),
        }
        if matched {
            self.matches.push(start..self.offset);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preserves_order_escapes_arrays_and_other_versions() {
        let input = r#"{ "z":"a\\\"b", "version":"0.1.0", "a":[{"version":"9"}], "packages":{"x/y":{"version":"0.1.0"}} }"#;
        let changed = replace_versions(
            input,
            &[vec!["version"], vec!["packages", "x/y", "version"]],
            "0.2.0",
        )
        .unwrap();
        assert_eq!(changed, input.replace("\"0.1.0\"", "\"0.2.0\""));
    }
    #[test]
    fn rejects_missing_duplicate_and_non_string_versions() {
        for input in ["{}", r#"{"version":"1","version":"2"}"#, r#"{"version":1}"#] {
            assert!(replace_versions(input, &[vec!["version"]], "0.1.0").is_err());
        }
    }
}

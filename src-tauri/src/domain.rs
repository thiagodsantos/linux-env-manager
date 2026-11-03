use crate::models::{ImportResult, InvalidImportLine, ParsedEnv, SaveVariableInput, Variables};

pub fn valid_key(key: &str) -> bool {
    let mut characters = key.chars();
    match characters.next() {
        Some(first) if first.is_ascii_alphabetic() || first == '_' => {}
        _ => return false,
    }
    characters.all(|character| character.is_ascii_alphanumeric() || character == '_')
}

fn validate_key(key: &str) -> Result<(), String> {
    if valid_key(key) {
        Ok(())
    } else {
        Err("Keys must match [A-Za-z_][A-Za-z0-9_]*.".to_string())
    }
}

fn validate_value(value: &str) -> Result<(), String> {
    if value.contains('\0') {
        Err("Variable values cannot contain NUL characters.".to_string())
    } else {
        Ok(())
    }
}

pub(crate) fn parse_env(contents: &str) -> ParsedEnv {
    let mut parsed = ParsedEnv::default();

    for (index, raw_line) in contents.lines().enumerate() {
        let line_number = index + 1;
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let assignment = line.strip_prefix("export ").unwrap_or(line);
        let Some((raw_key, raw_value)) = assignment.split_once('=') else {
            parsed.invalid_lines.push(InvalidImportLine {
                line: line_number,
                message: "Expected KEY=value assignment.".to_string(),
            });
            continue;
        };

        let key = raw_key.trim();
        if !valid_key(key) {
            parsed.invalid_lines.push(InvalidImportLine {
                line: line_number,
                message: "Invalid variable key.".to_string(),
            });
            continue;
        }

        let value = raw_value.trim();
        let decoded = if value.starts_with('\'') || value.starts_with('"') {
            let quote = value.chars().next().unwrap_or_default();
            if value.len() < 2 || !value.ends_with(quote) {
                parsed.invalid_lines.push(InvalidImportLine {
                    line: line_number,
                    message: "Quoted values must have matching outer quotes.".to_string(),
                });
                continue;
            }
            value[quote.len_utf8()..value.len() - quote.len_utf8()].to_string()
        } else {
            if value.ends_with('\'') || value.ends_with('"') {
                parsed.invalid_lines.push(InvalidImportLine {
                    line: line_number,
                    message: "Quoted values must have matching outer quotes.".to_string(),
                });
                continue;
            }
            value.to_string()
        };

        if validate_value(&decoded).is_err() {
            parsed.invalid_lines.push(InvalidImportLine {
                line: line_number,
                message: "Variable values cannot contain NUL characters.".to_string(),
            });
            continue;
        }

        // Repeated assignments are deterministic: the final valid assignment wins.
        parsed.variables.insert(key.to_string(), decoded);
    }

    parsed
}

pub(crate) fn merge_import(
    existing: &Variables,
    incoming: &Variables,
) -> (Variables, ImportResult) {
    let mut merged = existing.clone();
    let mut result = ImportResult {
        added: Vec::new(),
        updated: Vec::new(),
    };

    for (key, value) in incoming {
        if merged.contains_key(key) {
            result.updated.push(key.clone());
        } else {
            result.added.push(key.clone());
        }
        merged.insert(key.clone(), value.clone());
    }

    (merged, result)
}

pub(crate) fn apply_save(
    variables: &mut Variables,
    input: SaveVariableInput,
) -> Result<(), String> {
    validate_key(&input.key)?;
    validate_value(&input.value)?;

    if let Some(original_key) = input.original_key {
        if !variables.contains_key(&original_key) {
            return Err(
                "The variable being edited no longer exists. Reload and try again.".to_string(),
            );
        }
        if input.key != original_key && variables.contains_key(&input.key) {
            return Err("A variable with this key already exists.".to_string());
        }
        if input.key != original_key {
            variables.remove(&original_key);
        }
    } else if variables.contains_key(&input.key) {
        return Err("A variable with this key already exists.".to_string());
    }

    variables.insert(input.key, input.value);
    Ok(())
}

pub(crate) fn apply_delete(variables: &mut Variables, key: &str) -> Result<(), String> {
    if variables.remove(key).is_none() {
        return Err("The variable no longer exists. Reload and try again.".to_string());
    }
    Ok(())
}

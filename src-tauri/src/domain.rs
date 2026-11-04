use crate::models::{
    CreateProfileInput, ImportResult, InvalidImportLine, ParsedEnv, ProfileData, Profiles,
    SaveVariableInput, Variables,
};

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

pub(crate) fn validate_profile_name(name: &str) -> Result<(), String> {
    let trimmed = name.trim();
    if trimmed.is_empty() || trimmed.len() > 48 || trimmed.chars().any(char::is_control) {
        return Err("Profile names must contain 1 to 48 printable characters.".to_string());
    }
    if trimmed.eq_ignore_ascii_case("General") {
        return Err("General is a reserved profile name.".to_string());
    }
    Ok(())
}

pub(crate) fn create_profile(
    profiles: &mut Profiles,
    input: CreateProfileInput,
) -> Result<(), String> {
    let name = input.name.trim();
    validate_profile_name(name)?;
    if profiles
        .keys()
        .any(|existing| existing.eq_ignore_ascii_case(name))
    {
        return Err("A profile with this name already exists.".to_string());
    }
    if !valid_color(&input.color) {
        return Err("Choose a valid six-digit profile color.".to_string());
    }
    if input.description.trim().len() > 240 {
        return Err("Profile descriptions must be 240 characters or fewer.".to_string());
    }
    profiles.insert(
        name.to_string(),
        ProfileData {
            description: input.description.trim().to_string(),
            color: input.color,
            variables: Variables::new(),
        },
    );
    Ok(())
}

pub(crate) fn valid_color(color: &str) -> bool {
    color.as_bytes().len() == 7
        && color.starts_with('#')
        && color.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit)
}

pub(crate) fn effective_variables(profiles: &Profiles, name: &str) -> Result<Variables, String> {
    let general = profiles
        .get("General")
        .ok_or_else(|| "The General profile is missing from the saved store.".to_string())?;
    let selected = profiles.get(name).ok_or_else(|| {
        "The selected profile no longer exists. Reload and try again.".to_string()
    })?;
    let mut effective = general.variables.clone();
    if name != "General" {
        effective.extend(selected.variables.clone());
    }
    Ok(effective)
}

pub(crate) fn save_profile_variable(
    profiles: &mut Profiles,
    input: SaveVariableInput,
) -> Result<(), String> {
    let profile_name = input.profile_name.clone();
    if !profiles.contains_key(&profile_name) {
        return Err("The selected profile no longer exists. Reload and try again.".to_string());
    }
    if profile_name == "General" {
        let general = profiles.get_mut(&profile_name).ok_or_else(|| {
            "The selected profile no longer exists. Reload and try again.".to_string()
        })?;
        return apply_save(&mut general.variables, input);
    }

    let source_key = input.original_key.as_deref().unwrap_or(&input.key);
    let inherited = profiles
        .get("General")
        .and_then(|general| general.variables.get(source_key))
        .is_some();
    let profile = profiles.get_mut(&profile_name).ok_or_else(|| {
        "The selected profile no longer exists. Reload and try again.".to_string()
    })?;
    if inherited {
        validate_key(&input.key)?;
        validate_value(&input.value)?;
        if input.key != source_key && profile.variables.contains_key(&input.key) {
            return Err("A variable with this key already exists.".to_string());
        }
        if input.key != source_key {
            profile.variables.remove(source_key);
        }
        profile.variables.insert(input.key, input.value);
        return Ok(());
    }
    apply_save(&mut profile.variables, input)
}

pub(crate) fn apply_delete(variables: &mut Variables, key: &str) -> Result<(), String> {
    if variables.remove(key).is_none() {
        return Err("The variable no longer exists. Reload and try again.".to_string());
    }
    Ok(())
}

/// Scans description text for keywords and returns an emoji-prefixed version.
/// Used only for dashboard display - never for SMS content.
pub fn emojify_description(desc: &str) -> String {
    let lower = desc.to_lowercase();
    let mut emojis = Vec::new();

    // Weather / nature
    if lower.contains("rain") || lower.contains("umbrella") {
        emojis.push("\u{2602}\u{fe0f}");
    }
    if lower.contains("weather") || lower.contains("forecast") {
        emojis.push("\u{1f324}\u{fe0f}");
    }
    if lower.contains("snow") {
        emojis.push("\u{2744}\u{fe0f}");
    }
    if lower.contains("sun") && !lower.contains("sunday") {
        emojis.push("\u{2600}\u{fe0f}");
    }

    // Health / medical
    if lower.contains("dentist") || lower.contains("teeth") || lower.contains("tooth") {
        emojis.push("\u{1f9b7}");
    }
    if lower.contains("doctor") || lower.contains("appointment") || lower.contains("medical") {
        emojis.push("\u{1fa7a}");
    }
    if lower.contains("medicine") || lower.contains("vitamin") || lower.contains("pill") {
        emojis.push("\u{1f48a}");
    }
    if lower.contains("gym") || lower.contains("workout") || lower.contains("exercise") {
        emojis.push("\u{1f4aa}");
    }

    // Tasks / errands
    if lower.contains("groceries") || lower.contains("grocery") || lower.contains("shopping") {
        emojis.push("\u{1f6d2}");
    }
    if lower.contains("laundry") || lower.contains("dry clean") {
        emojis.push("\u{1f9fa}");
    }
    if lower.contains("cook")
        || lower.contains("dinner")
        || lower.contains("lunch")
        || lower.contains("recipe")
    {
        emojis.push("\u{1f373}");
    }
    if lower.contains("clean") && !lower.contains("dry clean") {
        emojis.push("\u{1f9f9}");
    }

    // Communication
    if lower.contains("call") || lower.contains("phone") {
        emojis.push("\u{1f4de}");
    }
    if lower.contains("email") || lower.contains("mail") {
        emojis.push("\u{1f4e7}");
    }
    if lower.contains("meeting") || lower.contains("zoom") {
        emojis.push("\u{1f4f9}");
    }
    if lower.contains("message") || lower.contains("text") || lower.contains("whatsapp") {
        emojis.push("\u{1f4ac}");
    }

    // Travel / transport
    if lower.contains("flight") || lower.contains("airport") || lower.contains("fly") {
        emojis.push("\u{2708}\u{fe0f}");
    }
    if lower.contains("car") || lower.contains("drive") || lower.contains("parking") {
        emojis.push("\u{1f697}");
    }
    if lower.contains("train") || lower.contains("bus") || lower.contains("commute") {
        emojis.push("\u{1f68a}");
    }

    // Work / study
    if lower.contains("deadline") || lower.contains("due") || lower.contains("submit") {
        emojis.push("\u{23f0}");
    }
    if lower.contains("pay") || lower.contains("bill") || lower.contains("invoice") {
        emojis.push("\u{1f4b0}");
    }
    if lower.contains("birthday") || lower.contains("party") || lower.contains("celebration") {
        emojis.push("\u{1f389}");
    }
    if lower.contains("book") || lower.contains("read") || lower.contains("study") {
        emojis.push("\u{1f4da}");
    }

    // Pets / home
    if lower.contains("dog") || lower.contains("walk") || lower.contains("pet") {
        emojis.push("\u{1f436}");
    }
    if lower.contains("cat") && !lower.contains("catch") {
        emojis.push("\u{1f431}");
    }
    if lower.contains("water") || lower.contains("plant") || lower.contains("garden") {
        emojis.push("\u{1f331}");
    }

    if emojis.is_empty() {
        desc.to_string()
    } else {
        // Cap at 3 emojis to avoid clutter
        let prefix: String = emojis.into_iter().take(3).collect::<Vec<_>>().join("");
        format!("{} {}", prefix, desc)
    }
}

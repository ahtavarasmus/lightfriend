/// Message-level signal extraction for importance classification.
/// Computes cheap, no-LLM signals from message content and conversation context.

pub struct MessageSignals {
    pub length: usize,
    pub has_question: bool,
    pub has_time_reference: bool,
    pub has_action_verb: bool,
    pub emotional_intensity: u8, // 0=normal, 1=elevated, 2=high
    pub unseen_from_sender_last_hour: i32,
    pub is_reply_to_user: bool,
}

impl MessageSignals {
    pub fn extract(
        content: &str,
        recent_messages: &[crate::models::ontology_models::OntMessage],
        sender_name: &str,
        now: i32,
    ) -> Self {
        let lower = content.to_lowercase();

        let has_question = content.contains('?')
            || lower.starts_with("can you")
            || lower.starts_with("will you")
            || lower.starts_with("are you")
            || lower.starts_with("do you")
            || lower.starts_with("could you")
            || lower.starts_with("would you")
            || lower.starts_with("have you")
            || lower.starts_with("did you");

        let has_time_reference = {
            let time_words = [
                "today",
                "tomorrow",
                "tonight",
                "asap",
                "urgent",
                "now",
                "immediately",
                "right away",
                "this morning",
                "this afternoon",
                "this evening",
                "hurry",
                "soon",
            ];
            time_words.iter().any(|w| lower.contains(w)) || has_clock_time(&lower)
        };

        let has_action_verb = {
            let action_words = [
                "meet", "call", "send", "pay", "book", "confirm", "pick up", "bring", "come",
                "arrive", "leave", "schedule", "cancel", "sign", "submit", "reply", "respond",
                "check",
            ];
            action_words.iter().any(|w| lower.contains(w))
        };

        let emotional_intensity = {
            let exclamation_count = content.chars().filter(|c| *c == '!').count();
            let caps_ratio = if content.len() > 5 {
                let alpha_chars: Vec<char> =
                    content.chars().filter(|c| c.is_alphabetic()).collect();
                if alpha_chars.len() > 3 {
                    let upper = alpha_chars.iter().filter(|c| c.is_uppercase()).count();
                    upper as f32 / alpha_chars.len() as f32
                } else {
                    0.0
                }
            } else {
                0.0
            };
            let has_repeated_punct =
                content.contains("!!") || content.contains("??") || content.contains("...");

            if exclamation_count >= 3 || caps_ratio > 0.7 {
                2
            } else if exclamation_count >= 1 || has_repeated_punct || caps_ratio > 0.4 {
                1
            } else {
                0
            }
        };

        // Count unseen messages from this sender in the last hour
        // recent_messages comes in DESC order
        let one_hour_ago = now - 3600;
        let unseen_from_sender_last_hour = recent_messages
            .iter()
            .filter(|m| {
                m.sender_name == sender_name
                    && m.created_at >= one_hour_ago
                    && m.sender_name != "You"
            })
            .count() as i32;

        // Is this a reply to something the user sent?
        // Check if the most recent "You" message is newer than the most recent sender message
        // (excluding the current one being evaluated)
        let is_reply_to_user = {
            let last_user_msg = recent_messages.iter().find(|m| m.sender_name == "You");
            let last_sender_msg = recent_messages
                .iter()
                .skip(1) // skip current message
                .find(|m| m.sender_name == sender_name);

            match (last_user_msg, last_sender_msg) {
                (Some(user_msg), Some(sender_msg)) => user_msg.created_at > sender_msg.created_at,
                (Some(_), None) => true, // user sent something but sender hadn't messaged before
                _ => false,
            }
        };

        MessageSignals {
            length: content.len(),
            has_question,
            has_time_reference,
            has_action_verb,
            emotional_intensity,
            unseen_from_sender_last_hour,
            is_reply_to_user,
        }
    }

    pub fn format_for_prompt(&self) -> String {
        let mut parts = Vec::new();

        // Length signal
        if self.length < 30 {
            parts.push("short message".to_string());
        }

        if self.has_question {
            parts.push("contains a question".to_string());
        }

        if self.has_time_reference {
            parts.push("references a specific time".to_string());
        }

        if self.has_action_verb {
            parts.push("requests an action".to_string());
        }

        match self.emotional_intensity {
            2 => parts.push("high emotional intensity (caps/exclamation)".to_string()),
            1 => parts.push("elevated tone".to_string()),
            _ => {}
        }

        if self.unseen_from_sender_last_hour >= 3 {
            parts.push(format!(
                "{} messages from sender in the last hour (escalation pattern)",
                self.unseen_from_sender_last_hour
            ));
        } else if self.unseen_from_sender_last_hour == 2 {
            parts.push("2 messages from sender in the last hour".to_string());
        }

        if self.is_reply_to_user {
            parts.push("responding to your earlier message".to_string());
        }

        if parts.is_empty() {
            "no notable signals".to_string()
        } else {
            parts.join(", ")
        }
    }
}

/// Check for clock time patterns like "3:30", "15:00", "3pm", "3 pm"
fn has_clock_time(s: &str) -> bool {
    let bytes = s.as_bytes();
    let len = bytes.len();
    for i in 0..len {
        // Check for digit:digit pattern (3:30, 15:00)
        if bytes[i] == b':' && i > 0 && i + 1 < len {
            if bytes[i - 1].is_ascii_digit() && bytes[i + 1].is_ascii_digit() {
                return true;
            }
        }
        // Check for digit followed by "am" or "pm"
        if bytes[i].is_ascii_digit() && i + 2 <= len {
            let after = &s[i + 1..];
            if after.starts_with("am")
                || after.starts_with("pm")
                || after.starts_with(" am")
                || after.starts_with(" pm")
            {
                return true;
            }
        }
    }
    false
}

/// Twilio SMS error code decoder.
///
/// Maps Twilio error codes to a human-readable title, likely cause, and
/// suggested action. Used to enrich the failed-SMS admin alerts so a code
/// like "30007" turns into "Carrier filtering (10DLC compliance issue)"
/// with an actionable next step.
///
/// Source: https://www.twilio.com/docs/api/errors

/// Whether the failure is something we can act on or routine carrier noise.
///
/// - `CarrierNoise`: routine failures driven by the carrier/recipient, not by
///   anything Lightfriend can fix in the short term. These flow into the 6h
///   digest email only; they do NOT page the admin per-event.
/// - `Actionable`: failures that point at a config/code/account problem on our
///   side. These page the admin immediately via notify-server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCategory {
    CarrierNoise,
    Actionable,
}

#[derive(Debug, Clone)]
pub struct ErrorContext {
    pub title: &'static str,
    pub likely_cause: &'static str,
    pub suggested_action: &'static str,
    pub category: ErrorCategory,
}

pub fn decode(code: &str) -> Option<ErrorContext> {
    let normalized = code.trim();
    match normalized {
        // 21xxx — request validation errors (Twilio rejected before sending)
        "21211" => Some(ErrorContext {
            title: "Invalid To phone number",
            likely_cause: "The destination number is malformed or not E.164.",
            suggested_action: "Check the user's phone number on file. Most likely a bad signup or country code missing.",
            category: ErrorCategory::Actionable,
        }),
        "21408" => Some(ErrorContext {
            title: "SMS not enabled for region",
            likely_cause: "The destination country is not enabled on this Twilio account.",
            suggested_action: "Enable the country in Twilio Console > Messaging > Geo Permissions, or route through a provider that supports it.",
            category: ErrorCategory::Actionable,
        }),
        "21610" => Some(ErrorContext {
            title: "Recipient unsubscribed (STOP)",
            likely_cause: "The user replied STOP and is now opted out at the carrier level.",
            suggested_action: "User must reply START to resume. Do not retry from app; the block is enforced by Twilio.",
            category: ErrorCategory::CarrierNoise,
        }),
        "21611" => Some(ErrorContext {
            title: "Number is blocked",
            likely_cause: "The destination number is on Twilio's block list.",
            suggested_action: "Check Twilio Console for the block reason. User may have asked Twilio directly to block.",
            category: ErrorCategory::Actionable,
        }),
        "21612" => Some(ErrorContext {
            title: "Number cannot receive SMS",
            likely_cause: "Destination is a landline or VoIP that does not accept SMS.",
            suggested_action: "Ask the user for a mobile-capable number. Cannot be fixed on our side.",
            category: ErrorCategory::Actionable,
        }),
        "21614" => Some(ErrorContext {
            title: "Not a valid mobile number",
            likely_cause: "Destination is not a mobile-capable line.",
            suggested_action: "Same as 21612 — collect a mobile number from the user.",
            category: ErrorCategory::Actionable,
        }),

        // 30xxx — delivery failures (Twilio sent, carrier or downstream issue)
        "30001" => Some(ErrorContext {
            title: "Queue overflow",
            likely_cause: "Twilio's send queue for this number is full (too many messages too fast).",
            suggested_action: "Reduce outbound rate. Consider adding a backoff or a higher-throughput number.",
            category: ErrorCategory::Actionable,
        }),
        "30002" => Some(ErrorContext {
            title: "Account suspended",
            likely_cause: "The Twilio account is suspended (billing or trust/safety).",
            suggested_action: "Check Twilio account status immediately. Production-critical.",
            category: ErrorCategory::Actionable,
        }),
        "30003" => Some(ErrorContext {
            title: "Destination handset unreachable",
            likely_cause: "Phone off, out of coverage, or destination number stopped working.",
            suggested_action: "Retry later. If persistent for one user, ask them to confirm their number.",
            category: ErrorCategory::CarrierNoise,
        }),
        "30004" => Some(ErrorContext {
            title: "Message blocked",
            likely_cause: "Carrier blocked delivery. Could be number-level, content-pattern, or shortcode block.",
            suggested_action: "If persistent across users, check sender number reputation. If single user, they may have blocked our number.",
            category: ErrorCategory::CarrierNoise,
        }),
        "30005" => Some(ErrorContext {
            title: "Unknown destination handset",
            likely_cause: "Destination number is unassigned or no longer in service.",
            suggested_action: "Ask user to confirm their phone number. Common for stale signups.",
            category: ErrorCategory::Actionable,
        }),
        "30006" => Some(ErrorContext {
            title: "Landline or unreachable carrier",
            likely_cause: "Destination is a landline, or the carrier route is broken.",
            suggested_action: "If a single number, collect a mobile alternative. If carrier-wide, escalate to Twilio support.",
            category: ErrorCategory::Actionable,
        }),
        "30007" => Some(ErrorContext {
            title: "Carrier filtered (10DLC / spam)",
            likely_cause: "US carrier (AT&T, T-Mobile, Verizon) filtered the message. Usually an A2P 10DLC compliance issue: unregistered Brand/Campaign, low sender reputation, or international traffic through a US long code.",
            suggested_action: "If destination is US: verify 10DLC registration in Twilio Console (Messaging > Regulatory Compliance > A2P 10DLC). If destination is non-US sent through a US long code: route through Telnyx or a local long code for that region.",
            category: ErrorCategory::CarrierNoise,
        }),
        "30008" => Some(ErrorContext {
            title: "Unknown delivery error",
            likely_cause: "Carrier accepted but never confirmed delivery. Common for cross-border long-code SMS.",
            suggested_action: "If international: long-code SMS is unreliable across borders. Consider a local sender for that region. If domestic: usually transient — only worry if pattern persists.",
            category: ErrorCategory::CarrierNoise,
        }),
        "30009" => Some(ErrorContext {
            title: "Missing inbound segment",
            likely_cause: "Twilio received a partial multipart message that never completed.",
            suggested_action: "Carrier-side issue, nothing to do on our side.",
            category: ErrorCategory::CarrierNoise,
        }),
        "30010" => Some(ErrorContext {
            title: "Message price exceeds MaxPrice",
            likely_cause: "Outbound price for the route is higher than the configured MaxPrice on the message.",
            suggested_action: "Raise MaxPrice on outbound for that route, or route through a cheaper provider.",
            category: ErrorCategory::Actionable,
        }),
        "30011" => Some(ErrorContext {
            title: "WhatsApp template issue",
            likely_cause: "WhatsApp-specific: outgoing template was rejected or not approved.",
            suggested_action: "Check WhatsApp template approval status in Twilio Console.",
            category: ErrorCategory::Actionable,
        }),
        "30032" => Some(ErrorContext {
            title: "Toll-free number not registered",
            likely_cause: "Sending from a US toll-free number that has not completed verification.",
            suggested_action: "Submit toll-free verification in Twilio Console.",
            category: ErrorCategory::Actionable,
        }),
        "30033" => Some(ErrorContext {
            title: "US A2P daily cap / campaign throughput exceeded",
            likely_cause: "10DLC daily message limit reached for this campaign, OR the campaign is unregistered/under-tier.",
            suggested_action: "Check 10DLC campaign tier and daily volume in Twilio Console. May need to register a higher tier or split traffic across multiple campaigns.",
            category: ErrorCategory::CarrierNoise,
        }),
        "30034" => Some(ErrorContext {
            title: "US A2P unregistered traffic",
            likely_cause: "Sending to US destinations from a long code without A2P 10DLC registration.",
            suggested_action: "Complete 10DLC Brand + Campaign registration in Twilio, or route US traffic through Telnyx with their registration.",
            category: ErrorCategory::CarrierNoise,
        }),
        "30035" => Some(ErrorContext {
            title: "Message blocked by Twilio",
            likely_cause: "Twilio's content filters flagged the message (looks like spam/phishing/prohibited content).",
            suggested_action: "Review what the assistant generated. May need a guardrail on outbound content patterns.",
            category: ErrorCategory::CarrierNoise,
        }),
        "30036" => Some(ErrorContext {
            title: "Carrier-blocked (STIR/SHAKEN / spam)",
            likely_cause: "Verizon or similar blocked based on caller-ID attestation or spam scoring.",
            suggested_action: "Sender number reputation issue. Consider rotating numbers or improving 10DLC trust score.",
            category: ErrorCategory::CarrierNoise,
        }),

        _ => None,
    }
}

/// Decide whether a Twilio error code should page the admin immediately.
///
/// Unknown codes default to Actionable so new failure modes don't go silent.
pub fn is_actionable(code: Option<&str>) -> bool {
    let code = match code {
        Some(c) if !c.is_empty() && c != "N/A" => c,
        _ => return true,
    };
    match decode(code) {
        Some(ctx) => matches!(ctx.category, ErrorCategory::Actionable),
        None => true,
    }
}

/// Render the error context as plain-text lines for inclusion in alert bodies.
/// Returns an empty string if the code is unknown.
pub fn render_context_plain(code: Option<&str>) -> String {
    let code = match code {
        Some(c) if !c.is_empty() && c != "N/A" => c,
        _ => return String::new(),
    };
    match decode(code) {
        Some(ctx) => format!(
            "\nWhat this means: {}\nLikely cause: {}\nSuggested action: {}\n",
            ctx.title, ctx.likely_cause, ctx.suggested_action
        ),
        None => String::new(),
    }
}

/// Render the error context as HTML for inclusion in email bodies.
/// Returns an empty string if the code is unknown.
pub fn render_context_html(code: Option<&str>) -> String {
    let code = match code {
        Some(c) if !c.is_empty() && c != "N/A" => c,
        _ => return String::new(),
    };
    match decode(code) {
        Some(ctx) => format!(
            r#"<div style="margin: 16px 0; padding: 12px 16px; background: #f8f9fa; border-left: 4px solid #0066cc; border-radius: 4px;">
                <div style="font-weight: 600; margin-bottom: 6px;">{}</div>
                <div style="font-size: 14px; color: #555; margin-bottom: 4px;"><strong>Likely cause:</strong> {}</div>
                <div style="font-size: 14px; color: #555;"><strong>Suggested action:</strong> {}</div>
            </div>"#,
            ctx.title, ctx.likely_cause, ctx.suggested_action
        ),
        None => String::new(),
    }
}

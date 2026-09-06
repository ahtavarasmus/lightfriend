# Lightfriend landing design brief

Research and implementation prompt, 2026-09-06.

## References

- [Anthropic — Improving frontend design through Skills](https://claude.com/blog/improving-frontend-design-through-skills): supply explicit visual direction and product context instead of accepting familiar generated defaults. Typography, hierarchy, and composition should be deliberate.
- [InterfaceKit — What makes a website look AI-generated?](https://blog.interfacekit.io/what-makes-a-website-look-ai-generated): test whether the copy could describe another business, whether hierarchy survives removing decoration, and whether failure and mobile states work. This is practitioner advice, not a validated AI detector.
- [Nielsen Norman Group — How Users Read on the Web](https://www.nngroup.com/articles/how-users-read-on-the-web/): use descriptive headings, concise paragraphs, and factual language instead of promotional filler.
- [Nielsen Norman Group — Photos as Web Content](https://www.nngroup.com/articles/photos-as-web-content/): relevant product and real-person imagery carries more useful information than decorative stock imagery.

These references inform the following choices; they do not establish a universal formula for identifying AI authorship. Fonts, rounded corners, and colors are not individually evidence of AI generation.

## Prompt used for this revision

Refine Lightfriend as a small, personal service built by Rasmus for his own dumbphone. Keep the concrete Nokia headline and existing field photograph. Preserve useful product explanations, the labeled illustrative SMS exchange, accurate billing terms, and working Stripe checkout.

Apply this checklist:

- Replace interchangeable slogans with headings that say what the section contains. Remove ornamental uppercase kickers, repeated arrows, and decorative step numbers.
- Give the headline one clear focal point. Use an offset desktop hero so the child and the text have their own space; stack naturally on mobile.
- Break the repeated centered-heading / paragraph / card pattern. Use a left-aligned explanation beside the SMS example, compact setup rows, and a real founder portrait beside his own story.
- Let spacing, alignment, type size, and thin rules organize the content. Remove colored panels, decorative borders, drop shadows, fake logos, and nested card treatments. Keep speech bubbles because they explain the SMS interaction.
- Use restrained charcoal and white with Lightfriend's blue for links and primary actions. Keep body text comfortable to read. Do not add fashionable fonts, noisy textures, stickers, or motion simply to appear different.
- Keep existing customer words verbatim. Never invent customer proof, usage statistics, testimonials, or conversation provenance. Clearly label the example.
- Put countries directly on the landing: United States, Canada, Finland, Netherlands, United Kingdom, Australia. State these have local Lightfriend numbers. Other countries may work; offer rasmus@lightfriend.ai to check before subscribing. Remove landing links to the legacy supported-countries page.
- Make price, included usage, and overage terms easy to scan. Preserve all billing schedules and error recovery. Stripe controls its own embedded checkout styling.
- Reduce the footer's competing links and repeated sales copy. Keep contact, product documentation, source code, and legal links easy to find.
- Verify 320px and 390px widths, keyboard controls, local anchors, email links, contrast, and live pricing loading. Preserve analytics opt-out and reduced-motion behavior.

## Scope

This is a landing-page direction, not a redesign of signed-in application pages. Existing background artwork is retained; its provenance is not being reclassified as a real customer photograph. The monthly price summary must stay synchronized with Stripe when pricing changes.

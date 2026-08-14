use crate::profile::stripe::StripePricingTable;
use crate::utils::datafast::track_goal;
use crate::utils::seo::{use_seo, SeoMeta};
use gloo_timers::callback::Timeout;
use web_sys::window;
use yew::prelude::*;

const PLATFORM_STORAGE_KEY: &str = "lightfriend_onboarding_platforms";
const PRIORITY_STORAGE_KEY: &str = "lightfriend_onboarding_priorities";

#[derive(Clone, Copy, PartialEq, Eq)]
enum OnboardingStep {
    Choose,
    Preview,
    Peace,
    Plans,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Platform {
    Whatsapp,
    Signal,
    Telegram,
    Email,
}

impl Platform {
    fn slug(self) -> &'static str {
        match self {
            Self::Whatsapp => "whatsapp",
            Self::Signal => "signal",
            Self::Telegram => "telegram",
            Self::Email => "email",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Whatsapp => "WhatsApp",
            Self::Signal => "Signal",
            Self::Telegram => "Telegram",
            Self::Email => "Email",
        }
    }

    fn icon_class(self) -> &'static str {
        match self {
            Self::Whatsapp => "fa-brands fa-whatsapp",
            Self::Signal => "fa-brands fa-signal-messenger",
            Self::Telegram => "fa-brands fa-telegram",
            Self::Email => "fa-solid fa-envelope",
        }
    }

    fn promise(self) -> &'static str {
        match self {
            Self::Whatsapp => "Keep the people, leave the group-chat noise behind.",
            Self::Signal => "Stay reachable to the people who use Signal with you.",
            Self::Telegram => "Let channels stay busy without pulling you back in.",
            Self::Email => "Let deadlines reach you without living in your inbox.",
        }
    }

    fn from_slug(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "whatsapp" => Some(Self::Whatsapp),
            "signal" => Some(Self::Signal),
            "telegram" => Some(Self::Telegram),
            "email" => Some(Self::Email),
            _ => None,
        }
    }
}

const PLATFORMS: [Platform; 4] = [
    Platform::Whatsapp,
    Platform::Signal,
    Platform::Telegram,
    Platform::Email,
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Priority {
    ImportantPeople,
    TimeSensitive,
    Deadlines,
    Reminders,
}

impl Priority {
    fn slug(self) -> &'static str {
        match self {
            Self::ImportantPeople => "important-people",
            Self::TimeSensitive => "time-sensitive",
            Self::Deadlines => "deadlines",
            Self::Reminders => "reminders",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::ImportantPeople => "Important people",
            Self::TimeSensitive => "Time-sensitive changes",
            Self::Deadlines => "Deadlines & appointments",
            Self::Reminders => "Reminders & commitments",
        }
    }

    fn from_slug(value: &str) -> Option<Self> {
        match value.trim() {
            "important-people" => Some(Self::ImportantPeople),
            "time-sensitive" => Some(Self::TimeSensitive),
            "deadlines" => Some(Self::Deadlines),
            "reminders" => Some(Self::Reminders),
            _ => None,
        }
    }
}

const PRIORITIES: [Priority; 4] = [
    Priority::ImportantPeople,
    Priority::TimeSensitive,
    Priority::Deadlines,
    Priority::Reminders,
];

struct DemoStory {
    sender: &'static str,
    important_message: &'static str,
    reply: &'static str,
    quiet_sender_one: &'static str,
    quiet_message_one: &'static str,
    quiet_sender_two: &'static str,
    quiet_message_two: &'static str,
}

fn story_for(platform: Platform) -> DemoStory {
    match platform {
        Platform::Whatsapp => DemoStory {
            sender: "Anna",
            important_message: "Can you pick Leo up at 17:30? I’m stuck at work.",
            reply: "Yes, I’ll get him.",
            quiet_sender_one: "Weekend plans",
            quiet_message_one: "14 new messages",
            quiet_sender_two: "Family group",
            quiet_message_two: "Mika sent a photo",
        },
        Platform::Signal => DemoStory {
            sender: "Mika",
            important_message: "The train leaves at 18:05, not 18:35. Can you still make it?",
            reply: "Yes. I’m leaving now.",
            quiet_sender_one: "Cycling group",
            quiet_message_one: "8 new messages",
            quiet_sender_two: "Sofia",
            quiet_message_two: "Reacted to a photo",
        },
        Platform::Telegram => DemoStory {
            sender: "Volunteer team",
            important_message: "Tonight’s meeting moved to the library at 19:00.",
            reply: "Thanks, I’ll be there.",
            quiet_sender_one: "Local news",
            quiet_message_one: "21 new posts",
            quiet_sender_two: "Book club",
            quiet_message_two: "5 new messages",
        },
        Platform::Email => DemoStory {
            sender: "Northside Dental",
            important_message: "Your appointment today has moved from 16:00 to 14:30.",
            reply: "14:30 works for me. Thank you.",
            quiet_sender_one: "Weekly newsletter",
            quiet_message_one: "Your Friday reading list",
            quiet_sender_two: "Store updates",
            quiet_message_two: "New season, now available",
        },
    }
}

fn parse_storage<T>(key: &str, parser: impl Fn(&str) -> Option<T>) -> Vec<T> {
    window()
        .and_then(|window| window.local_storage().ok().flatten())
        .and_then(|storage| storage.get_item(key).ok().flatten())
        .map(|value| value.split(',').filter_map(parser).collect())
        .unwrap_or_default()
}

fn initial_platforms() -> Vec<Platform> {
    if let Some(platform) = window()
        .and_then(|window| window.location().search().ok())
        .and_then(|search| web_sys::UrlSearchParams::new_with_str(&search).ok())
        .and_then(|params| params.get("platform"))
        .and_then(|value| Platform::from_slug(&value))
    {
        return vec![platform];
    }

    parse_storage(PLATFORM_STORAGE_KEY, Platform::from_slug)
}

fn initial_priorities() -> Vec<Priority> {
    let saved = parse_storage(PRIORITY_STORAGE_KEY, Priority::from_slug);
    if saved.is_empty() {
        vec![Priority::ImportantPeople, Priority::TimeSensitive]
    } else {
        saved
    }
}

fn persist_choices(platforms: &[Platform], priorities: &[Priority]) {
    let Some(storage) = window().and_then(|window| window.local_storage().ok().flatten()) else {
        return;
    };

    let platform_value = platforms
        .iter()
        .map(|platform| platform.slug())
        .collect::<Vec<_>>()
        .join(",");
    let priority_value = priorities
        .iter()
        .map(|priority| priority.slug())
        .collect::<Vec<_>>()
        .join(",");
    let _ = storage.set_item(PLATFORM_STORAGE_KEY, &platform_value);
    let _ = storage.set_item(PRIORITY_STORAGE_KEY, &priority_value);
}

fn priority_reason(priorities: &[Priority]) -> &'static str {
    if priorities.contains(&Priority::TimeSensitive) {
        "Time-sensitive change"
    } else if priorities.contains(&Priority::ImportantPeople) {
        "Important person"
    } else if priorities.contains(&Priority::Deadlines) {
        "Deadline or appointment"
    } else {
        "Commitment you asked to protect"
    }
}

fn selected_platform_names(platforms: &[Platform]) -> String {
    let names = platforms
        .iter()
        .map(|platform| platform.label())
        .collect::<Vec<_>>();
    match names.as_slice() {
        [] => String::new(),
        [name] => (*name).to_string(),
        [first, second] => format!("{} and {}", first, second),
        _ => format!(
            "{}, and {}",
            names[..names.len() - 1].join(", "),
            names[names.len() - 1]
        ),
    }
}

#[function_component(Onboarding)]
pub fn onboarding() -> Html {
    use_seo(SeoMeta {
        title: "Meet your Lightfriend — See how it protects your attention",
        description: "Choose the places that matter and see how Lightfriend keeps routine messages quiet while important people reach any phone.",
        canonical: "https://lightfriend.ai/get-started",
        og_type: "website",
    });

    let step = use_state(|| OnboardingStep::Choose);
    let selected_platforms = use_state(initial_platforms);
    let selected_priorities = use_state(initial_priorities);
    let active_platform = use_state(|| {
        initial_platforms()
            .first()
            .copied()
            .unwrap_or(Platform::Whatsapp)
    });
    let demo_phase = use_state(|| 0_u8);
    let demo_run = use_state(|| 0_u32);
    let demo_skipped = use_state(|| false);

    {
        let current_step = *step;
        use_effect_with_deps(
            move |current_step| {
                if let Some(window) = window() {
                    window.scroll_to_with_x_and_y(0.0, 0.0);
                }
                let step_name = match *current_step {
                    OnboardingStep::Choose => "choose",
                    OnboardingStep::Preview => "preview",
                    OnboardingStep::Peace => "peace",
                    OnboardingStep::Plans => "plans",
                };
                track_goal("onboarding_step_view", &[("step", step_name)]);
                || ()
            },
            current_step,
        );
    }

    {
        let demo_phase = demo_phase.clone();
        let dependency = (*step, *demo_run, *active_platform, *demo_skipped);
        use_effect_with_deps(
            move |(current_step, _, _, skipped)| {
                let mut timers = Vec::new();
                if *current_step == OnboardingStep::Preview {
                    if *skipped {
                        demo_phase.set(7);
                    } else {
                        demo_phase.set(0);
                        for (delay, phase) in [
                            (450, 1),
                            (1_650, 2),
                            (2_850, 3),
                            (4_150, 4),
                            (5_650, 5),
                            (6_900, 6),
                            (8_100, 7),
                        ] {
                            let demo_phase = demo_phase.clone();
                            timers.push(Timeout::new(delay, move || demo_phase.set(phase)));
                        }
                    }
                }
                move || drop(timers)
            },
            dependency,
        );
    }

    {
        let phase = *demo_phase;
        let platform = *active_platform;
        let skipped = *demo_skipped;
        use_effect_with_deps(
            move |(phase, skipped)| {
                if *phase == 7 && !*skipped {
                    track_goal(
                        "onboarding_demo_completed",
                        &[("platform", platform.slug())],
                    );
                }
                || ()
            },
            (phase, skipped),
        );
    }

    let toggle_platform = {
        let selected_platforms = selected_platforms.clone();
        Callback::from(move |platform: Platform| {
            let mut next = (*selected_platforms).clone();
            if let Some(index) = next.iter().position(|item| *item == platform) {
                next.remove(index);
                track_goal(
                    "onboarding_platform_toggled",
                    &[("platform", platform.slug()), ("selected", "false")],
                );
            } else {
                next.push(platform);
                track_goal(
                    "onboarding_platform_toggled",
                    &[("platform", platform.slug()), ("selected", "true")],
                );
            }
            selected_platforms.set(next);
        })
    };

    let toggle_priority = {
        let selected_priorities = selected_priorities.clone();
        Callback::from(move |priority: Priority| {
            let mut next = (*selected_priorities).clone();
            if let Some(index) = next.iter().position(|item| *item == priority) {
                if next.len() > 1 {
                    next.remove(index);
                }
            } else {
                next.push(priority);
            }
            selected_priorities.set(next);
        })
    };

    let begin_preview = {
        let step = step.clone();
        let selected_platforms = selected_platforms.clone();
        let selected_priorities = selected_priorities.clone();
        let active_platform = active_platform.clone();
        let demo_skipped = demo_skipped.clone();
        Callback::from(move |_: MouseEvent| {
            let Some(first_platform) = selected_platforms.first().copied() else {
                return;
            };
            active_platform.set(first_platform);
            demo_skipped.set(false);
            persist_choices(&selected_platforms, &selected_priorities);
            let selected_count = selected_platforms.len().to_string();
            track_goal(
                "onboarding_preferences_completed",
                &[("selected_platforms", selected_count.as_str())],
            );
            step.set(OnboardingStep::Preview);
        })
    };

    let replay_demo = {
        let demo_run = demo_run.clone();
        let demo_skipped = demo_skipped.clone();
        Callback::from(move |_: MouseEvent| {
            demo_skipped.set(false);
            demo_run.set(*demo_run + 1);
        })
    };

    let skip_demo = {
        let demo_skipped = demo_skipped.clone();
        Callback::from(move |_: MouseEvent| {
            demo_skipped.set(true);
            track_goal("onboarding_demo_skipped", &[]);
        })
    };

    let show_peace = {
        let step = step.clone();
        Callback::from(move |_: MouseEvent| step.set(OnboardingStep::Peace))
    };

    let show_plans = {
        let step = step.clone();
        Callback::from(move |_: MouseEvent| {
            track_goal("onboarding_pricing_opened", &[("source", "peace_step")]);
            step.set(OnboardingStep::Plans);
        })
    };

    let back_to_choose = {
        let step = step.clone();
        Callback::from(move |_: MouseEvent| step.set(OnboardingStep::Choose))
    };

    let back_to_preview = {
        let step = step.clone();
        let demo_run = demo_run.clone();
        let demo_skipped = demo_skipped.clone();
        Callback::from(move |_: MouseEvent| {
            demo_skipped.set(false);
            demo_run.set(*demo_run + 1);
            step.set(OnboardingStep::Preview);
        })
    };

    let back_to_peace = {
        let step = step.clone();
        Callback::from(move |_: MouseEvent| step.set(OnboardingStep::Peace))
    };

    let story = story_for(*active_platform);
    let phase_class = format!("phase-{}", *demo_phase);
    let platform_names = selected_platform_names(&selected_platforms);
    let progress_value = match *step {
        OnboardingStep::Choose => 1,
        OnboardingStep::Preview => 2,
        OnboardingStep::Peace => 3,
        OnboardingStep::Plans => 4,
    };

    html! {
        <>
            <style>{ONBOARDING_STYLES}</style>
            <main class="onboarding-page">
                <div class="onboarding-atmosphere" aria-hidden="true">
                    <span class="atmosphere-orb atmosphere-orb-one"></span>
                    <span class="atmosphere-orb atmosphere-orb-two"></span>
                </div>

                <header class="onboarding-progress" aria-label="Onboarding progress">
                    <span class="onboarding-context">{"Your quiet setup"}</span>
                    <div class="progress-track" aria-hidden="true">
                        {for (1..=4).map(|index| html! {
                            <span class={classes!("progress-segment", (index <= progress_value).then_some("active"))}></span>
                        })}
                    </div>
                    <span class="progress-copy">{format!("{} of 4", progress_value)}</span>
                </header>

                {
                    match *step {
                        OnboardingStep::Choose => html! {
                            <section class="onboarding-screen choose-screen" aria-labelledby="choose-heading">
                                <div class="screen-heading">
                                    <p class="onboarding-eyebrow">{"Make it yours"}</p>
                                    <h1 id="choose-heading">{"Where do you want to stop checking?"}</h1>
                                    <p>{"Choose every place that matters. We’ll show you exactly how Lightfriend gives your attention back."}</p>
                                </div>

                                <div class="platform-grid" role="group" aria-label="Platforms">
                                    {for PLATFORMS.iter().copied().map(|platform| {
                                        let selected = selected_platforms.contains(&platform);
                                        let toggle_platform = toggle_platform.clone();
                                        html! {
                                            <button
                                                type="button"
                                                class={classes!("platform-choice", selected.then_some("selected"))}
                                                aria-pressed={selected.to_string()}
                                                onclick={Callback::from(move |_| toggle_platform.emit(platform))}
                                            >
                                                <span class="platform-choice-icon"><i class={platform.icon_class()} aria-hidden="true"></i></span>
                                                <span class="platform-choice-copy">
                                                    <strong>{platform.label()}</strong>
                                                    <small>{platform.promise()}</small>
                                                </span>
                                                <span class="choice-check" aria-hidden="true"><i class="fa-solid fa-check"></i></span>
                                            </button>
                                        }
                                    })}
                                </div>

                                <div class="priority-panel">
                                    <div>
                                        <p class="priority-title">{"What should break through?"}</p>
                                        <p class="priority-subtitle">{"Pick what deserves your attention. You can change this later."}</p>
                                    </div>
                                    <div class="priority-options" role="group" aria-label="Messages that should break through">
                                        {for PRIORITIES.iter().copied().map(|priority| {
                                            let selected = selected_priorities.contains(&priority);
                                            let toggle_priority = toggle_priority.clone();
                                            html! {
                                                <button
                                                    type="button"
                                                    class={classes!("priority-chip", selected.then_some("selected"))}
                                                    aria-pressed={selected.to_string()}
                                                    onclick={Callback::from(move |_| toggle_priority.emit(priority))}
                                                >
                                                    <span class="priority-dot"></span>
                                                    {priority.label()}
                                                </button>
                                            }
                                        })}
                                    </div>
                                </div>

                                <div class="screen-actions">
                                    <span class="selection-hint">
                                        {if selected_platforms.is_empty() {
                                            "Choose at least one place to continue".to_string()
                                        } else {
                                            format!("We’ll preview {} first", selected_platforms[0].label())
                                        }}
                                    </span>
                                    <button
                                        type="button"
                                        class="onboarding-primary"
                                        disabled={selected_platforms.is_empty()}
                                        onclick={begin_preview}
                                    >
                                        <span>{"Show me how it works"}</span>
                                        <i class="fa-solid fa-arrow-right" aria-hidden="true"></i>
                                    </button>
                                </div>
                            </section>
                        },
                        OnboardingStep::Preview => html! {
                            <section class="onboarding-screen preview-screen" aria-labelledby="preview-heading">
                                <div class="preview-heading-row">
                                    <div class="screen-heading preview-heading">
                                        <p class="onboarding-eyebrow">{"A quiet layer between you and the noise"}</p>
                                        <h1 id="preview-heading">{format!("See Lightfriend handle {}.", active_platform.label())}</h1>
                                    </div>
                                    if selected_platforms.len() > 1 {
                                        <div class="preview-platform-tabs" role="tablist" aria-label="Preview another selected platform">
                                            {for selected_platforms.iter().copied().map(|platform| {
                                                let active = *active_platform == platform;
                                                let active_platform = active_platform.clone();
                                                let demo_skipped = demo_skipped.clone();
                                                html! {
                                                    <button
                                                        type="button"
                                                        role="tab"
                                                        aria-selected={active.to_string()}
                                                        class={classes!("preview-platform-tab", active.then_some("active"))}
                                                        onclick={Callback::from(move |_| {
                                                            demo_skipped.set(false);
                                                            active_platform.set(platform);
                                                        })}
                                                    >
                                                        <i class={platform.icon_class()} aria-hidden="true"></i>
                                                        <span>{platform.label()}</span>
                                                    </button>
                                                }
                                            })}
                                        </div>
                                    }
                                </div>

                                <div class={classes!("demo-stage", phase_class)} aria-live="polite">
                                    <article class="demo-source-card">
                                        <div class="demo-card-header">
                                            <span class="demo-app-identity">
                                                <i class={active_platform.icon_class()} aria-hidden="true"></i>
                                                <strong>{active_platform.label()}</strong>
                                            </span>
                                            <span class="demo-live-indicator"><span></span>{"Incoming"}</span>
                                        </div>
                                        <div class="source-message quiet-message first">
                                            <span class="source-avatar">{story.quiet_sender_one.chars().next().unwrap_or('•')}</span>
                                            <span><strong>{story.quiet_sender_one}</strong><small>{story.quiet_message_one}</small></span>
                                        </div>
                                        <div class="source-message important-message">
                                            <span class="source-avatar important">{story.sender.chars().next().unwrap_or('•')}</span>
                                            <span><strong>{story.sender}</strong><small>{story.important_message}</small></span>
                                            <span class="important-badge">{"Matters"}</span>
                                        </div>
                                        <div class="source-message quiet-message second">
                                            <span class="source-avatar">{story.quiet_sender_two.chars().next().unwrap_or('•')}</span>
                                            <span><strong>{story.quiet_sender_two}</strong><small>{story.quiet_message_two}</small></span>
                                        </div>
                                        <div class="source-reply-confirmation">
                                            <i class="fa-solid fa-check" aria-hidden="true"></i>
                                            {format!("Reply sent to {}", story.sender)}
                                        </div>
                                    </article>

                                    <div class="demo-bridge" aria-label="Lightfriend decides what needs your attention">
                                        <span class="bridge-line bridge-line-in"></span>
                                        <span class="bridge-line bridge-line-out"></span>
                                        <span class="bridge-pulse bridge-pulse-forward"></span>
                                        <span class="bridge-pulse bridge-pulse-return"></span>
                                        <div class="lightfriend-core">
                                            <img src="/assets/fav.png" alt="" />
                                        </div>
                                        <div class="decision-copy">
                                            if *demo_phase < 2 {
                                                <span>{"Watching quietly"}</span>
                                            } else if *demo_phase == 2 {
                                                <span>{"Reading the moment"}</span>
                                            } else if *demo_phase < 6 {
                                                <><strong>{"This matters"}</strong><small>{priority_reason(&selected_priorities)}</small></>
                                            } else {
                                                <><strong>{"Handled"}</strong><small>{"You never opened the app"}</small></>
                                            }
                                        </div>
                                        <div class="quiet-counter">
                                            <strong>{"22"}</strong>
                                            <span>{"routine updates stayed quiet"}</span>
                                        </div>
                                    </div>

                                    <article class="demo-phone-wrap">
                                        <div class="phone-shadow"></div>
                                        <div class="demo-phone">
                                            <div class="phone-speaker"></div>
                                            <div class="phone-screen">
                                                <div class="phone-status"><span>{"LIGHTFRIEND"}</span><span>{"12:42"}</span></div>
                                                <div class="phone-resting-state">
                                                    <span class="resting-time">{"12:42"}</span>
                                                    <small>{"Nothing needs your attention"}</small>
                                                </div>
                                                <div class="phone-notification">
                                                    <div class="phone-notification-label">
                                                        <img src="/assets/fav.png" alt="" />
                                                        <span>{format!("Important {}", active_platform.label())}</span>
                                                    </div>
                                                    <strong>{story.sender}</strong>
                                                    <p>{story.important_message}</p>
                                                    <small>{"Reply here to answer"}</small>
                                                </div>
                                                <div class="phone-reply">
                                                    <span>{story.reply}</span>
                                                    <i class="fa-solid fa-check-double" aria-hidden="true"></i>
                                                </div>
                                                <div class="phone-finished">
                                                    <i class="fa-solid fa-check" aria-hidden="true"></i>
                                                    <strong>{"Reply delivered"}</strong>
                                                    <small>{"Nothing else needs you"}</small>
                                                </div>
                                            </div>
                                            <div class="phone-controls"><span></span><span></span><span></span></div>
                                            <div class="phone-keypad" aria-hidden="true">
                                                {for (1..=9).map(|number| html! { <span>{number}</span> })}
                                                <span>{"*"}</span><span>{"0"}</span><span>{"#"}</span>
                                            </div>
                                        </div>
                                    </article>
                                </div>

                                <div class="demo-caption">
                                    <div>
                                        <strong>{
                                            if *demo_phase < 3 {
                                                "Lightfriend keeps watch, so you don’t have to."
                                            } else if *demo_phase < 7 {
                                                "Only the message that matters crosses the quiet."
                                            } else {
                                                "One important moment handled. Twenty-two reasons not to check."
                                            }
                                        }</strong>
                                        <span>{"This is a preview. You choose the rules after connecting."}</span>
                                    </div>
                                    <div class="demo-controls">
                                        <button type="button" class="onboarding-quiet-button" onclick={replay_demo}>
                                            <i class="fa-solid fa-rotate-right" aria-hidden="true"></i>{"Replay"}
                                        </button>
                                        if *demo_phase < 7 {
                                            <button type="button" class="onboarding-quiet-button" onclick={skip_demo}>{"Skip animation"}</button>
                                        }
                                    </div>
                                </div>

                                <div class="screen-actions preview-actions">
                                    <button type="button" class="onboarding-back" onclick={back_to_choose}>
                                        <i class="fa-solid fa-arrow-left" aria-hidden="true"></i>{"Change choices"}
                                    </button>
                                    <button type="button" class="onboarding-primary" onclick={show_peace}>
                                        <span>{"Continue"}</span><i class="fa-solid fa-arrow-right" aria-hidden="true"></i>
                                    </button>
                                </div>
                            </section>
                        },
                        OnboardingStep::Peace => html! {
                            <section class="onboarding-screen peace-screen" aria-labelledby="peace-heading">
                                <div class="peace-visual" aria-hidden="true">
                                    <span class="peace-ring peace-ring-one"></span>
                                    <span class="peace-ring peace-ring-two"></span>
                                    <span class="peace-ring peace-ring-three"></span>
                                    <div class="peace-core"><img src="/assets/fav.png" alt="" /></div>
                                    {for selected_platforms.iter().copied().enumerate().map(|(index, platform)| html! {
                                        <span class={classes!("peace-platform", format!("peace-platform-{}", index + 1))}>
                                            <i class={platform.icon_class()}></i>
                                        </span>
                                    })}
                                </div>
                                <div class="peace-copy">
                                    <p class="onboarding-eyebrow">{"The feeling Lightfriend is built for"}</p>
                                    <h1 id="peace-heading">{"You’re free to stop checking."}</h1>
                                    <p class="peace-lead">
                                        {format!("Lightfriend can keep watch over {} and reach you on any phone when something truly deserves your attention.", platform_names)}
                                    </p>
                                    <div class="peace-statement">
                                        <span class="peace-status-dot"></span>
                                        <div>
                                            <strong>{"Until then, there’s nothing to check."}</strong>
                                            <small>{"Go fully offline. Be present. Lightfriend has your back."}</small>
                                        </div>
                                    </div>
                                    <p class="preview-disclaimer">{"Nothing has been connected yet. Your trial includes guided setup for the services you selected."}</p>
                                    <div class="peace-actions">
                                        <button type="button" class="onboarding-back" onclick={back_to_preview}>
                                            <i class="fa-solid fa-arrow-left" aria-hidden="true"></i>{"Watch again"}
                                        </button>
                                        <button type="button" class="onboarding-primary calm-primary" onclick={show_plans}>
                                            <span>{"See plans and start free trial"}</span><i class="fa-solid fa-arrow-right" aria-hidden="true"></i>
                                        </button>
                                    </div>
                                </div>
                            </section>
                        },
                        OnboardingStep::Plans => html! {
                            <section class="onboarding-screen plans-screen" aria-labelledby="plans-heading">
                                <div class="plans-heading-row">
                                    <div class="screen-heading plans-heading">
                                        <p class="onboarding-eyebrow">{"Your quiet starts here"}</p>
                                        <h1 id="plans-heading">{"Choose how Lightfriend reaches you."}</h1>
                                        <p>{"Every plan starts with a 7-day free trial. You’ll connect your first selected service after checkout."}</p>
                                    </div>
                                    <div class="plan-summary">
                                        <span>{"Your preview"}</span>
                                        <strong>{platform_names}</strong>
                                        <small>{"Important moments reach any phone. Routine noise stays quiet."}</small>
                                    </div>
                                </div>
                                <div class="onboarding-pricing-shell" data-testid="onboarding-pricing-table">
                                    <StripePricingTable />
                                </div>
                                <div class="plans-footer">
                                    <button type="button" class="onboarding-back" onclick={back_to_peace}>
                                        <i class="fa-solid fa-arrow-left" aria-hidden="true"></i>{"Back"}</button>
                                    <p><i class="fa-solid fa-shield-halved" aria-hidden="true"></i>{"Open source · Private by architecture · Cancel anytime"}</p>
                                </div>
                            </section>
                        },
                    }
                }
            </main>
        </>
    }
}

const ONBOARDING_STYLES: &str = r#"
    .onboarding-page {
        --ob-bg: #0b1013; --ob-panel: rgba(19, 27, 31, 0.9); --ob-panel-soft: rgba(255, 255, 255, 0.045);
        --ob-border: rgba(235, 245, 240, 0.12); --ob-text: #f4f7f3; --ob-muted: rgba(228, 237, 232, 0.64);
        --ob-faint: rgba(228, 237, 232, 0.42); --ob-mint: #a8dec2; --ob-mint-deep: #6eaf91;
        position: relative; min-height: 100svh; overflow: hidden; padding: 6.6rem 1.5rem 4rem;
        background: radial-gradient(circle at 18% 0%, rgba(93, 150, 124, 0.12), transparent 34rem),
            radial-gradient(circle at 84% 95%, rgba(90, 129, 149, 0.12), transparent 36rem), var(--ob-bg);
        color: var(--ob-text); font-family: 'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif; box-sizing: border-box;
    }
    .onboarding-page * { box-sizing: border-box; }
    .onboarding-atmosphere { position: absolute; inset: 0; overflow: hidden; pointer-events: none; }
    .atmosphere-orb { position: absolute; width: 26rem; height: 26rem; border: 1px solid rgba(168, 222, 194, 0.08); border-radius: 50%; }
    .atmosphere-orb::before, .atmosphere-orb::after { content: ''; position: absolute; inset: 12%; border: inherit; border-radius: inherit; }
    .atmosphere-orb::after { inset: 28%; }
    .atmosphere-orb-one { top: -16rem; right: -10rem; } .atmosphere-orb-two { bottom: -19rem; left: -11rem; }
    .onboarding-progress { position: relative; z-index: 2; display: grid; grid-template-columns: 1fr minmax(140px, 220px) 1fr; align-items: center; width: min(100%, 1180px); margin: 0 auto 4rem; }
    .onboarding-context { color: var(--ob-faint); font-size: 0.65rem; font-weight: 700; letter-spacing: 0.1em; text-transform: uppercase; }
    .progress-track { display: grid; grid-template-columns: repeat(4, 1fr); gap: 6px; }
    .progress-segment { height: 3px; overflow: hidden; border-radius: 999px; background: rgba(255, 255, 255, 0.09); }
    .progress-segment::after { content: ''; display: block; width: 100%; height: 100%; border-radius: inherit; background: var(--ob-mint); transform: scaleX(0); transform-origin: left; transition: transform 240ms ease-out; }
    .progress-segment.active::after { transform: scaleX(1); }
    .progress-copy { justify-self: end; color: var(--ob-faint); font-size: 0.72rem; font-variant-numeric: tabular-nums; letter-spacing: 0.08em; text-transform: uppercase; }
    .onboarding-screen { position: relative; z-index: 1; width: min(100%, 1180px); margin: 0 auto; }
    .screen-heading { max-width: 790px; }
    .onboarding-eyebrow { margin: 0 0 1rem; color: var(--ob-mint); font-size: 0.72rem; font-weight: 700; letter-spacing: 0.14em; text-transform: uppercase; }
    .screen-heading h1, .peace-copy h1 { margin: 0; color: var(--ob-text); font-size: clamp(2.55rem, 5.5vw, 5.4rem); font-weight: 590; letter-spacing: -0.06em; line-height: 0.99; text-wrap: balance; }
    .screen-heading > p:last-child { max-width: 680px; margin: 1.35rem 0 0; color: var(--ob-muted); font-size: clamp(1rem, 1.7vw, 1.18rem); line-height: 1.65; text-wrap: pretty; }
    .platform-grid { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 0.8rem; margin-top: 3.2rem; }
    .platform-choice { position: relative; display: grid; grid-template-rows: auto 1fr; gap: 1.7rem; min-height: 205px; padding: 1.35rem; overflow: hidden; color: var(--ob-text); text-align: left; border: 1px solid var(--ob-border); border-radius: 20px; background: linear-gradient(145deg, rgba(255,255,255,0.06), rgba(255,255,255,0.025)); box-shadow: 0 18px 50px rgba(1, 8, 11, 0.16); cursor: pointer; transition: border-color 180ms ease, background 180ms ease, transform 150ms ease; }
    .platform-choice::before { content: ''; position: absolute; inset: 0; opacity: 0; background: radial-gradient(circle at 15% 12%, rgba(168, 222, 194, 0.15), transparent 52%); transition: opacity 180ms ease; }
    .platform-choice:hover { border-color: rgba(168, 222, 194, 0.42); transform: translateY(-2px); } .platform-choice:active { transform: scale(0.98); }
    .platform-choice.selected { border-color: rgba(168, 222, 194, 0.7); background: linear-gradient(145deg, rgba(168, 222, 194, 0.12), rgba(255,255,255,0.035)); }
    .platform-choice.selected::before { opacity: 1; }
    .platform-choice-icon { position: relative; display: grid; place-items: center; width: 48px; height: 48px; border: 1px solid rgba(255,255,255,0.12); border-radius: 14px; background: rgba(5, 12, 15, 0.54); font-size: 1.35rem; }
    .platform-choice-copy { position: relative; display: grid; align-content: end; gap: 0.5rem; }
    .platform-choice-copy strong { font-size: 1.04rem; font-weight: 650; } .platform-choice-copy small { color: var(--ob-muted); font-size: 0.79rem; line-height: 1.5; }
    .choice-check { position: absolute; top: 1rem; right: 1rem; display: grid; place-items: center; width: 25px; height: 25px; border: 1px solid rgba(255,255,255,0.15); border-radius: 50%; color: transparent; font-size: 0.65rem; transition: color 180ms ease, background 180ms ease, border-color 180ms ease; }
    .platform-choice.selected .choice-check { border-color: var(--ob-mint); background: var(--ob-mint); color: #0c1712; }
    .priority-panel { display: grid; grid-template-columns: minmax(210px, 0.62fr) 1.38fr; gap: 2rem; align-items: center; margin-top: 1rem; padding: 1.25rem 1.35rem; border: 1px solid var(--ob-border); border-radius: 18px; background: rgba(5, 11, 14, 0.34); }
    .priority-title { margin: 0; font-size: 0.94rem; font-weight: 650; } .priority-subtitle { margin: 0.32rem 0 0; color: var(--ob-faint); font-size: 0.74rem; line-height: 1.4; }
    .priority-options { display: flex; flex-wrap: wrap; gap: 0.5rem; }
    .priority-chip { display: inline-flex; align-items: center; gap: 0.55rem; min-height: 38px; padding: 0.54rem 0.76rem; color: var(--ob-muted); border: 1px solid rgba(255,255,255,0.1); border-radius: 999px; background: rgba(255,255,255,0.03); font: inherit; font-size: 0.76rem; cursor: pointer; transition: color 160ms ease, border-color 160ms ease, background 160ms ease, transform 150ms ease; }
    .priority-chip:hover { color: var(--ob-text); border-color: rgba(168,222,194,0.38); } .priority-chip:active { transform: scale(0.97); }
    .priority-dot { width: 7px; height: 7px; border: 1px solid currentColor; border-radius: 50%; }
    .priority-chip.selected { color: #d9f3e5; border-color: rgba(168,222,194,0.42); background: rgba(168,222,194,0.09); }
    .priority-chip.selected .priority-dot { border-color: var(--ob-mint); background: var(--ob-mint); box-shadow: 0 0 0 3px rgba(168,222,194,0.09); }
    .screen-actions { display: flex; justify-content: space-between; align-items: center; gap: 1rem; margin-top: 2rem; }
    .selection-hint { color: var(--ob-faint); font-size: 0.75rem; }
    .onboarding-primary, .onboarding-back, .onboarding-quiet-button { font: inherit; cursor: pointer; }
    .onboarding-primary { display: inline-flex; align-items: center; justify-content: center; gap: 0.75rem; min-height: 50px; padding: 0.78rem 1.15rem; border: 0; border-radius: 999px; background: var(--ob-text); color: #0b1114; font-size: 0.84rem; font-weight: 700; box-shadow: 0 10px 30px rgba(0,0,0,0.18), inset 0 -1px 0 rgba(0,0,0,0.16); transition: background 160ms ease, transform 150ms ease, opacity 160ms ease; }
    .onboarding-primary:hover { background: #fff; transform: translateY(-1px); } .onboarding-primary:active { transform: scale(0.98); }
    .onboarding-primary:disabled { opacity: 0.3; cursor: not-allowed; transform: none; }
    .onboarding-back, .onboarding-quiet-button { display: inline-flex; align-items: center; gap: 0.5rem; min-height: 40px; padding: 0.55rem 0; border: 0; background: transparent; color: var(--ob-muted); font-size: 0.78rem; transition: color 160ms ease, transform 150ms ease; }
    .onboarding-back:hover, .onboarding-quiet-button:hover { color: var(--ob-text); } .onboarding-back:active, .onboarding-quiet-button:active { transform: scale(0.97); }
    .preview-heading-row, .plans-heading-row { display: flex; justify-content: space-between; align-items: end; gap: 2rem; }
    .preview-heading h1, .plans-heading h1 { font-size: clamp(2.2rem, 4.1vw, 4rem); }
    .preview-platform-tabs { display: flex; gap: 0.35rem; padding: 0.3rem; border: 1px solid var(--ob-border); border-radius: 999px; background: rgba(0,0,0,0.14); }
    .preview-platform-tab { display: inline-flex; align-items: center; gap: 0.45rem; min-height: 36px; padding: 0.48rem 0.7rem; color: var(--ob-faint); border: 0; border-radius: 999px; background: transparent; font: inherit; font-size: 0.72rem; cursor: pointer; transition: color 160ms ease, background 160ms ease; }
    .preview-platform-tab.active { color: var(--ob-text); background: rgba(255,255,255,0.09); }
    .demo-stage { position: relative; display: grid; grid-template-columns: minmax(250px, 1fr) 180px minmax(260px, 0.92fr); gap: 1.3rem; align-items: center; min-height: 500px; margin-top: 2.3rem; padding: clamp(1rem, 2.6vw, 2rem); overflow: hidden; border: 1px solid var(--ob-border); border-radius: 28px; background: radial-gradient(circle at 50% 45%, rgba(168,222,194,0.065), transparent 34%), rgba(255,255,255,0.025); box-shadow: 0 35px 90px rgba(0,0,0,0.22); }
    .demo-stage::before { content: ''; position: absolute; inset: 0; opacity: 0.32; background-image: linear-gradient(rgba(255,255,255,0.025) 1px, transparent 1px), linear-gradient(90deg, rgba(255,255,255,0.025) 1px, transparent 1px); background-size: 40px 40px; mask-image: radial-gradient(circle at center, black, transparent 76%); pointer-events: none; }
    .demo-source-card { position: relative; z-index: 1; align-self: center; min-height: 330px; padding: 1.1rem; border: 1px solid rgba(255,255,255,0.12); border-radius: 20px; background: rgba(16,23,27,0.9); box-shadow: 0 22px 60px rgba(0,0,0,0.26); }
    .demo-card-header { display: flex; align-items: center; justify-content: space-between; margin-bottom: 1rem; }
    .demo-app-identity { display: inline-flex; align-items: center; gap: 0.6rem; font-size: 0.8rem; } .demo-app-identity i { color: var(--ob-mint); font-size: 1.05rem; }
    .demo-live-indicator { display: inline-flex; align-items: center; gap: 0.35rem; color: var(--ob-faint); font-size: 0.63rem; letter-spacing: 0.04em; text-transform: uppercase; }
    .demo-live-indicator > span { width: 6px; height: 6px; border-radius: 50%; background: var(--ob-mint-deep); }
    .source-message { display: grid; grid-template-columns: 34px 1fr auto; gap: 0.7rem; align-items: center; margin-top: 0.6rem; padding: 0.78rem; opacity: 0; border: 1px solid rgba(255,255,255,0.07); border-radius: 13px; background: rgba(255,255,255,0.035); transform: translateY(8px); transition: opacity 240ms ease-out, transform 240ms ease-out, border-color 240ms ease, background 240ms ease; }
    .source-message > span:nth-child(2) { display: grid; min-width: 0; } .source-message strong { overflow: hidden; font-size: 0.72rem; font-weight: 650; text-overflow: ellipsis; white-space: nowrap; }
    .source-message small { overflow: hidden; margin-top: 0.18rem; color: var(--ob-faint); font-size: 0.63rem; text-overflow: ellipsis; white-space: nowrap; }
    .source-avatar { display: grid; place-items: center; width: 34px; height: 34px; border-radius: 50%; background: rgba(255,255,255,0.08); color: var(--ob-muted); font-size: 0.7rem; font-weight: 700; }
    .source-avatar.important { background: rgba(168,222,194,0.12); color: var(--ob-mint); }
    .important-badge { padding: 0.22rem 0.4rem; opacity: 0; border-radius: 999px; background: rgba(168,222,194,0.12); color: var(--ob-mint); font-size: 0.53rem; font-weight: 700; letter-spacing: 0.05em; text-transform: uppercase; transition: opacity 200ms ease; }
    .phase-1 .source-message, .phase-2 .source-message, .phase-3 .source-message, .phase-4 .source-message, .phase-5 .source-message, .phase-6 .source-message, .phase-7 .source-message { opacity: 1; transform: translateY(0); }
    .phase-1 .source-message.first { transition-delay: 0ms; } .phase-1 .source-message.important { transition-delay: 45ms; } .phase-1 .source-message.second { transition-delay: 90ms; }
    .phase-2 .important-message, .phase-3 .important-message, .phase-4 .important-message, .phase-5 .important-message, .phase-6 .important-message, .phase-7 .important-message { border-color: rgba(168,222,194,0.45); background: rgba(168,222,194,0.075); }
    .phase-2 .important-badge, .phase-3 .important-badge, .phase-4 .important-badge, .phase-5 .important-badge, .phase-6 .important-badge, .phase-7 .important-badge { opacity: 1; }
    .phase-3 .quiet-message, .phase-4 .quiet-message, .phase-5 .quiet-message, .phase-6 .quiet-message, .phase-7 .quiet-message { opacity: 0.24; transform: scale(0.985); }
    .source-reply-confirmation { display: flex; align-items: center; justify-content: center; gap: 0.45rem; margin-top: 0.8rem; opacity: 0; color: var(--ob-mint); font-size: 0.66rem; transition: opacity 220ms ease-out; }
    .phase-6 .source-reply-confirmation, .phase-7 .source-reply-confirmation { opacity: 1; }
    .demo-bridge { position: relative; z-index: 2; display: grid; place-items: center; min-height: 330px; }
    .lightfriend-core { position: relative; z-index: 3; display: grid; place-items: center; width: 72px; height: 72px; border: 1px solid rgba(168,222,194,0.3); border-radius: 22px; background: #111a1e; box-shadow: 0 0 0 9px rgba(168,222,194,0.035), 0 15px 40px rgba(0,0,0,0.3); transition: border-color 220ms ease, box-shadow 220ms ease, transform 220ms ease; }
    .lightfriend-core img { width: 34px; height: 34px; object-fit: contain; }
    .phase-2 .lightfriend-core { border-color: rgba(168,222,194,0.72); box-shadow: 0 0 0 12px rgba(168,222,194,0.06), 0 16px 44px rgba(0,0,0,0.3); transform: scale(1.035); }
    .bridge-line { position: absolute; top: calc(50% - 1px); height: 1px; background: linear-gradient(90deg, transparent, rgba(168,222,194,0.32), transparent); }
    .bridge-line-in { right: 50%; width: calc(50% + 92px); } .bridge-line-out { left: 50%; width: calc(50% + 92px); }
    .bridge-pulse { position: absolute; top: calc(50% - 3px); z-index: 4; width: 6px; height: 6px; opacity: 0; border-radius: 50%; background: var(--ob-mint); box-shadow: 0 0 14px rgba(168,222,194,0.8); }
    .phase-2 .bridge-pulse-forward { animation: ob-pulse-forward 850ms ease-in-out both; } .phase-4 .bridge-pulse-forward { animation: ob-pulse-out 850ms ease-in-out both; } .phase-6 .bridge-pulse-return { animation: ob-pulse-return 850ms ease-in-out both; }
    @keyframes ob-pulse-forward { 0% { left: -58%; opacity: 0; } 12% { opacity: 1; } 100% { left: calc(50% - 3px); opacity: 0; } }
    @keyframes ob-pulse-out { 0% { left: calc(50% - 3px); opacity: 0; } 12% { opacity: 1; } 100% { left: 156%; opacity: 0; } }
    @keyframes ob-pulse-return { 0% { left: 156%; opacity: 0; } 12% { opacity: 1; } 100% { left: -58%; opacity: 0; } }
    .decision-copy { position: absolute; top: calc(50% + 52px); display: grid; gap: 0.18rem; width: 160px; text-align: center; }
    .decision-copy span, .decision-copy strong { color: var(--ob-muted); font-size: 0.68rem; font-weight: 600; } .decision-copy strong { color: var(--ob-mint); } .decision-copy small { color: var(--ob-faint); font-size: 0.57rem; }
    .quiet-counter { position: absolute; bottom: 0.45rem; display: grid; justify-items: center; opacity: 0; transform: translateY(5px); transition: opacity 220ms ease-out, transform 220ms ease-out; }
    .quiet-counter strong { color: var(--ob-text); font-size: 1.25rem; font-weight: 560; font-variant-numeric: tabular-nums; } .quiet-counter span { max-width: 120px; color: var(--ob-faint); font-size: 0.55rem; line-height: 1.35; text-align: center; }
    .phase-3 .quiet-counter, .phase-4 .quiet-counter, .phase-5 .quiet-counter, .phase-6 .quiet-counter, .phase-7 .quiet-counter { opacity: 1; transform: translateY(0); }
    .demo-phone-wrap { position: relative; z-index: 1; display: grid; place-items: center; min-height: 430px; }
    .phone-shadow { position: absolute; bottom: 1.2rem; width: 180px; height: 26px; border-radius: 50%; background: rgba(0,0,0,0.5); filter: blur(13px); }
    .demo-phone { position: relative; width: 218px; min-height: 414px; padding: 16px 14px 18px; border: 1px solid rgba(255,255,255,0.18); border-radius: 32px; background: linear-gradient(145deg, #293338, #151d21 72%); box-shadow: inset 0 1px 0 rgba(255,255,255,0.12), inset 0 -8px 20px rgba(0,0,0,0.22), 0 28px 70px rgba(0,0,0,0.34); transform: rotate(1.2deg); }
    .phone-speaker { width: 36px; height: 4px; margin: 0 auto 11px; border-radius: 999px; background: rgba(255,255,255,0.18); }
    .phone-screen { position: relative; height: 225px; overflow: hidden; padding: 0.7rem; border: 1px solid rgba(200,227,212,0.2); border-radius: 12px; background: #d8e2d4; color: #132018; box-shadow: inset 0 2px 9px rgba(37,61,45,0.18); }
    .phone-status { display: flex; justify-content: space-between; color: rgba(19,32,24,0.58); font-size: 0.46rem; font-weight: 750; letter-spacing: 0.06em; }
    .phone-resting-state { position: absolute; inset: 2.2rem 0.8rem 0.8rem; display: grid; place-content: center; justify-items: center; gap: 0.4rem; opacity: 1; text-align: center; transition: opacity 200ms ease; }
    .resting-time { font-size: 2.15rem; font-weight: 520; letter-spacing: -0.07em; } .phone-resting-state small { max-width: 120px; color: rgba(19,32,24,0.62); font-size: 0.56rem; line-height: 1.35; }
    .phase-4 .phone-resting-state, .phase-5 .phone-resting-state, .phase-6 .phone-resting-state, .phase-7 .phone-resting-state { opacity: 0; }
    .phone-notification { position: absolute; inset: 2rem 0.65rem auto; padding: 0.7rem; opacity: 0; border: 1px solid rgba(19,32,24,0.13); border-radius: 9px; background: rgba(247,250,245,0.84); box-shadow: 0 5px 16px rgba(34,55,42,0.12); transform: translateY(7px); transition: opacity 240ms ease-out, transform 240ms ease-out; }
    .phase-4 .phone-notification, .phase-5 .phone-notification { opacity: 1; transform: translateY(0); } .phase-6 .phone-notification, .phase-7 .phone-notification { opacity: 0.4; transform: translateY(-4px); }
    .phone-notification-label { display: flex; align-items: center; gap: 0.35rem; margin-bottom: 0.5rem; color: rgba(19,32,24,0.6); font-size: 0.48rem; font-weight: 700; letter-spacing: 0.035em; text-transform: uppercase; }
    .phone-notification-label img { width: 11px; height: 11px; } .phone-notification > strong { display: block; font-size: 0.65rem; }
    .phone-notification p { margin: 0.25rem 0 0; font-size: 0.58rem; line-height: 1.4; } .phone-notification > small { display: block; margin-top: 0.5rem; color: rgba(19,32,24,0.55); font-size: 0.48rem; }
    .phone-reply { position: absolute; right: 0.7rem; bottom: 0.75rem; display: flex; align-items: end; gap: 0.35rem; max-width: 80%; padding: 0.52rem 0.62rem; opacity: 0; border-radius: 8px 8px 2px 8px; background: #294838; color: #eff8f1; font-size: 0.56rem; transform: translateY(5px); transition: opacity 220ms ease-out, transform 220ms ease-out; }
    .phone-reply i { color: #9fdbbd; font-size: 0.46rem; } .phase-5 .phone-reply, .phase-6 .phone-reply { opacity: 1; transform: translateY(0); } .phase-7 .phone-reply { opacity: 0; }
    .phone-finished { position: absolute; inset: 2.6rem 0.8rem 0.8rem; display: grid; place-content: center; justify-items: center; gap: 0.3rem; opacity: 0; text-align: center; transition: opacity 240ms ease-out; }
    .phone-finished i { display: grid; place-items: center; width: 30px; height: 30px; margin-bottom: 0.3rem; border-radius: 50%; background: #294838; color: #eaf6ee; font-size: 0.7rem; }
    .phone-finished strong { font-size: 0.72rem; } .phone-finished small { color: rgba(19,32,24,0.57); font-size: 0.52rem; }
    .phase-7 .phone-notification, .phase-7 .phone-reply { opacity: 0; } .phase-7 .phone-finished { opacity: 1; }
    .phone-controls { display: grid; grid-template-columns: 1fr 1.15fr 1fr; gap: 9px; margin: 13px 15px 10px; } .phone-controls span { height: 17px; border-radius: 7px; background: rgba(255,255,255,0.09); box-shadow: inset 0 1px 0 rgba(255,255,255,0.08); }
    .phone-controls span:nth-child(2) { border: 1px solid rgba(168,222,194,0.22); background: rgba(168,222,194,0.12); }
    .phone-keypad { display: grid; grid-template-columns: repeat(3, 1fr); gap: 6px 10px; padding: 0 13px; } .phone-keypad span { display: grid; place-items: center; height: 20px; border-radius: 7px; background: rgba(255,255,255,0.055); color: rgba(255,255,255,0.48); font-size: 0.52rem; box-shadow: inset 0 1px 0 rgba(255,255,255,0.06); }
    .demo-caption { display: flex; justify-content: space-between; align-items: center; gap: 1.5rem; margin-top: 1rem; padding: 0 0.4rem; } .demo-caption > div:first-child { display: grid; gap: 0.24rem; }
    .demo-caption strong { font-size: 0.78rem; font-weight: 600; } .demo-caption span { color: var(--ob-faint); font-size: 0.64rem; } .demo-controls { display: flex; gap: 1rem; }
    .preview-actions { margin-top: 1.3rem; }
    .peace-screen { display: grid; grid-template-columns: minmax(320px, 0.8fr) minmax(0, 1fr); gap: clamp(3rem, 8vw, 8rem); align-items: center; min-height: 650px; }
    .peace-visual { position: relative; display: grid; place-items: center; width: min(100%, 470px); aspect-ratio: 1; margin: 0 auto; }
    .peace-ring { position: absolute; border: 1px solid rgba(168,222,194,0.15); border-radius: 50%; } .peace-ring-one { inset: 8%; } .peace-ring-two { inset: 23%; border-color: rgba(168,222,194,0.22); } .peace-ring-three { inset: 37%; border-color: rgba(168,222,194,0.3); background: rgba(168,222,194,0.025); }
    .peace-core { position: relative; z-index: 2; display: grid; place-items: center; width: 76px; height: 76px; border: 1px solid rgba(168,222,194,0.42); border-radius: 24px; background: #111a1e; box-shadow: 0 0 0 15px rgba(168,222,194,0.035), 0 20px 55px rgba(0,0,0,0.32); }
    .peace-core img { width: 36px; height: 36px; }
    .peace-platform { position: absolute; z-index: 3; display: grid; place-items: center; width: 46px; height: 46px; border: 1px solid var(--ob-border); border-radius: 15px; background: #141d21; color: var(--ob-muted); box-shadow: 0 14px 36px rgba(0,0,0,0.25); }
    .peace-platform-1 { top: 11%; left: 28%; } .peace-platform-2 { top: 24%; right: 10%; } .peace-platform-3 { bottom: 13%; right: 25%; } .peace-platform-4 { bottom: 22%; left: 9%; }
    .peace-copy { max-width: 670px; } .peace-lead { max-width: 620px; margin: 1.5rem 0 0; color: var(--ob-muted); font-size: clamp(1.02rem, 1.8vw, 1.22rem); line-height: 1.7; text-wrap: pretty; }
    .peace-statement { display: flex; gap: 0.9rem; align-items: flex-start; margin-top: 2rem; padding: 1.1rem 1.2rem; border: 1px solid rgba(168,222,194,0.22); border-radius: 17px; background: rgba(168,222,194,0.055); }
    .peace-status-dot { flex: 0 0 auto; width: 9px; height: 9px; margin-top: 0.36rem; border-radius: 50%; background: var(--ob-mint); box-shadow: 0 0 0 5px rgba(168,222,194,0.08); }
    .peace-statement > div { display: grid; gap: 0.28rem; } .peace-statement strong { font-size: 0.9rem; font-weight: 650; } .peace-statement small { color: var(--ob-muted); font-size: 0.74rem; line-height: 1.45; }
    .preview-disclaimer { margin: 1rem 0 0; color: var(--ob-faint); font-size: 0.68rem; line-height: 1.5; } .peace-actions { display: flex; justify-content: space-between; align-items: center; gap: 1rem; margin-top: 2rem; }
    .calm-primary { background: var(--ob-mint); color: #102019; } .calm-primary:hover { background: #bcebd0; }
    .plans-heading-row { align-items: start; } .plan-summary { display: grid; gap: 0.35rem; width: min(100%, 310px); padding: 1rem 1.1rem; border: 1px solid rgba(168,222,194,0.2); border-radius: 16px; background: rgba(168,222,194,0.05); }
    .plan-summary > span { color: var(--ob-mint); font-size: 0.6rem; font-weight: 700; letter-spacing: 0.09em; text-transform: uppercase; } .plan-summary strong { font-size: 0.85rem; font-weight: 620; } .plan-summary small { color: var(--ob-muted); font-size: 0.66rem; line-height: 1.45; }
    .onboarding-pricing-shell { min-height: 520px; margin-top: 2.3rem; padding: 1rem; border: 1px solid var(--ob-border); border-radius: 24px; background: rgba(255,255,255,0.035); box-shadow: 0 30px 80px rgba(0,0,0,0.2); }
    .onboarding-pricing-shell .stripe-pricing-table-wrap { width: 100%; min-height: 480px; } .onboarding-pricing-shell stripe-pricing-table { display: block; width: 100%; min-height: 480px; border-radius: 16px; }
    .onboarding-pricing-shell .stripe-pricing-loading, .onboarding-pricing-shell .stripe-pricing-error { display: grid; place-items: center; min-height: 480px; color: var(--ob-muted); text-align: center; }
    .plans-footer { display: flex; justify-content: space-between; align-items: center; gap: 1rem; margin-top: 1rem; } .plans-footer p { display: flex; align-items: center; gap: 0.5rem; margin: 0; color: var(--ob-faint); font-size: 0.66rem; } .plans-footer p i { color: var(--ob-mint); }
    .onboarding-page button:focus-visible, .onboarding-page a:focus-visible { outline: 3px solid var(--ob-mint); outline-offset: 4px; }
    @media (max-width: 980px) {
        .platform-grid { grid-template-columns: repeat(2, minmax(0, 1fr)); } .platform-choice { min-height: 170px; }
        .demo-stage { grid-template-columns: minmax(220px, 1fr) 120px minmax(230px, 0.8fr); } .demo-phone { transform: scale(0.91) rotate(1.2deg); }
        .peace-screen { grid-template-columns: minmax(260px, 0.7fr) minmax(0, 1fr); gap: 3rem; }
    }
    @media (max-width: 760px) {
        .onboarding-page { padding: 5.8rem 1rem 3rem; } .onboarding-progress { grid-template-columns: auto 1fr auto; gap: 1rem; margin-bottom: 2.6rem; } .onboarding-context { font-size: 0.58rem; }
        .screen-heading h1, .peace-copy h1 { font-size: clamp(2.35rem, 12vw, 4.1rem); } .platform-grid { grid-template-columns: 1fr 1fr; margin-top: 2.2rem; }
        .platform-choice { min-height: 158px; padding: 1rem; gap: 1rem; border-radius: 17px; } .platform-choice-icon { width: 42px; height: 42px; border-radius: 12px; } .platform-choice-copy small { font-size: 0.7rem; }
        .priority-panel { grid-template-columns: 1fr; gap: 1rem; } .screen-actions { align-items: flex-end; } .selection-hint { max-width: 45%; }
        .preview-heading-row, .plans-heading-row { display: grid; align-items: start; } .preview-platform-tabs { width: max-content; max-width: 100%; overflow-x: auto; }
        .demo-stage { grid-template-columns: 1fr; min-height: auto; padding: 1rem; } .demo-source-card { min-height: 300px; } .demo-bridge { min-height: 150px; }
        .bridge-line { left: calc(50% - 1px); width: 1px; height: calc(50% + 52px); background: linear-gradient(transparent, rgba(168,222,194,0.32), transparent); }
        .bridge-line-in { top: -56px; right: auto; } .bridge-line-out { top: 50%; } .bridge-pulse { left: calc(50% - 3px); }
        .phase-2 .bridge-pulse-forward, .phase-4 .bridge-pulse-forward, .phase-6 .bridge-pulse-return { animation: none; }
        .decision-copy { top: calc(50% + 47px); } .quiet-counter { right: 0; bottom: 50%; transform: translateY(50%); }
        .phase-3 .quiet-counter, .phase-4 .quiet-counter, .phase-5 .quiet-counter, .phase-6 .quiet-counter, .phase-7 .quiet-counter { transform: translateY(50%); }
        .demo-phone-wrap { min-height: 420px; } .demo-caption { align-items: flex-start; } .demo-controls { flex-direction: column; gap: 0; align-items: flex-end; }
        .peace-screen { grid-template-columns: 1fr; gap: 1.5rem; min-height: auto; } .peace-visual { width: min(84vw, 370px); }
        .peace-copy { text-align: center; } .peace-lead { margin-right: auto; margin-left: auto; } .peace-statement, .preview-disclaimer { text-align: left; } .plan-summary { width: 100%; }
    }
    @media (max-width: 470px) {
        .platform-grid { grid-template-columns: 1fr; } .platform-choice { grid-template-columns: auto 1fr; grid-template-rows: 1fr; min-height: 112px; align-items: center; } .platform-choice-copy { align-content: center; }
        .priority-options { display: grid; grid-template-columns: 1fr; } .priority-chip { width: 100%; } .screen-actions, .peace-actions { flex-wrap: wrap; }
        .screen-actions .onboarding-primary, .peace-actions .onboarding-primary { width: 100%; order: -1; } .selection-hint { max-width: 100%; width: 100%; text-align: center; }
        .preview-actions .onboarding-primary { order: 0; width: auto; } .demo-caption { display: grid; } .demo-controls { flex-direction: row; align-items: center; }
        .plans-footer { align-items: flex-start; } .plans-footer p { max-width: 230px; text-align: right; }
    }
    @media (prefers-reduced-motion: reduce) { .onboarding-page *, .onboarding-page *::before, .onboarding-page *::after { scroll-behavior: auto !important; animation: none !important; transition: none !important; } }
"#;

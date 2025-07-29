use yew::prelude::*;
use gloo_timers::callback::Timeout;

#[function_component(AnimationComponent)]
pub fn animation_component() -> Html {
    let stage = use_state(|| 0u32);

    {
        let stage_clone = stage.clone();
        let stage_setter = stage.setter();
        use_effect(move || {
            let delay = match *stage_clone {
                0 => 0, // Start immediately to stage 1
                1 => 1000, // Luukas shown, wait 0.2s for green arrow
                2 => 1000, // Green arrow shown, wait 0.2s for green flash
                3 => 1000, // Green flash fade in out (slower, 1s), then to notification phone
                4 => 7000, // Hold notification phone for 5s, then reset
                5 => 0, // Reset, immediately to Krister
                6 => 1000, // Krister shown, wait 0.2s for red arrow
                7 => 5000, // Hold red arrow for 5s, then reset
                8 => 0, // Reset, loop back to Luukas
                _ => 0,
            };

            let next_stage = match *stage_clone {
                8 => 1, // Loop back
                _ => *stage_clone + 1,
            };

            let timeout = Timeout::new(delay, move || {
                stage_setter.set(next_stage);
            });
            timeout.forget();

            || () // Cleanup
        });
    }

    let notification = match *stage {
        1 | 2 | 3 | 4 => Some(("Luukas", "Hey I see saw you cycled past me, stop!!")),
        6 | 7 => Some(("Krister", "look this meme is so funny XD")),
        _ => None,
    };

    let middle_img = match *stage {
        2 | 3 | 4 => "green-arrow.png".to_string(),
        7 => "red-arrow.png".to_string(),
        _ => "white-bg.png".to_string(),
    };

    let middle_src = format!("/assets/{}", middle_img);

    let mut middle_class = if middle_img != "white-bg.png" { "fade-in".to_string() } else { "".to_string() };
    if middle_img == "green-arrow.png" {
        middle_class.push_str(" green-arrow");
    } else if middle_img == "red-arrow.png" {
        middle_class.push_str(" red-arrow");
    }

    let middle_style = if middle_img == "white-bg.png" {
        "width: 200px; height: auto; opacity: 0;"
    } else {
        "width: 200px; height: auto;"
    };

    let right_wrapper_class = if *stage == 3 { "green-flash" } else { "" };

    html! {
        <div class="animation-container" style="width: 100%; display: flex; justify-content: space-around; align-items: center; height: 80vh;">
            <style>
                {r#"
                    .block {
                        flex: 1;
                        display: flex;
                        justify-content: center;
                        align-items: center;
                        position: relative;
                    }
                    @keyframes slideIn {
                        from { transform: translateY(-100%); opacity: 0; }
                        to { transform: translateY(0); opacity: 1; }
                    }
                    @keyframes fadeIn {
                        from { opacity: 0; }
                        to { opacity: 1; }
                    }
                    @keyframes greenGlow {
                        0% { opacity: 0; }
                        50% { opacity: 0.8; }
                        100% { opacity: 0; }
                    }
                    .notification {
                        position: relative;
                        width: 360px;
                        background: rgba(26, 26, 26, 0.95);
                        backdrop-filter: blur(10px);
                        border-radius: 40px;
                        padding: 20px;
                        box-shadow: 0 16px 32px rgba(0,0,0,0.3);
                        animation: slideIn 0.8s ease-out forwards;
                        z-index: 10;
                        font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
                        overflow: hidden;
                        border: 1px solid rgba(30, 144, 255, 0.1);
                    }
                    .notification-header {
                        display: flex;
                        align-items: center;
                        margin-bottom: 10px;
                    }
                    .notification-icon {
                        width: 48px;
                        height: 48px;
                        margin-right: 20px;
                        display: flex;
                        align-items: center;
                        justify-content: center;
                    }
                    .notification-icon img {
                        width: 100%;
                        height: 100%;
                    }
                    .notification-title {
                        font-weight: bold;
                        font-size: 28px;
                        color: #fff;
                    }
                    .notification-message {
                        font-size: 28px;
                        color: #ddd;
                    }
                    .fade-in {
                        animation: fadeIn 0.5s ease-in-out;
                    }
                    .green-flash {
                        position: relative;
                    }
                    .green-flash::before {
                        content: '';
                        position: absolute;
                        top: 50%;
                        left: 50%;
                        transform: translate(-50%, -50%);
                        width: 200px;
                        height: 200px;
                        background: rgba(0, 255, 0, 0.5);
                        border-radius: 50%;
                        filter: blur(50px);
                        opacity: 0;
                        animation: greenGlow 1s ease-in-out;
                        z-index: -1;
                    }
                    @media (max-width: 950px) {
                        .animation-container {
                            flex-direction: column;
                        }
                        .block.middle img.green-arrow {
                            transform: rotate(90deg);
                        }
                        .block.middle img.red-arrow {
                            transform: rotate(90deg);
                        }
                    }
                "#}
            </style>
            <div class="block left">
                { if let Some((sender, message)) = notification {
                    html! {
                        <div class="notification">
                            <div class="notification-header">
                                <div class="notification-icon">
                                    <img src="https://upload.wikimedia.org/wikipedia/commons/6/6b/WhatsApp.svg" alt="WhatsApp Logo" />

                                </div>
                                <div class="notification-title">{sender}</div>
                            </div>
                            <div class="notification-message">{message}</div>
                        </div>
                    }
                } else { html! {} } }
            </div>
            <div class="block middle">
                <img key={middle_img.clone()} class={middle_class} src={middle_src} alt="middle" style={middle_style} />
            </div>
            <div class="block right">
                <div class={right_wrapper_class} style="position: relative;">
                    <img src="/assets/empty-phone.png" alt="phone" style="width: 400px; height: auto; opacity: 1; transition: opacity 0.5s ease-in-out;" />
                    <img src="/assets/notification-phone.png" alt="phone" style={format!("position: absolute; top: 0; left: 0; width: 400px; height: auto; opacity: {}; transition: opacity 0.5s ease-in-out;", if *stage >= 3 && *stage <= 6 { "1" } else { "0" })} />
                </div>
            </div>
        </div>
    }
}

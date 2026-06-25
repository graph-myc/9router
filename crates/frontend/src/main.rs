//! 9Router frontend (Leptos CSR): a small Status + Test UI over the backend API.

use gloo_net::http::Request;
use leptos::prelude::*;
use leptos::task::spawn_local;

fn main() {
    console_error_panic_hook::set_once();
    leptos::mount::mount_to_body(App);
}

#[component]
fn App() -> impl IntoView {
    let (tab, set_tab) = signal("status".to_string());
    view! {
        <header style="padding:1rem 1.25rem;border-bottom:1px solid #e5e5e5;font-family:system-ui">
            <h1 style="margin:0;font-size:1.1rem">"9Router — Aggregator " <small style="color:#888">"v0.0.1"</small></h1>
            <nav style="display:flex;gap:.5rem;margin-top:.5rem">
                <button on:click=move |_| set_tab.set("status".to_string())>"Status"</button>
                <button on:click=move |_| set_tab.set("test".to_string())>"Test"</button>
            </nav>
        </header>
        <main style="padding:1.25rem;font-family:system-ui">
            {move || {
                if tab.get() == "test" {
                    view! { <TestView/> }.into_any()
                } else {
                    view! { <StatusView/> }.into_any()
                }
            }}
        </main>
    }
}

#[component]
fn StatusView() -> impl IntoView {
    let (info, set_info) = signal("loading…".to_string());
    spawn_local(async move {
        let version = fetch_text("/version").await;
        let models = fetch_text("/v1/models").await;
        set_info.set(format!("version → {version}\n\nmodels →\n{models}"));
    });
    view! {
        <h2>"Status"</h2>
        <pre style="background:#f6f6f6;padding:1rem;border-radius:8px;white-space:pre-wrap">
            {move || info.get()}
        </pre>
    }
}

#[component]
fn TestView() -> impl IntoView {
    let (model, set_model) = signal(String::new());
    let (prompt, set_prompt) = signal(String::new());
    let (out, set_out) = signal(String::new());

    let send = move |_| {
        let model = model.get();
        let prompt = prompt.get();
        set_out.set("…".to_string());
        spawn_local(async move {
            let body = serde_json::json!({
                "model": model,
                "messages": [{ "role": "user", "content": prompt }],
                "stream": false
            });
            let result = match Request::post("/v1/chat/completions").json(&body) {
                Ok(req) => match req.send().await {
                    Ok(resp) => resp.text().await.unwrap_or_default(),
                    Err(e) => format!("network error: {e}"),
                },
                Err(e) => format!("request error: {e}"),
            };
            set_out.set(result);
        });
    };

    view! {
        <h2>"Test a model / combo"</h2>
        <div style="display:flex;flex-direction:column;gap:.5rem;max-width:640px">
            <input
                placeholder="model or combo (e.g. openai/gpt-4o-mini, or a combo name)"
                on:input=move |e| set_model.set(event_target_value(&e))
                prop:value=move || model.get()
            />
            <textarea
                rows="4"
                placeholder="prompt"
                on:input=move |e| set_prompt.set(event_target_value(&e))
                prop:value=move || prompt.get()
            ></textarea>
            <button on:click=send>"Send"</button>
        </div>
        <pre style="background:#f6f6f6;padding:1rem;border-radius:8px;white-space:pre-wrap;margin-top:1rem">
            {move || out.get()}
        </pre>
    }
}

async fn fetch_text(url: &str) -> String {
    match Request::get(url).send().await {
        Ok(r) => r.text().await.unwrap_or_default(),
        Err(e) => format!("error: {e}"),
    }
}

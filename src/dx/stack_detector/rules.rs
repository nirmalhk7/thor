use super::TechBadge;

type BadgeSpec = (&'static str, &'static str, &'static str);
type Rule = (&'static [&'static str], BadgeSpec);

const UI_FRAMEWORKS: &[Rule] = &[
    (
        &["__svelte", ".svelte", "sveltekit"],
        ("SvelteKit / Svelte", "Framework", "🧡"),
    ),
    (
        &["ng-version", "ng-app", "angular"],
        ("Angular", "Framework", "🅰️"),
    ),
];

const ALTERNATIVE_FRAMEWORKS: &[Rule] = &[
    (&["astro", "data-astro-cid"], ("Astro", "Framework", "🚀")),
    (&["__remix", "remix-context"], ("Remix", "Framework", "💿")),
    (&["solid-js", "data-solid"], ("SolidJS", "UI", "🔷")),
    (&["qwik", "q:container"], ("Qwik", "Framework", "⚡")),
];

const LIBRARIES: &[Rule] = &[
    (&["typescript", ".ts"], ("TypeScript", "Language", "🔷")),
    (&["htmx", "hx-get", "hx-post"], ("htmx", "Library", "⚡")),
    (&["alpine", "x-data"], ("Alpine.js", "Library", "🏔️")),
    (&["jquery", "jquery.min.js"], ("jQuery", "Library", "💲")),
];

const PLATFORMS: &[Rule] = &[
    (&["shopify", "cdn.shopify.com"], ("Shopify", "SaaS", "🛍️")),
    (
        &["webflow", "uploads-ssl.webflow.com"],
        ("Webflow", "SaaS", "🎨"),
    ),
    (&["squarespace"], ("Squarespace", "SaaS", "⬛")),
    (&["wix.com", "wixstatic.com"], ("Wix", "SaaS", "✨")),
    (
        &["wordpress", "wp-content"],
        ("WordPress (PHP)", "CMS", "📝"),
    ),
    (&["hugo"], ("Hugo (Go)", "SSG", "🐹")),
    (&["jekyll"], ("Jekyll (Ruby)", "SSG", "💎")),
    (&["ghost"], ("Ghost (Node)", "CMS", "👻")),
];

const SERVICES: &[Rule] = &[
    (
        &["googletagmanager.com", "gtm.js"],
        ("Google Tag Manager", "Analytics", "📊"),
    ),
    (
        &["segment.com", "analytics.js"],
        ("Segment", "Analytics", "📈"),
    ),
    (&["mixpanel"], ("Mixpanel", "Analytics", "🟣")),
    (&["posthog"], ("PostHog", "Analytics", "🦔")),
    (
        &["intercom", "widget.intercom.io"],
        ("Intercom", "Support", "💬"),
    ),
    (
        &["stripe.com", "js.stripe.com"],
        ("Stripe", "Payments", "💳"),
    ),
    (&["supabase"], ("Supabase", "Database", "⚡")),
    (&["firebase", "firestore"], ("Firebase", "Backend", "🔥")),
    (&["graphql", "__schema"], ("GraphQL", "API", "🕸️")),
];

const SERVERS: &[Rule] = &[
    (&["cloudflare"], ("Cloudflare CDN", "Infra", "☁️")),
    (&["nginx"], ("Nginx", "Server", "🟢")),
    (&["apache"], ("Apache", "Server", "🪶")),
    (&["caddy"], ("Caddy", "Server", "🔒")),
];

const RUNTIMES: &[Rule] = &[
    (
        &["gunicorn", "uvicorn", "django"],
        ("Python (Django)", "Backend", "🐍"),
    ),
    (&["puma", "phusion"], ("Ruby on Rails", "Backend", "💎")),
    (
        &["express", "node", "vercel"],
        ("Node.js / Vercel", "Runtime", "🟢"),
    ),
];

pub(super) fn detect(
    html_snippet: &str,
    html_lower: &str,
    url_lower: &str,
    server_header: Option<&str>,
    x_powered_by: Option<&str>,
) -> Vec<TechBadge> {
    let mut badges = Vec::new();
    detect_frameworks(html_lower, url_lower, x_powered_by, &mut badges);
    detect_languages_and_styles(html_snippet, html_lower, url_lower, &mut badges);
    append_all(html_lower, LIBRARIES, &mut badges);
    append_first(html_lower, PLATFORMS, &mut badges);
    append_all(html_lower, SERVICES, &mut badges);
    detect_headers(server_header, x_powered_by, &mut badges);
    badges
}

fn detect_frameworks(
    html_lower: &str,
    url_lower: &str,
    x_powered_by: Option<&str>,
    badges: &mut Vec<TechBadge>,
) {
    // 1. Next.js & React
    if html_lower.contains("__next_data__")
        || html_lower.contains("_next/static")
        || url_lower.contains("nextjs.org")
        || x_powered_by.is_some_and(|s| s.to_lowercase().contains("next.js"))
    {
        badges.push(badge(("Next.js", "Framework", "▲")));
        badges.push(badge(("React 18", "UI", "⚛️")));
    } else if html_lower.contains("react") || html_lower.contains("data-reactroot") {
        badges.push(badge(("React", "UI", "⚛️")));
    }

    // 2. Vue.js & Nuxt
    if html_lower.contains("__nuxt") || html_lower.contains("_nuxt/") {
        badges.push(badge(("Nuxt", "Framework", "💚")));
        badges.push(badge(("Vue.js", "UI", "🟢")));
    } else if html_lower.contains("data-v-") || html_lower.contains("vue") {
        badges.push(badge(("Vue.js", "UI", "🟢")));
    }

    append_all(html_lower, UI_FRAMEWORKS, badges);
    append_first(html_lower, ALTERNATIVE_FRAMEWORKS, badges);
}

fn detect_languages_and_styles(
    html_snippet: &str,
    html_lower: &str,
    url_lower: &str,
    badges: &mut Vec<TechBadge>,
) {
    // 6. Rust & WebAssembly
    if contains_any(html_lower, &[".wasm", "wasm_bindgen", "yew", "leptos"])
        || url_lower.contains("docs.rs")
    {
        badges.push(badge(("Rust (Wasm)", "Language", "🦀")));
    }

    // 7. Tailwind CSS & Bootstrap & UI Libraries
    if html_lower.contains("tailwindcss")
        || html_lower.contains("tailwind")
        || html_snippet.contains("flex items-center")
    {
        badges.push(badge(("Tailwind CSS", "Styling", "🎨")));
    } else if html_lower.contains("bootstrap") {
        badges.push(badge(("Bootstrap", "Styling", "🟣")));
    }
}

fn detect_headers(
    server_header: Option<&str>,
    x_powered_by: Option<&str>,
    badges: &mut Vec<TechBadge>,
) {
    if let Some(srv) = server_header {
        let srv_lower = srv.to_lowercase();
        append_first(&srv_lower, SERVERS, badges);
        append_first(&srv_lower, RUNTIMES, badges);
    }
    if let Some(pow) = x_powered_by {
        let pow_lower = pow.to_lowercase();
        if pow_lower.contains("express") {
            if !badges.iter().any(|b| b.name.contains("Node.js")) {
                badges.push(badge(("Node.js / Express", "Backend", "🟢")));
            }
        } else if pow_lower.contains("php") || pow_lower.contains("laravel") {
            if !badges.iter().any(|b| b.name.contains("WordPress")) {
                badges.push(badge(("PHP / Laravel", "Backend", "🐘")));
            }
        } else if pow_lower.contains("asp.net") {
            badges.push(badge(("ASP.NET (.NET)", "Backend", "🔷")));
        }
    }
}

fn contains_any(text: &str, patterns: &[&str]) -> bool {
    patterns.iter().any(|pattern| text.contains(pattern))
}

fn append_first(text: &str, rules: &[Rule], badges: &mut Vec<TechBadge>) {
    if let Some((_, details)) = rules
        .iter()
        .find(|(patterns, _)| contains_any(text, patterns))
    {
        badges.push(badge(*details));
    }
}

fn append_all(text: &str, rules: &[Rule], badges: &mut Vec<TechBadge>) {
    for (_, details) in rules
        .iter()
        .filter(|(patterns, _)| contains_any(text, patterns))
    {
        badges.push(badge(*details));
    }
}

fn badge((name, category, icon): BadgeSpec) -> TechBadge {
    TechBadge {
        name: name.into(),
        category: category.into(),
        icon: icon.into(),
    }
}

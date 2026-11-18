use super::orchestrator::{
    ai_inspection, ai_metrics, ai_models, ai_prompts, ai_quality, backend_analysis, backend_api,
    backend_capture, backend_http, backend_labs, backend_workflows, frontend_assets, frontend_css,
    frontend_dom, frontend_interaction, frontend_layout, perf_capture, perf_lifecycle,
    perf_network, perf_profiles, perf_vitals, qa_execution, qa_exports, qa_policy, qa_tests,
    security_analysis, security_auth, security_evidence, security_fuzz, security_labs,
    security_web,
};
use super::{FeatureCategory, FeatureStatus};

type FeatureHandler =
    fn(&super::FeatureEngine, &serde_json::Value) -> Result<serde_json::Value, String>;

pub(super) struct FeatureMetadata {
    pub(super) slug: &'static str,
    pub(super) name: &'static str,
    pub(super) category: FeatureCategory,
    pub(super) status: FeatureStatus,
    pub(super) handler: FeatureHandler,
}

// Indexed by the unchanged FeatureId discriminants / FeatureId::ALL order.
pub(super) const FEATURES: [FeatureMetadata; 144] = [
    // ComponentTreeLens
    FeatureMetadata {
        slug: "component-tree-lens",
        name: "Component Tree Lens",
        category: FeatureCategory::Frontend,
        status: FeatureStatus::Stable,
        handler: frontend_dom::component_tree_lens,
    },
    // ResponsiveMatrix
    FeatureMetadata {
        slug: "responsive-matrix",
        name: "Responsive Matrix",
        category: FeatureCategory::Frontend,
        status: FeatureStatus::Stable,
        handler: frontend_layout::responsive_matrix,
    },
    // CssCascade
    FeatureMetadata {
        slug: "css-cascade",
        name: "CSS Cascade Inspector",
        category: FeatureCategory::Frontend,
        status: FeatureStatus::Stable,
        handler: frontend_css::css_cascade,
    },
    // DesignTokens
    FeatureMetadata {
        slug: "design-tokens",
        name: "Design Token Extractor",
        category: FeatureCategory::Frontend,
        status: FeatureStatus::Stable,
        handler: frontend_css::design_tokens,
    },
    // A11yLens
    FeatureMetadata {
        slug: "a11y-lens",
        name: "Accessibility Lens",
        category: FeatureCategory::Frontend,
        status: FeatureStatus::Stable,
        handler: frontend_dom::a11y_lens,
    },
    // StateTimeTravel
    FeatureMetadata {
        slug: "state-timetravel",
        name: "State Time-Travel",
        category: FeatureCategory::Frontend,
        status: FeatureStatus::Stable,
        handler: frontend_interaction::state_time_travel,
    },
    // ClsHeatmap
    FeatureMetadata {
        slug: "cls-heatmap",
        name: "CLS Heatmap",
        category: FeatureCategory::Frontend,
        status: FeatureStatus::Stable,
        handler: frontend_layout::cls_heatmap,
    },
    // ComponentSandbox
    FeatureMetadata {
        slug: "component-sandbox",
        name: "Component Sandbox",
        category: FeatureCategory::Frontend,
        status: FeatureStatus::Stable,
        handler: frontend_interaction::component_sandbox,
    },
    // NetworkComponentBinding
    FeatureMetadata {
        slug: "network-component-binding",
        name: "Network↔Component Binding",
        category: FeatureCategory::Frontend,
        status: FeatureStatus::Stable,
        handler: frontend_interaction::network_component_binding,
    },
    // FontAuditor
    FeatureMetadata {
        slug: "font-auditor",
        name: "Font Auditor",
        category: FeatureCategory::Frontend,
        status: FeatureStatus::Stable,
        handler: frontend_assets::font_auditor,
    },
    // ZIndexVisualizer
    FeatureMetadata {
        slug: "zindex-visualizer",
        name: "Z-Index Visualizer",
        category: FeatureCategory::Frontend,
        status: FeatureStatus::Stable,
        handler: frontend_dom::z_index_visualizer,
    },
    // HydrationDiffLens
    FeatureMetadata {
        slug: "hydration-diff-lens",
        name: "Hydration Diff Lens",
        category: FeatureCategory::Frontend,
        status: FeatureStatus::Beta,
        handler: frontend_dom::hydration_diff_lens,
    },
    // CssCoverageMap
    FeatureMetadata {
        slug: "css-coverage-map",
        name: "CSS Coverage Map",
        category: FeatureCategory::Frontend,
        status: FeatureStatus::Beta,
        handler: frontend_css::css_coverage_map,
    },
    // FocusOrderOverlay
    FeatureMetadata {
        slug: "focus-order-overlay",
        name: "Focus Order Overlay",
        category: FeatureCategory::Frontend,
        status: FeatureStatus::Beta,
        handler: frontend_interaction::focus_order_overlay,
    },
    // EventListenerMap
    FeatureMetadata {
        slug: "event-listener-map",
        name: "Event Listener Map",
        category: FeatureCategory::Frontend,
        status: FeatureStatus::Beta,
        handler: frontend_interaction::event_listener_map,
    },
    // CssVariableInspector
    FeatureMetadata {
        slug: "css-variable-inspector",
        name: "CSS Variable Inspector",
        category: FeatureCategory::Frontend,
        status: FeatureStatus::Beta,
        handler: frontend_css::css_variable_inspector,
    },
    // LayoutThrashDetector
    FeatureMetadata {
        slug: "layout-thrash-detector",
        name: "Layout Thrash Detector",
        category: FeatureCategory::Frontend,
        status: FeatureStatus::Beta,
        handler: frontend_layout::layout_thrash_detector,
    },
    // MediaCqMatrix
    FeatureMetadata {
        slug: "media-cq-matrix",
        name: "Media / Container Query Matrix",
        category: FeatureCategory::Frontend,
        status: FeatureStatus::Beta,
        handler: frontend_layout::media_cq_matrix,
    },
    // ImageDeliveryAuditor
    FeatureMetadata {
        slug: "image-delivery-auditor",
        name: "Image Delivery Auditor",
        category: FeatureCategory::Frontend,
        status: FeatureStatus::Beta,
        handler: frontend_assets::image_delivery_auditor,
    },
    // CssContainmentAdvisor
    FeatureMetadata {
        slug: "css-containment-advisor",
        name: "CSS Containment Advisor",
        category: FeatureCategory::Frontend,
        status: FeatureStatus::Beta,
        handler: frontend_css::css_containment_advisor,
    },
    // AriaLiveMonitor
    FeatureMetadata {
        slug: "aria-live-monitor",
        name: "ARIA Live Monitor",
        category: FeatureCategory::Frontend,
        status: FeatureStatus::Beta,
        handler: frontend_interaction::aria_live_monitor,
    },
    // SelectorSpecificityBudget
    FeatureMetadata {
        slug: "selector-specificity-budget",
        name: "Selector Specificity Budget",
        category: FeatureCategory::Frontend,
        status: FeatureStatus::Beta,
        handler: frontend_css::selector_specificity_budget,
    },
    // DomSnapshotCapture
    FeatureMetadata {
        slug: "dom-snapshot-capture",
        name: "DOM Snapshot Capture",
        category: FeatureCategory::Frontend,
        status: FeatureStatus::Experimental,
        handler: frontend_dom::dom_snapshot_capture,
    },
    // ApiWorkbench
    FeatureMetadata {
        slug: "api-workbench",
        name: "API Workbench",
        category: FeatureCategory::Backend,
        status: FeatureStatus::Stable,
        handler: backend_api::api_workbench,
    },
    // AuthVault
    FeatureMetadata {
        slug: "auth-vault",
        name: "Auth Vault",
        category: FeatureCategory::Backend,
        status: FeatureStatus::Stable,
        handler: backend_api::auth_vault,
    },
    // ContractValidation
    FeatureMetadata {
        slug: "contract-validation",
        name: "Contract Validation",
        category: FeatureCategory::Backend,
        status: FeatureStatus::Stable,
        handler: backend_api::contract_validation,
    },
    // WebhookSink
    FeatureMetadata {
        slug: "webhook-sink",
        name: "Webhook Sink",
        category: FeatureCategory::Backend,
        status: FeatureStatus::Stable,
        handler: backend_capture::webhook_sink,
    },
    // IdorMatrix
    FeatureMetadata {
        slug: "idor-matrix",
        name: "IDOR Matrix",
        category: FeatureCategory::Backend,
        status: FeatureStatus::Stable,
        handler: backend_capture::idor_matrix,
    },
    // RequestChaining
    FeatureMetadata {
        slug: "request-chaining",
        name: "Request Chaining",
        category: FeatureCategory::Backend,
        status: FeatureStatus::Stable,
        handler: backend_api::request_chaining,
    },
    // GrpcGraphql
    FeatureMetadata {
        slug: "grpc-graphql",
        name: "gRPC / GraphQL Inspector",
        category: FeatureCategory::Backend,
        status: FeatureStatus::Stable,
        handler: backend_analysis::grpc_graphql,
    },
    // TraceAdjacency
    FeatureMetadata {
        slug: "trace-adjacency",
        name: "Trace Adjacency",
        category: FeatureCategory::Backend,
        status: FeatureStatus::Stable,
        handler: backend_capture::trace_adjacency,
    },
    // MockStub
    FeatureMetadata {
        slug: "mock-stub",
        name: "Mock / Stub Server",
        category: FeatureCategory::Backend,
        status: FeatureStatus::Stable,
        handler: backend_capture::mock_stub,
    },
    // ChaosInjection
    FeatureMetadata {
        slug: "chaos-injection",
        name: "Chaos Injection",
        category: FeatureCategory::Backend,
        status: FeatureStatus::Experimental,
        handler: backend_analysis::chaos_injection,
    },
    // IdempotencyAuditor
    FeatureMetadata {
        slug: "idempotency-auditor",
        name: "Idempotency Auditor",
        category: FeatureCategory::Backend,
        status: FeatureStatus::Stable,
        handler: backend_analysis::idempotency_auditor,
    },
    // DbAdjacency
    FeatureMetadata {
        slug: "db-adjacency",
        name: "DB Adjacency",
        category: FeatureCategory::Backend,
        status: FeatureStatus::Stable,
        handler: backend_analysis::db_adjacency,
    },
    // WebhookSignatureVault
    FeatureMetadata {
        slug: "webhook-signature-vault",
        name: "Webhook Signature Vault",
        category: FeatureCategory::Backend,
        status: FeatureStatus::Beta,
        handler: backend_analysis::webhook_signature_vault,
    },
    // RateLimitProbe
    FeatureMetadata {
        slug: "rate-limit-probe",
        name: "Rate Limit Probe",
        category: FeatureCategory::Backend,
        status: FeatureStatus::Beta,
        handler: backend_http::rate_limit_probe,
    },
    // PaginationAuditor
    FeatureMetadata {
        slug: "pagination-auditor",
        name: "Pagination Auditor",
        category: FeatureCategory::Backend,
        status: FeatureStatus::Beta,
        handler: backend_http::pagination_auditor,
    },
    // CacheSemanticsLens
    FeatureMetadata {
        slug: "cache-semantics-lens",
        name: "Cache Semantics Lens",
        category: FeatureCategory::Backend,
        status: FeatureStatus::Beta,
        handler: backend_http::cache_semantics_lens,
    },
    // MultipartWorkbench
    FeatureMetadata {
        slug: "multipart-workbench",
        name: "Multipart Analyzer (offline)",
        category: FeatureCategory::Backend,
        status: FeatureStatus::Experimental,
        handler: backend_labs::multipart_workbench,
    },
    // SseWsServerLab
    FeatureMetadata {
        slug: "sse-ws-server-lab",
        name: "SSE Server Lab",
        category: FeatureCategory::Backend,
        status: FeatureStatus::Experimental,
        handler: backend_labs::sse_ws_server_lab,
    },
    // OpenApiDiffGate
    FeatureMetadata {
        slug: "openapi-diff-gate",
        name: "OpenAPI Diff Gate",
        category: FeatureCategory::Backend,
        status: FeatureStatus::Beta,
        handler: backend_http::open_api_diff_gate,
    },
    // OauthFlowCapture
    FeatureMetadata {
        slug: "oauth-flow-capture",
        name: "OAuth Flow Capture",
        category: FeatureCategory::Backend,
        status: FeatureStatus::Beta,
        handler: backend_workflows::oauth_flow_capture,
    },
    // QueueAdjacency
    FeatureMetadata {
        slug: "queue-adjacency",
        name: "Queue Adjacency",
        category: FeatureCategory::Backend,
        status: FeatureStatus::Beta,
        handler: backend_workflows::queue_adjacency,
    },
    // GrpcStreamWorkbench
    FeatureMetadata {
        slug: "grpc-stream-workbench",
        name: "gRPC Stream Analyzer (offline)",
        category: FeatureCategory::Backend,
        status: FeatureStatus::Experimental,
        handler: backend_workflows::grpc_stream_workbench,
    },
    // LiveStubServer
    FeatureMetadata {
        slug: "live-stub-server",
        name: "Live Stub Server",
        category: FeatureCategory::Backend,
        status: FeatureStatus::Experimental,
        handler: backend_labs::live_stub_server,
    },
    // HeaderContractFuzzer
    FeatureMetadata {
        slug: "header-contract-fuzzer",
        name: "Header Contract Fuzzer",
        category: FeatureCategory::Backend,
        status: FeatureStatus::Beta,
        handler: backend_labs::header_contract_fuzzer,
    },
    // AuthzMatrix
    FeatureMetadata {
        slug: "authz-matrix",
        name: "Authorization Matrix",
        category: FeatureCategory::Security,
        status: FeatureStatus::Stable,
        handler: security_auth::authz_matrix,
    },
    // ResponseDiff
    FeatureMetadata {
        slug: "response-diff",
        name: "Response Diff",
        category: FeatureCategory::Security,
        status: FeatureStatus::Stable,
        handler: security_analysis::response_diff,
    },
    // HeaderAnalyzer
    FeatureMetadata {
        slug: "header-analyzer",
        name: "Security Header Analyzer",
        category: FeatureCategory::Security,
        status: FeatureStatus::Stable,
        handler: security_analysis::header_analyzer,
    },
    // JwtDeep
    FeatureMetadata {
        slug: "jwt-deep",
        name: "JWT Deep Inspector",
        category: FeatureCategory::Security,
        status: FeatureStatus::Stable,
        handler: security_auth::jwt_deep,
    },
    // PromoteFinding
    FeatureMetadata {
        slug: "promote-finding",
        name: "Promote to Finding",
        category: FeatureCategory::Security,
        status: FeatureStatus::Stable,
        handler: security_evidence::promote_finding,
    },
    // SurfaceCrawler
    FeatureMetadata {
        slug: "surface-crawler",
        name: "Attack Surface Crawler",
        category: FeatureCategory::Security,
        status: FeatureStatus::Stable,
        handler: security_analysis::surface_crawler,
    },
    // StorageAuditor
    FeatureMetadata {
        slug: "storage-auditor",
        name: "Storage Auditor",
        category: FeatureCategory::Security,
        status: FeatureStatus::Stable,
        handler: security_analysis::storage_auditor,
    },
    // FuzzRepeater
    FeatureMetadata {
        slug: "fuzz-repeater",
        name: "Fuzz Repeater",
        category: FeatureCategory::Security,
        status: FeatureStatus::Stable,
        handler: security_fuzz::fuzz_repeater,
    },
    // CsrfAnalyzer
    FeatureMetadata {
        slug: "csrf-analyzer",
        name: "CSRF Analyzer",
        category: FeatureCategory::Security,
        status: FeatureStatus::Stable,
        handler: security_fuzz::csrf_analyzer,
    },
    // SinkPollution
    FeatureMetadata {
        slug: "sink-pollution",
        name: "Sink / Pollution Detector",
        category: FeatureCategory::Security,
        status: FeatureStatus::Beta,
        handler: security_fuzz::sink_pollution,
    },
    // TlsInspector
    FeatureMetadata {
        slug: "tls-inspector",
        name: "TLS Inspector",
        category: FeatureCategory::Security,
        status: FeatureStatus::Stable,
        handler: security_fuzz::tls_inspector,
    },
    // OobListener
    FeatureMetadata {
        slug: "oob-listener",
        name: "Out-of-Band Listener",
        category: FeatureCategory::Security,
        status: FeatureStatus::Experimental,
        handler: security_labs::oob_listener,
    },
    // DomXssTaintMap
    FeatureMetadata {
        slug: "dom-xss-taint-map",
        name: "DOM XSS Taint Analyzer (offline)",
        category: FeatureCategory::Security,
        status: FeatureStatus::Beta,
        handler: security_web::dom_xss_taint_map,
    },
    // CorsDeepProbe
    FeatureMetadata {
        slug: "cors-deep-probe",
        name: "CORS Deep Probe",
        category: FeatureCategory::Security,
        status: FeatureStatus::Beta,
        handler: security_web::cors_deep_probe,
    },
    // CspPolicyForensics
    FeatureMetadata {
        slug: "csp-policy-forensics",
        name: "CSP Policy Forensics",
        category: FeatureCategory::Security,
        status: FeatureStatus::Beta,
        handler: security_web::csp_policy_forensics,
    },
    // OauthOidcMisconfig
    FeatureMetadata {
        slug: "oauth-oidc-misconfig",
        name: "OAuth / OIDC Misconfig",
        category: FeatureCategory::Security,
        status: FeatureStatus::Beta,
        handler: security_web::oauth_oidc_misconfig,
    },
    // CookieSessionForensics
    FeatureMetadata {
        slug: "cookie-session-forensics",
        name: "Cookie Session Forensics",
        category: FeatureCategory::Security,
        status: FeatureStatus::Beta,
        handler: security_web::cookie_session_forensics,
    },
    // SessionFixationLab
    FeatureMetadata {
        slug: "session-fixation-lab",
        name: "Session Fixation Lab",
        category: FeatureCategory::Security,
        status: FeatureStatus::Experimental,
        handler: security_labs::session_fixation_lab,
    },
    // GraphqlAuthzMatrix
    FeatureMetadata {
        slug: "graphql-authz-matrix",
        name: "GraphQL Authz Matrix",
        category: FeatureCategory::Security,
        status: FeatureStatus::Beta,
        handler: security_auth::graphql_authz_matrix,
    },
    // MassAssignmentDiff
    FeatureMetadata {
        slug: "mass-assignment-diff",
        name: "Mass Assignment Diff",
        category: FeatureCategory::Security,
        status: FeatureStatus::Beta,
        handler: security_evidence::mass_assignment_diff,
    },
    // RaceWindowRepeater
    FeatureMetadata {
        slug: "race-window-repeater",
        name: "Race Window Analyzer",
        category: FeatureCategory::Security,
        status: FeatureStatus::Experimental,
        handler: security_labs::race_window_repeater,
    },
    // SsrfLabHarness
    FeatureMetadata {
        slug: "ssrf-lab-harness",
        name: "SSRF Lab Harness",
        category: FeatureCategory::Security,
        status: FeatureStatus::Experimental,
        handler: security_labs::ssrf_lab_harness,
    },
    // SriSupplyChainAuditor
    FeatureMetadata {
        slug: "sri-supply-chain-auditor",
        name: "SRI Supply Chain Auditor",
        category: FeatureCategory::Security,
        status: FeatureStatus::Beta,
        handler: security_evidence::sri_supply_chain_auditor,
    },
    // WebsocketAuthzProbe
    FeatureMetadata {
        slug: "websocket-authz-probe",
        name: "WebSocket Authz Probe",
        category: FeatureCategory::Security,
        status: FeatureStatus::Beta,
        handler: security_auth::websocket_authz_probe,
    },
    // PerfBudgets
    FeatureMetadata {
        slug: "perf-budgets",
        name: "Performance Budgets",
        category: FeatureCategory::Performance,
        status: FeatureStatus::Stable,
        handler: perf_vitals::perf_budgets,
    },
    // TraceDiff
    FeatureMetadata {
        slug: "trace-diff",
        name: "Trace Diff",
        category: FeatureCategory::Performance,
        status: FeatureStatus::Stable,
        handler: perf_capture::trace_diff,
    },
    // WaterfallForensics
    FeatureMetadata {
        slug: "waterfall-forensics",
        name: "Waterfall Forensics",
        category: FeatureCategory::Performance,
        status: FeatureStatus::Stable,
        handler: perf_capture::waterfall_forensics,
    },
    // MemoryLeakHunter
    FeatureMetadata {
        slug: "memory-leak-hunter",
        name: "Memory Leak Hunter",
        category: FeatureCategory::Performance,
        status: FeatureStatus::Beta,
        handler: perf_capture::memory_leak_hunter,
    },
    // SyntheticMonitor
    FeatureMetadata {
        slug: "synthetic-monitor",
        name: "Synthetic Monitor",
        category: FeatureCategory::Performance,
        status: FeatureStatus::Stable,
        handler: perf_lifecycle::synthetic_monitor,
    },
    // RumLab
    FeatureMetadata {
        slug: "rum-lab",
        name: "RUM vs Lab",
        category: FeatureCategory::Performance,
        status: FeatureStatus::Stable,
        handler: perf_vitals::rum_lab,
    },
    // HdrHistogramHelper
    FeatureMetadata {
        slug: "hdr-histogram-helper",
        name: "HDR Histogram Helper",
        category: FeatureCategory::Performance,
        status: FeatureStatus::Stable,
        handler: perf_profiles::hdr_histogram_helper,
    },
    // InpAttribution
    FeatureMetadata {
        slug: "inp-attribution",
        name: "INP Attribution",
        category: FeatureCategory::Performance,
        status: FeatureStatus::Stable,
        handler: perf_vitals::inp_attribution,
    },
    // CdnCache
    FeatureMetadata {
        slug: "cdn-cache",
        name: "CDN Cache Analyzer",
        category: FeatureCategory::Performance,
        status: FeatureStatus::Stable,
        handler: perf_capture::cdn_cache,
    },
    // VitalsUnderLoad
    FeatureMetadata {
        slug: "vitals-under-load",
        name: "Vitals Under Load",
        category: FeatureCategory::Performance,
        status: FeatureStatus::Stable,
        handler: perf_vitals::vitals_under_load,
    },
    // FlameGraph
    FeatureMetadata {
        slug: "flame-graph",
        name: "Flame Graph",
        category: FeatureCategory::Performance,
        status: FeatureStatus::Experimental,
        handler: perf_profiles::flame_graph,
    },
    // ThirdPartyAttribution
    FeatureMetadata {
        slug: "third-party-attribution",
        name: "Third-Party Attribution",
        category: FeatureCategory::Performance,
        status: FeatureStatus::Beta,
        handler: perf_capture::third_party_attribution,
    },
    // LcpElementForensics
    FeatureMetadata {
        slug: "lcp-element-forensics",
        name: "LCP Element Forensics",
        category: FeatureCategory::Performance,
        status: FeatureStatus::Beta,
        handler: perf_vitals::lcp_element_forensics,
    },
    // LongTaskRadar
    FeatureMetadata {
        slug: "long-task-radar",
        name: "Long Task Radar",
        category: FeatureCategory::Performance,
        status: FeatureStatus::Beta,
        handler: perf_profiles::long_task_radar,
    },
    // MainThreadBlockMap
    FeatureMetadata {
        slug: "main-thread-block-map",
        name: "Main-Thread Block Map",
        category: FeatureCategory::Performance,
        status: FeatureStatus::Beta,
        handler: perf_profiles::main_thread_block_map,
    },
    // BfCacheProbe
    FeatureMetadata {
        slug: "bfcache-probe",
        name: "bfcache Probe",
        category: FeatureCategory::Performance,
        status: FeatureStatus::Beta,
        handler: perf_lifecycle::bf_cache_probe,
    },
    // FreezeResumeBench
    FeatureMetadata {
        slug: "freeze-resume-bench",
        name: "Freeze / Resume Bench",
        category: FeatureCategory::Performance,
        status: FeatureStatus::Experimental,
        handler: perf_lifecycle::freeze_resume_bench,
    },
    // SoftNavVitals
    FeatureMetadata {
        slug: "soft-nav-vitals",
        name: "Soft-Nav Vitals (experimental)",
        category: FeatureCategory::Performance,
        status: FeatureStatus::Experimental,
        handler: perf_vitals::soft_nav_vitals,
    },
    // SpeculationLab
    FeatureMetadata {
        slug: "speculation-lab",
        name: "Speculation Lab",
        category: FeatureCategory::Performance,
        status: FeatureStatus::Experimental,
        handler: perf_lifecycle::speculation_lab,
    },
    // ConnPoolInspector
    FeatureMetadata {
        slug: "conn-pool-inspector",
        name: "Connection Pool Inspector",
        category: FeatureCategory::Performance,
        status: FeatureStatus::Beta,
        handler: perf_network::conn_pool_inspector,
    },
    // Http3PathAudit
    FeatureMetadata {
        slug: "http3-path-audit",
        name: "HTTP/3 Path Audit",
        category: FeatureCategory::Performance,
        status: FeatureStatus::Beta,
        handler: perf_network::http3_path_audit,
    },
    // CompressionBudget
    FeatureMetadata {
        slug: "compression-budget",
        name: "Compression Budget",
        category: FeatureCategory::Performance,
        status: FeatureStatus::Beta,
        handler: perf_network::compression_budget,
    },
    // MediaWeightGate
    FeatureMetadata {
        slug: "media-weight-gate",
        name: "Media Weight Gate",
        category: FeatureCategory::Performance,
        status: FeatureStatus::Beta,
        handler: perf_network::media_weight_gate,
    },
    // ContinuousProfiler
    FeatureMetadata {
        slug: "continuous-profiler",
        name: "Continuous Profiler",
        category: FeatureCategory::Performance,
        status: FeatureStatus::Experimental,
        handler: perf_profiles::continuous_profiler,
    },
    // PerformanceSla
    FeatureMetadata {
        slug: "performance-sla",
        name: "Performance SLA",
        category: FeatureCategory::Performance,
        status: FeatureStatus::Stable,
        handler: perf_profiles::performance_sla,
    },
    // ResilientLocators
    FeatureMetadata {
        slug: "resilient-locators",
        name: "Resilient Locators",
        category: FeatureCategory::Qa,
        status: FeatureStatus::Stable,
        handler: qa_tests::resilient_locators,
    },
    // FlakeRadar
    FeatureMetadata {
        slug: "flake-radar",
        name: "Flake Radar",
        category: FeatureCategory::Qa,
        status: FeatureStatus::Stable,
        handler: qa_tests::flake_radar,
    },
    // VisualRegression
    FeatureMetadata {
        slug: "visual-regression",
        name: "Visual Regression",
        category: FeatureCategory::Qa,
        status: FeatureStatus::Beta,
        handler: qa_tests::visual_regression,
    },
    // IdentityFixtures
    FeatureMetadata {
        slug: "identity-fixtures",
        name: "Identity Fixtures",
        category: FeatureCategory::Qa,
        status: FeatureStatus::Stable,
        handler: qa_tests::identity_fixtures,
    },
    // DataDrivenScenarios
    FeatureMetadata {
        slug: "data-driven-scenarios",
        name: "Data-Driven Scenarios",
        category: FeatureCategory::Qa,
        status: FeatureStatus::Stable,
        handler: qa_tests::data_driven_scenarios,
    },
    // RealVitalsGates
    FeatureMetadata {
        slug: "real-vitals-gates",
        name: "Real Vitals Gates",
        category: FeatureCategory::Qa,
        status: FeatureStatus::Stable,
        handler: qa_tests::real_vitals_gates,
    },
    // ContractTesting
    FeatureMetadata {
        slug: "contract-testing",
        name: "Contract Testing",
        category: FeatureCategory::Qa,
        status: FeatureStatus::Stable,
        handler: qa_exports::contract_testing,
    },
    // DeflakedExport
    FeatureMetadata {
        slug: "deflaked-export",
        name: "Deflaked Export",
        category: FeatureCategory::Qa,
        status: FeatureStatus::Stable,
        handler: qa_exports::deflaked_export,
    },
    // CiBundle
    FeatureMetadata {
        slug: "ci-bundle",
        name: "CI Bundle",
        category: FeatureCategory::Qa,
        status: FeatureStatus::Stable,
        handler: qa_exports::ci_bundle,
    },
    // TimeTravelTrace
    FeatureMetadata {
        slug: "time-travel-trace",
        name: "Time-Travel Trace",
        category: FeatureCategory::Qa,
        status: FeatureStatus::Stable,
        handler: qa_exports::time_travel_trace,
    },
    // DeterminismHarness
    FeatureMetadata {
        slug: "determinism-harness",
        name: "Determinism Harness",
        category: FeatureCategory::Qa,
        status: FeatureStatus::Stable,
        handler: qa_exports::determinism_harness,
    },
    // AuthzMatrixTests
    FeatureMetadata {
        slug: "authz-matrix-tests",
        name: "Authz Matrix Tests",
        category: FeatureCategory::Qa,
        status: FeatureStatus::Stable,
        handler: qa_exports::authz_matrix_tests,
    },
    // FlakeTriager
    FeatureMetadata {
        slug: "flake-triager",
        name: "Flake Triager",
        category: FeatureCategory::Qa,
        status: FeatureStatus::Beta,
        handler: qa_policy::flake_triager,
    },
    // QuarantinePolicy
    FeatureMetadata {
        slug: "quarantine-policy",
        name: "Quarantine Policy",
        category: FeatureCategory::Qa,
        status: FeatureStatus::Stable,
        handler: qa_policy::quarantine_policy,
    },
    // ContractDriftGate
    FeatureMetadata {
        slug: "contract-drift-gate",
        name: "Contract Drift Gate",
        category: FeatureCategory::Qa,
        status: FeatureStatus::Beta,
        handler: qa_policy::contract_drift_gate,
    },
    // HermeticFixturePack
    FeatureMetadata {
        slug: "hermetic-fixture-pack",
        name: "Hermetic Fixture Pack",
        category: FeatureCategory::Qa,
        status: FeatureStatus::Stable,
        handler: qa_policy::hermetic_fixture_pack,
    },
    // A11yTestGen
    FeatureMetadata {
        slug: "a11y-test-gen",
        name: "A11y Test Gen",
        category: FeatureCategory::Qa,
        status: FeatureStatus::Stable,
        handler: qa_policy::a11y_test_gen,
    },
    // SnapshotStrategyPack
    FeatureMetadata {
        slug: "snapshot-strategy-pack",
        name: "Snapshot Strategy Pack",
        category: FeatureCategory::Qa,
        status: FeatureStatus::Stable,
        handler: qa_policy::snapshot_strategy_pack,
    },
    // CoverageAdjacencyMap
    FeatureMetadata {
        slug: "coverage-adjacency-map",
        name: "Coverage Adjacency Map",
        category: FeatureCategory::Qa,
        status: FeatureStatus::Stable,
        handler: qa_execution::coverage_adjacency_map,
    },
    // ParallelShardPlanner
    FeatureMetadata {
        slug: "parallel-shard-planner",
        name: "Parallel Shard Planner",
        category: FeatureCategory::Qa,
        status: FeatureStatus::Stable,
        handler: qa_execution::parallel_shard_planner,
    },
    // TracePropertyMiner
    FeatureMetadata {
        slug: "trace-property-miner",
        name: "Trace Property Miner",
        category: FeatureCategory::Qa,
        status: FeatureStatus::Beta,
        handler: qa_execution::trace_property_miner,
    },
    // RuntimeSelfHeal
    FeatureMetadata {
        slug: "runtime-self-heal",
        name: "Runtime Self-Heal",
        category: FeatureCategory::Qa,
        status: FeatureStatus::Experimental,
        handler: qa_execution::runtime_self_heal,
    },
    // OracleSynthesizer
    FeatureMetadata {
        slug: "oracle-synthesizer",
        name: "Oracle Synthesizer",
        category: FeatureCategory::Qa,
        status: FeatureStatus::Stable,
        handler: qa_execution::oracle_synthesizer,
    },
    // MutationHookBridge
    FeatureMetadata {
        slug: "mutation-hook-bridge",
        name: "Mutation Hook Bridge",
        category: FeatureCategory::Qa,
        status: FeatureStatus::Experimental,
        handler: qa_execution::mutation_hook_bridge,
    },
    // LlmInspector
    FeatureMetadata {
        slug: "llm-inspector",
        name: "LLM Inspector",
        category: FeatureCategory::Ai,
        status: FeatureStatus::Stable,
        handler: ai_inspection::llm_inspector,
    },
    // StreamingWaterfall
    FeatureMetadata {
        slug: "streaming-waterfall",
        name: "Streaming Waterfall",
        category: FeatureCategory::Ai,
        status: FeatureStatus::Stable,
        handler: ai_inspection::streaming_waterfall,
    },
    // PromptDiff
    FeatureMetadata {
        slug: "prompt-diff",
        name: "Prompt Diff",
        category: FeatureCategory::Ai,
        status: FeatureStatus::Stable,
        handler: ai_prompts::prompt_diff,
    },
    // CostDashboard
    FeatureMetadata {
        slug: "cost-dashboard",
        name: "Cost Dashboard",
        category: FeatureCategory::Ai,
        status: FeatureStatus::Stable,
        handler: ai_prompts::cost_dashboard,
    },
    // EvalReplay
    FeatureMetadata {
        slug: "eval-replay",
        name: "Eval Replay",
        category: FeatureCategory::Ai,
        status: FeatureStatus::Stable,
        handler: ai_models::eval_replay,
    },
    // ModelComparison
    FeatureMetadata {
        slug: "model-comparison",
        name: "Model Comparison",
        category: FeatureCategory::Ai,
        status: FeatureStatus::Stable,
        handler: ai_models::model_comparison,
    },
    // AgentTrace
    FeatureMetadata {
        slug: "agent-trace",
        name: "Agent Trace",
        category: FeatureCategory::Ai,
        status: FeatureStatus::Experimental,
        handler: ai_models::agent_trace,
    },
    // ContextHeatmap
    FeatureMetadata {
        slug: "context-heatmap",
        name: "Context Heatmap",
        category: FeatureCategory::Ai,
        status: FeatureStatus::Stable,
        handler: ai_prompts::context_heatmap,
    },
    // RagInspector
    FeatureMetadata {
        slug: "rag-inspector",
        name: "RAG Inspector",
        category: FeatureCategory::Ai,
        status: FeatureStatus::Beta,
        handler: ai_prompts::rag_inspector,
    },
    // InjectionScanner
    FeatureMetadata {
        slug: "injection-scanner",
        name: "Prompt Injection Scanner",
        category: FeatureCategory::Ai,
        status: FeatureStatus::Beta,
        handler: ai_prompts::injection_scanner,
    },
    // LlmLoad
    FeatureMetadata {
        slug: "llm-load",
        name: "LLM Load",
        category: FeatureCategory::Ai,
        status: FeatureStatus::Stable,
        handler: ai_models::llm_load,
    },
    // DeterministicReplay
    FeatureMetadata {
        slug: "deterministic-replay",
        name: "Deterministic Replay",
        category: FeatureCategory::Ai,
        status: FeatureStatus::Stable,
        handler: ai_models::deterministic_replay,
    },
    // ToolCallForensics
    FeatureMetadata {
        slug: "tool-call-forensics",
        name: "Tool Call Forensics",
        category: FeatureCategory::Ai,
        status: FeatureStatus::Beta,
        handler: ai_inspection::tool_call_forensics,
    },
    // JsonSchemaGuard
    FeatureMetadata {
        slug: "json-schema-guard",
        name: "JSON Schema Guard",
        category: FeatureCategory::Ai,
        status: FeatureStatus::Beta,
        handler: ai_quality::json_schema_guard,
    },
    // CacheHitLedger
    FeatureMetadata {
        slug: "cache-hit-ledger",
        name: "Cache Hit Ledger",
        category: FeatureCategory::Ai,
        status: FeatureStatus::Beta,
        handler: ai_metrics::cache_hit_ledger,
    },
    // TokenWasteAuditor
    FeatureMetadata {
        slug: "token-waste-auditor",
        name: "Token Waste Auditor",
        category: FeatureCategory::Ai,
        status: FeatureStatus::Beta,
        handler: ai_metrics::token_waste_auditor,
    },
    // StreamingSloGates
    FeatureMetadata {
        slug: "streaming-slo-gates",
        name: "Streaming SLO Gates",
        category: FeatureCategory::Ai,
        status: FeatureStatus::Beta,
        handler: ai_metrics::streaming_slo_gates,
    },
    // RouterDecisionTrace
    FeatureMetadata {
        slug: "router-decision-trace",
        name: "Router Decision Trace",
        category: FeatureCategory::Ai,
        status: FeatureStatus::Beta,
        handler: ai_metrics::router_decision_trace,
    },
    // McpTrafficLens
    FeatureMetadata {
        slug: "mcp-traffic-lens",
        name: "MCP Traffic Lens",
        category: FeatureCategory::Ai,
        status: FeatureStatus::Experimental,
        handler: ai_inspection::mcp_traffic_lens,
    },
    // EvalDatasetMiner
    FeatureMetadata {
        slug: "eval-dataset-miner",
        name: "Eval Dataset Miner",
        category: FeatureCategory::Ai,
        status: FeatureStatus::Beta,
        handler: ai_quality::eval_dataset_miner,
    },
    // HallucinationOracle
    FeatureMetadata {
        slug: "hallucination-oracle",
        name: "Hallucination Oracle (evidence-gated)",
        category: FeatureCategory::Ai,
        status: FeatureStatus::Beta,
        handler: ai_quality::hallucination_oracle,
    },
    // EmbeddingRagBench
    FeatureMetadata {
        slug: "embedding-rag-bench",
        name: "Embedding / RAG Bench",
        category: FeatureCategory::Ai,
        status: FeatureStatus::Beta,
        handler: ai_quality::embedding_rag_bench,
    },
    // ProtocolQuirkDiff
    FeatureMetadata {
        slug: "protocol-quirk-diff",
        name: "Protocol Quirk Diff",
        category: FeatureCategory::Ai,
        status: FeatureStatus::Beta,
        handler: ai_metrics::protocol_quirk_diff,
    },
    // LocalCloudParity
    FeatureMetadata {
        slug: "local-cloud-parity",
        name: "Local / Cloud Parity",
        category: FeatureCategory::Ai,
        status: FeatureStatus::Beta,
        handler: ai_metrics::local_cloud_parity,
    },
];

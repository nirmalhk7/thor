//! Thor Engineer Feature Suite.
//!
//! Engineer feature requests organized into six specialist domains plus a
//! Performance SLA subsystem. Every feature is backed by real Rust logic in one
//! of the domain modules and registered here so the shell / IPC layer can
//! enumerate, enable, and invoke them through a single [`FeatureEngine`].
//!
//! Access the process-wide engine via [`crate::AppState::global().features()`].
//!
//! ## Soft-Nav Vitals (honest capture note)
//!
//! [`FeatureId::SoftNavVitals`] analyzes `SoftNavRecord` payloads. Live capture
//! requires evaluating [`cdp_bridge::CdpBridge::soft_nav_and_speculation_script`]
//! (wired by `BrowserRunner` navigate) and feeding
//! [`cdp_bridge::CdpBridge::parse_soft_navs`] into `PerfEngine::soft_nav_vitals`.
//! All-zero records (`lcp=inp=cls=0`) are refused. Without script evaluation
//! (or offline JSON), SoftNav reports stay empty — not simulated.

pub mod ai;
pub mod backend;
pub mod cdp_bridge;
pub mod frontend;
pub mod orchestrator;
pub mod perf_features;
pub mod qa;
pub mod security;
pub mod sla;

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use ai::AiEngine;
use backend::BackendEngine;
use cdp_bridge::CdpBridge;
use frontend::FrontendEngine;
use perf_features::PerfEngine;
use qa::QaEngine;
use security::SecurityEngine;
use sla::SlaEngine;

/// Specialist domain a feature belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FeatureCategory {
    Frontend,
    Backend,
    Security,
    Performance,
    Qa,
    Ai,
}

impl FeatureCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            FeatureCategory::Frontend => "Frontend",
            FeatureCategory::Backend => "Backend",
            FeatureCategory::Security => "Security",
            FeatureCategory::Performance => "Performance",
            FeatureCategory::Qa => "QA",
            FeatureCategory::Ai => "AI",
        }
    }
}

/// Maturity/health status of a feature implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FeatureStatus {
    Stable,
    Beta,
    Experimental,
}

impl FeatureStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            FeatureStatus::Stable => "stable",
            FeatureStatus::Beta => "beta",
            FeatureStatus::Experimental => "experimental",
        }
    }
}

/// Stable identifier for each registered engineer feature.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FeatureId {
    // Frontend (11)
    ComponentTreeLens,
    ResponsiveMatrix,
    CssCascade,
    DesignTokens,
    A11yLens,
    StateTimeTravel,
    ClsHeatmap,
    ComponentSandbox,
    NetworkComponentBinding,
    FontAuditor,
    ZIndexVisualizer,
    // Frontend wave-2 (12)
    HydrationDiffLens,
    CssCoverageMap,
    FocusOrderOverlay,
    EventListenerMap,
    CssVariableInspector,
    LayoutThrashDetector,
    MediaCqMatrix,
    ImageDeliveryAuditor,
    CssContainmentAdvisor,
    AriaLiveMonitor,
    SelectorSpecificityBudget,
    DomSnapshotCapture,
    // Backend (12)
    ApiWorkbench,
    AuthVault,
    ContractValidation,
    WebhookSink,
    IdorMatrix,
    RequestChaining,
    GrpcGraphql,
    TraceAdjacency,
    MockStub,
    ChaosInjection,
    IdempotencyAuditor,
    DbAdjacency,
    // Backend wave-2 (12)
    WebhookSignatureVault,
    RateLimitProbe,
    PaginationAuditor,
    CacheSemanticsLens,
    MultipartWorkbench,
    SseWsServerLab,
    OpenApiDiffGate,
    OauthFlowCapture,
    QueueAdjacency,
    GrpcStreamWorkbench,
    LiveStubServer,
    HeaderContractFuzzer,
    // Security (12)
    AuthzMatrix,
    ResponseDiff,
    HeaderAnalyzer,
    JwtDeep,
    PromoteFinding,
    SurfaceCrawler,
    StorageAuditor,
    FuzzRepeater,
    CsrfAnalyzer,
    SinkPollution,
    TlsInspector,
    OobListener,
    // Security wave-2 (12)
    DomXssTaintMap,
    CorsDeepProbe,
    CspPolicyForensics,
    OauthOidcMisconfig,
    CookieSessionForensics,
    SessionFixationLab,
    GraphqlAuthzMatrix,
    MassAssignmentDiff,
    RaceWindowRepeater,
    SsrfLabHarness,
    SriSupplyChainAuditor,
    WebsocketAuthzProbe,
    // Performance (24 + SLA)
    PerfBudgets,
    TraceDiff,
    WaterfallForensics,
    MemoryLeakHunter,
    SyntheticMonitor,
    RumLab,
    HdrHistogramHelper,
    InpAttribution,
    CdnCache,
    VitalsUnderLoad,
    FlameGraph,
    ThirdPartyAttribution,
    LcpElementForensics,
    LongTaskRadar,
    MainThreadBlockMap,
    BfCacheProbe,
    FreezeResumeBench,
    SoftNavVitals,
    SpeculationLab,
    ConnPoolInspector,
    Http3PathAudit,
    CompressionBudget,
    MediaWeightGate,
    ContinuousProfiler,
    PerformanceSla,
    // QA (12 wave-1 + 12 wave-2)
    ResilientLocators,
    FlakeRadar,
    VisualRegression,
    IdentityFixtures,
    DataDrivenScenarios,
    RealVitalsGates,
    ContractTesting,
    DeflakedExport,
    CiBundle,
    TimeTravelTrace,
    DeterminismHarness,
    AuthzMatrixTests,
    FlakeTriager,
    QuarantinePolicy,
    ContractDriftGate,
    HermeticFixturePack,
    A11yTestGen,
    SnapshotStrategyPack,
    CoverageAdjacencyMap,
    ParallelShardPlanner,
    TracePropertyMiner,
    RuntimeSelfHeal,
    OracleSynthesizer,
    MutationHookBridge,
    // AI (12)
    LlmInspector,
    StreamingWaterfall,
    PromptDiff,
    CostDashboard,
    EvalReplay,
    ModelComparison,
    AgentTrace,
    ContextHeatmap,
    RagInspector,
    InjectionScanner,
    LlmLoad,
    DeterministicReplay,
    // AI wave-2 (12)
    ToolCallForensics,
    JsonSchemaGuard,
    CacheHitLedger,
    TokenWasteAuditor,
    StreamingSloGates,
    RouterDecisionTrace,
    McpTrafficLens,
    EvalDatasetMiner,
    HallucinationOracle,
    EmbeddingRagBench,
    ProtocolQuirkDiff,
    LocalCloudParity,
}

impl FeatureId {
    /// All feature identifiers in registry order.
    pub const ALL: [FeatureId; 144] = [
        FeatureId::ComponentTreeLens,
        FeatureId::ResponsiveMatrix,
        FeatureId::CssCascade,
        FeatureId::DesignTokens,
        FeatureId::A11yLens,
        FeatureId::StateTimeTravel,
        FeatureId::ClsHeatmap,
        FeatureId::ComponentSandbox,
        FeatureId::NetworkComponentBinding,
        FeatureId::FontAuditor,
        FeatureId::ZIndexVisualizer,
        FeatureId::HydrationDiffLens,
        FeatureId::CssCoverageMap,
        FeatureId::FocusOrderOverlay,
        FeatureId::EventListenerMap,
        FeatureId::CssVariableInspector,
        FeatureId::LayoutThrashDetector,
        FeatureId::MediaCqMatrix,
        FeatureId::ImageDeliveryAuditor,
        FeatureId::CssContainmentAdvisor,
        FeatureId::AriaLiveMonitor,
        FeatureId::SelectorSpecificityBudget,
        FeatureId::DomSnapshotCapture,
        FeatureId::ApiWorkbench,
        FeatureId::AuthVault,
        FeatureId::ContractValidation,
        FeatureId::WebhookSink,
        FeatureId::IdorMatrix,
        FeatureId::RequestChaining,
        FeatureId::GrpcGraphql,
        FeatureId::TraceAdjacency,
        FeatureId::MockStub,
        FeatureId::ChaosInjection,
        FeatureId::IdempotencyAuditor,
        FeatureId::DbAdjacency,
        FeatureId::WebhookSignatureVault,
        FeatureId::RateLimitProbe,
        FeatureId::PaginationAuditor,
        FeatureId::CacheSemanticsLens,
        FeatureId::MultipartWorkbench,
        FeatureId::SseWsServerLab,
        FeatureId::OpenApiDiffGate,
        FeatureId::OauthFlowCapture,
        FeatureId::QueueAdjacency,
        FeatureId::GrpcStreamWorkbench,
        FeatureId::LiveStubServer,
        FeatureId::HeaderContractFuzzer,
        FeatureId::AuthzMatrix,
        FeatureId::ResponseDiff,
        FeatureId::HeaderAnalyzer,
        FeatureId::JwtDeep,
        FeatureId::PromoteFinding,
        FeatureId::SurfaceCrawler,
        FeatureId::StorageAuditor,
        FeatureId::FuzzRepeater,
        FeatureId::CsrfAnalyzer,
        FeatureId::SinkPollution,
        FeatureId::TlsInspector,
        FeatureId::OobListener,
        FeatureId::DomXssTaintMap,
        FeatureId::CorsDeepProbe,
        FeatureId::CspPolicyForensics,
        FeatureId::OauthOidcMisconfig,
        FeatureId::CookieSessionForensics,
        FeatureId::SessionFixationLab,
        FeatureId::GraphqlAuthzMatrix,
        FeatureId::MassAssignmentDiff,
        FeatureId::RaceWindowRepeater,
        FeatureId::SsrfLabHarness,
        FeatureId::SriSupplyChainAuditor,
        FeatureId::WebsocketAuthzProbe,
        FeatureId::PerfBudgets,
        FeatureId::TraceDiff,
        FeatureId::WaterfallForensics,
        FeatureId::MemoryLeakHunter,
        FeatureId::SyntheticMonitor,
        FeatureId::RumLab,
        FeatureId::HdrHistogramHelper,
        FeatureId::InpAttribution,
        FeatureId::CdnCache,
        FeatureId::VitalsUnderLoad,
        FeatureId::FlameGraph,
        FeatureId::ThirdPartyAttribution,
        FeatureId::LcpElementForensics,
        FeatureId::LongTaskRadar,
        FeatureId::MainThreadBlockMap,
        FeatureId::BfCacheProbe,
        FeatureId::FreezeResumeBench,
        FeatureId::SoftNavVitals,
        FeatureId::SpeculationLab,
        FeatureId::ConnPoolInspector,
        FeatureId::Http3PathAudit,
        FeatureId::CompressionBudget,
        FeatureId::MediaWeightGate,
        FeatureId::ContinuousProfiler,
        FeatureId::PerformanceSla,
        FeatureId::ResilientLocators,
        FeatureId::FlakeRadar,
        FeatureId::VisualRegression,
        FeatureId::IdentityFixtures,
        FeatureId::DataDrivenScenarios,
        FeatureId::RealVitalsGates,
        FeatureId::ContractTesting,
        FeatureId::DeflakedExport,
        FeatureId::CiBundle,
        FeatureId::TimeTravelTrace,
        FeatureId::DeterminismHarness,
        FeatureId::AuthzMatrixTests,
        FeatureId::FlakeTriager,
        FeatureId::QuarantinePolicy,
        FeatureId::ContractDriftGate,
        FeatureId::HermeticFixturePack,
        FeatureId::A11yTestGen,
        FeatureId::SnapshotStrategyPack,
        FeatureId::CoverageAdjacencyMap,
        FeatureId::ParallelShardPlanner,
        FeatureId::TracePropertyMiner,
        FeatureId::RuntimeSelfHeal,
        FeatureId::OracleSynthesizer,
        FeatureId::MutationHookBridge,
        FeatureId::LlmInspector,
        FeatureId::StreamingWaterfall,
        FeatureId::PromptDiff,
        FeatureId::CostDashboard,
        FeatureId::EvalReplay,
        FeatureId::ModelComparison,
        FeatureId::AgentTrace,
        FeatureId::ContextHeatmap,
        FeatureId::RagInspector,
        FeatureId::InjectionScanner,
        FeatureId::LlmLoad,
        FeatureId::DeterministicReplay,
        FeatureId::ToolCallForensics,
        FeatureId::JsonSchemaGuard,
        FeatureId::CacheHitLedger,
        FeatureId::TokenWasteAuditor,
        FeatureId::StreamingSloGates,
        FeatureId::RouterDecisionTrace,
        FeatureId::McpTrafficLens,
        FeatureId::EvalDatasetMiner,
        FeatureId::HallucinationOracle,
        FeatureId::EmbeddingRagBench,
        FeatureId::ProtocolQuirkDiff,
        FeatureId::LocalCloudParity,
    ];

    /// Stable machine slug (kebab-case) for IPC routing.
    pub fn slug(&self) -> &'static str {
        match self {
            FeatureId::ComponentTreeLens => "component-tree-lens",
            FeatureId::ResponsiveMatrix => "responsive-matrix",
            FeatureId::CssCascade => "css-cascade",
            FeatureId::DesignTokens => "design-tokens",
            FeatureId::A11yLens => "a11y-lens",
            FeatureId::StateTimeTravel => "state-timetravel",
            FeatureId::ClsHeatmap => "cls-heatmap",
            FeatureId::ComponentSandbox => "component-sandbox",
            FeatureId::NetworkComponentBinding => "network-component-binding",
            FeatureId::FontAuditor => "font-auditor",
            FeatureId::ZIndexVisualizer => "zindex-visualizer",
            FeatureId::ApiWorkbench => "api-workbench",
            FeatureId::AuthVault => "auth-vault",
            FeatureId::ContractValidation => "contract-validation",
            FeatureId::WebhookSink => "webhook-sink",
            FeatureId::IdorMatrix => "idor-matrix",
            FeatureId::RequestChaining => "request-chaining",
            FeatureId::GrpcGraphql => "grpc-graphql",
            FeatureId::TraceAdjacency => "trace-adjacency",
            FeatureId::MockStub => "mock-stub",
            FeatureId::ChaosInjection => "chaos-injection",
            FeatureId::IdempotencyAuditor => "idempotency-auditor",
            FeatureId::DbAdjacency => "db-adjacency",
            FeatureId::AuthzMatrix => "authz-matrix",
            FeatureId::ResponseDiff => "response-diff",
            FeatureId::HeaderAnalyzer => "header-analyzer",
            FeatureId::JwtDeep => "jwt-deep",
            FeatureId::PromoteFinding => "promote-finding",
            FeatureId::SurfaceCrawler => "surface-crawler",
            FeatureId::StorageAuditor => "storage-auditor",
            FeatureId::FuzzRepeater => "fuzz-repeater",
            FeatureId::CsrfAnalyzer => "csrf-analyzer",
            FeatureId::SinkPollution => "sink-pollution",
            FeatureId::TlsInspector => "tls-inspector",
            FeatureId::OobListener => "oob-listener",
            FeatureId::PerfBudgets => "perf-budgets",
            FeatureId::TraceDiff => "trace-diff",
            FeatureId::WaterfallForensics => "waterfall-forensics",
            FeatureId::MemoryLeakHunter => "memory-leak-hunter",
            FeatureId::SyntheticMonitor => "synthetic-monitor",
            FeatureId::RumLab => "rum-lab",
            FeatureId::HdrHistogramHelper => "hdr-histogram-helper",
            FeatureId::InpAttribution => "inp-attribution",
            FeatureId::CdnCache => "cdn-cache",
            FeatureId::VitalsUnderLoad => "vitals-under-load",
            FeatureId::FlameGraph => "flame-graph",
            FeatureId::ThirdPartyAttribution => "third-party-attribution",
            FeatureId::LcpElementForensics => "lcp-element-forensics",
            FeatureId::LongTaskRadar => "long-task-radar",
            FeatureId::MainThreadBlockMap => "main-thread-block-map",
            FeatureId::BfCacheProbe => "bfcache-probe",
            FeatureId::FreezeResumeBench => "freeze-resume-bench",
            FeatureId::SoftNavVitals => "soft-nav-vitals",
            FeatureId::SpeculationLab => "speculation-lab",
            FeatureId::ConnPoolInspector => "conn-pool-inspector",
            FeatureId::Http3PathAudit => "http3-path-audit",
            FeatureId::CompressionBudget => "compression-budget",
            FeatureId::MediaWeightGate => "media-weight-gate",
            FeatureId::ContinuousProfiler => "continuous-profiler",
            FeatureId::PerformanceSla => "performance-sla",
            FeatureId::ResilientLocators => "resilient-locators",
            FeatureId::FlakeRadar => "flake-radar",
            FeatureId::VisualRegression => "visual-regression",
            FeatureId::IdentityFixtures => "identity-fixtures",
            FeatureId::DataDrivenScenarios => "data-driven-scenarios",
            FeatureId::RealVitalsGates => "real-vitals-gates",
            FeatureId::ContractTesting => "contract-testing",
            FeatureId::DeflakedExport => "deflaked-export",
            FeatureId::CiBundle => "ci-bundle",
            FeatureId::TimeTravelTrace => "time-travel-trace",
            FeatureId::DeterminismHarness => "determinism-harness",
            FeatureId::AuthzMatrixTests => "authz-matrix-tests",
            FeatureId::FlakeTriager => "flake-triager",
            FeatureId::QuarantinePolicy => "quarantine-policy",
            FeatureId::ContractDriftGate => "contract-drift-gate",
            FeatureId::HermeticFixturePack => "hermetic-fixture-pack",
            FeatureId::A11yTestGen => "a11y-test-gen",
            FeatureId::SnapshotStrategyPack => "snapshot-strategy-pack",
            FeatureId::CoverageAdjacencyMap => "coverage-adjacency-map",
            FeatureId::ParallelShardPlanner => "parallel-shard-planner",
            FeatureId::TracePropertyMiner => "trace-property-miner",
            FeatureId::RuntimeSelfHeal => "runtime-self-heal",
            FeatureId::OracleSynthesizer => "oracle-synthesizer",
            FeatureId::MutationHookBridge => "mutation-hook-bridge",
            FeatureId::LlmInspector => "llm-inspector",
            FeatureId::StreamingWaterfall => "streaming-waterfall",
            FeatureId::PromptDiff => "prompt-diff",
            FeatureId::CostDashboard => "cost-dashboard",
            FeatureId::EvalReplay => "eval-replay",
            FeatureId::ModelComparison => "model-comparison",
            FeatureId::AgentTrace => "agent-trace",
            FeatureId::ContextHeatmap => "context-heatmap",
            FeatureId::RagInspector => "rag-inspector",
            FeatureId::InjectionScanner => "injection-scanner",
            FeatureId::LlmLoad => "llm-load",
            FeatureId::DeterministicReplay => "deterministic-replay",
FeatureId::HydrationDiffLens => "hydration-diff-lens",
            FeatureId::CssCoverageMap => "css-coverage-map",
            FeatureId::FocusOrderOverlay => "focus-order-overlay",
            FeatureId::EventListenerMap => "event-listener-map",
            FeatureId::CssVariableInspector => "css-variable-inspector",
            FeatureId::LayoutThrashDetector => "layout-thrash-detector",
            FeatureId::MediaCqMatrix => "media-cq-matrix",
            FeatureId::ImageDeliveryAuditor => "image-delivery-auditor",
            FeatureId::CssContainmentAdvisor => "css-containment-advisor",
            FeatureId::AriaLiveMonitor => "aria-live-monitor",
            FeatureId::SelectorSpecificityBudget => "selector-specificity-budget",
            FeatureId::DomSnapshotCapture => "dom-snapshot-capture",
            FeatureId::WebhookSignatureVault => "webhook-signature-vault",
            FeatureId::RateLimitProbe => "rate-limit-probe",
            FeatureId::PaginationAuditor => "pagination-auditor",
            FeatureId::CacheSemanticsLens => "cache-semantics-lens",
            FeatureId::MultipartWorkbench => "multipart-workbench",
            FeatureId::SseWsServerLab => "sse-ws-server-lab",
            FeatureId::OpenApiDiffGate => "openapi-diff-gate",
            FeatureId::OauthFlowCapture => "oauth-flow-capture",
            FeatureId::QueueAdjacency => "queue-adjacency",
            FeatureId::GrpcStreamWorkbench => "grpc-stream-workbench",
            FeatureId::LiveStubServer => "live-stub-server",
            FeatureId::HeaderContractFuzzer => "header-contract-fuzzer",
            FeatureId::DomXssTaintMap => "dom-xss-taint-map",
            FeatureId::CorsDeepProbe => "cors-deep-probe",
            FeatureId::CspPolicyForensics => "csp-policy-forensics",
            FeatureId::OauthOidcMisconfig => "oauth-oidc-misconfig",
            FeatureId::CookieSessionForensics => "cookie-session-forensics",
            FeatureId::SessionFixationLab => "session-fixation-lab",
            FeatureId::GraphqlAuthzMatrix => "graphql-authz-matrix",
            FeatureId::MassAssignmentDiff => "mass-assignment-diff",
            FeatureId::RaceWindowRepeater => "race-window-repeater",
            FeatureId::SsrfLabHarness => "ssrf-lab-harness",
            FeatureId::SriSupplyChainAuditor => "sri-supply-chain-auditor",
            FeatureId::WebsocketAuthzProbe => "websocket-authz-probe",
            FeatureId::ToolCallForensics => "tool-call-forensics",
            FeatureId::JsonSchemaGuard => "json-schema-guard",
            FeatureId::CacheHitLedger => "cache-hit-ledger",
            FeatureId::TokenWasteAuditor => "token-waste-auditor",
            FeatureId::StreamingSloGates => "streaming-slo-gates",
            FeatureId::RouterDecisionTrace => "router-decision-trace",
            FeatureId::McpTrafficLens => "mcp-traffic-lens",
            FeatureId::EvalDatasetMiner => "eval-dataset-miner",
            FeatureId::HallucinationOracle => "hallucination-oracle",
            FeatureId::EmbeddingRagBench => "embedding-rag-bench",
            FeatureId::ProtocolQuirkDiff => "protocol-quirk-diff",
            FeatureId::LocalCloudParity => "local-cloud-parity",
        }
    }

    /// Human-readable feature name.
    pub fn name(&self) -> &'static str {
        match self {
            FeatureId::ComponentTreeLens => "Component Tree Lens",
            FeatureId::ResponsiveMatrix => "Responsive Matrix",
            FeatureId::CssCascade => "CSS Cascade Inspector",
            FeatureId::DesignTokens => "Design Token Extractor",
            FeatureId::A11yLens => "Accessibility Lens",
            FeatureId::StateTimeTravel => "State Time-Travel",
            FeatureId::ClsHeatmap => "CLS Heatmap",
            FeatureId::ComponentSandbox => "Component Sandbox",
            FeatureId::NetworkComponentBinding => "Network↔Component Binding",
            FeatureId::FontAuditor => "Font Auditor",
            FeatureId::ZIndexVisualizer => "Z-Index Visualizer",
            FeatureId::ApiWorkbench => "API Workbench",
            FeatureId::AuthVault => "Auth Vault",
            FeatureId::ContractValidation => "Contract Validation",
            FeatureId::WebhookSink => "Webhook Sink",
            FeatureId::IdorMatrix => "IDOR Matrix",
            FeatureId::RequestChaining => "Request Chaining",
            FeatureId::GrpcGraphql => "gRPC / GraphQL Inspector",
            FeatureId::TraceAdjacency => "Trace Adjacency",
            FeatureId::MockStub => "Mock / Stub Server",
            FeatureId::ChaosInjection => "Chaos Injection",
            FeatureId::IdempotencyAuditor => "Idempotency Auditor",
            FeatureId::DbAdjacency => "DB Adjacency",
            FeatureId::AuthzMatrix => "Authorization Matrix",
            FeatureId::ResponseDiff => "Response Diff",
            FeatureId::HeaderAnalyzer => "Security Header Analyzer",
            FeatureId::JwtDeep => "JWT Deep Inspector",
            FeatureId::PromoteFinding => "Promote to Finding",
            FeatureId::SurfaceCrawler => "Attack Surface Crawler",
            FeatureId::StorageAuditor => "Storage Auditor",
            FeatureId::FuzzRepeater => "Fuzz Repeater",
            FeatureId::CsrfAnalyzer => "CSRF Analyzer",
            FeatureId::SinkPollution => "Sink / Pollution Detector",
            FeatureId::TlsInspector => "TLS Inspector",
            FeatureId::OobListener => "Out-of-Band Listener",
            FeatureId::PerfBudgets => "Performance Budgets",
            FeatureId::TraceDiff => "Trace Diff",
            FeatureId::WaterfallForensics => "Waterfall Forensics",
            FeatureId::MemoryLeakHunter => "Memory Leak Hunter",
            FeatureId::SyntheticMonitor => "Synthetic Monitor",
            FeatureId::RumLab => "RUM vs Lab",
            FeatureId::HdrHistogramHelper => "HDR Histogram Helper",
            FeatureId::InpAttribution => "INP Attribution",
            FeatureId::CdnCache => "CDN Cache Analyzer",
            FeatureId::VitalsUnderLoad => "Vitals Under Load",
            FeatureId::FlameGraph => "Flame Graph",
            FeatureId::ThirdPartyAttribution => "Third-Party Attribution",
            FeatureId::LcpElementForensics => "LCP Element Forensics",
            FeatureId::LongTaskRadar => "Long Task Radar",
            FeatureId::MainThreadBlockMap => "Main-Thread Block Map",
            FeatureId::BfCacheProbe => "bfcache Probe",
            FeatureId::FreezeResumeBench => "Freeze / Resume Bench",
            FeatureId::SoftNavVitals => "Soft-Nav Vitals (experimental)",
            FeatureId::SpeculationLab => "Speculation Lab",
            FeatureId::ConnPoolInspector => "Connection Pool Inspector",
            FeatureId::Http3PathAudit => "HTTP/3 Path Audit",
            FeatureId::CompressionBudget => "Compression Budget",
            FeatureId::MediaWeightGate => "Media Weight Gate",
            FeatureId::ContinuousProfiler => "Continuous Profiler",
            FeatureId::PerformanceSla => "Performance SLA",
            FeatureId::ResilientLocators => "Resilient Locators",
            FeatureId::FlakeRadar => "Flake Radar",
            FeatureId::VisualRegression => "Visual Regression",
            FeatureId::IdentityFixtures => "Identity Fixtures",
            FeatureId::DataDrivenScenarios => "Data-Driven Scenarios",
            FeatureId::RealVitalsGates => "Real Vitals Gates",
            FeatureId::ContractTesting => "Contract Testing",
            FeatureId::DeflakedExport => "Deflaked Export",
            FeatureId::CiBundle => "CI Bundle",
            FeatureId::TimeTravelTrace => "Time-Travel Trace",
            FeatureId::DeterminismHarness => "Determinism Harness",
            FeatureId::AuthzMatrixTests => "Authz Matrix Tests",
            FeatureId::FlakeTriager => "Flake Triager",
            FeatureId::QuarantinePolicy => "Quarantine Policy",
            FeatureId::ContractDriftGate => "Contract Drift Gate",
            FeatureId::HermeticFixturePack => "Hermetic Fixture Pack",
            FeatureId::A11yTestGen => "A11y Test Gen",
            FeatureId::SnapshotStrategyPack => "Snapshot Strategy Pack",
            FeatureId::CoverageAdjacencyMap => "Coverage Adjacency Map",
            FeatureId::ParallelShardPlanner => "Parallel Shard Planner",
            FeatureId::TracePropertyMiner => "Trace Property Miner",
            FeatureId::RuntimeSelfHeal => "Runtime Self-Heal",
            FeatureId::OracleSynthesizer => "Oracle Synthesizer",
            FeatureId::MutationHookBridge => "Mutation Hook Bridge",
            FeatureId::LlmInspector => "LLM Inspector",
            FeatureId::StreamingWaterfall => "Streaming Waterfall",
            FeatureId::PromptDiff => "Prompt Diff",
            FeatureId::CostDashboard => "Cost Dashboard",
            FeatureId::EvalReplay => "Eval Replay",
            FeatureId::ModelComparison => "Model Comparison",
            FeatureId::AgentTrace => "Agent Trace",
            FeatureId::ContextHeatmap => "Context Heatmap",
            FeatureId::RagInspector => "RAG Inspector",
            FeatureId::InjectionScanner => "Prompt Injection Scanner",
            FeatureId::LlmLoad => "LLM Load",
            FeatureId::DeterministicReplay => "Deterministic Replay",
FeatureId::HydrationDiffLens => "Hydration Diff Lens",
            FeatureId::CssCoverageMap => "CSS Coverage Map",
            FeatureId::FocusOrderOverlay => "Focus Order Overlay",
            FeatureId::EventListenerMap => "Event Listener Map",
            FeatureId::CssVariableInspector => "CSS Variable Inspector",
            FeatureId::LayoutThrashDetector => "Layout Thrash Detector",
            FeatureId::MediaCqMatrix => "Media / Container Query Matrix",
            FeatureId::ImageDeliveryAuditor => "Image Delivery Auditor",
            FeatureId::CssContainmentAdvisor => "CSS Containment Advisor",
            FeatureId::AriaLiveMonitor => "ARIA Live Monitor",
            FeatureId::SelectorSpecificityBudget => "Selector Specificity Budget",
            FeatureId::DomSnapshotCapture => "DOM Snapshot Capture",
            FeatureId::WebhookSignatureVault => "Webhook Signature Vault",
            FeatureId::RateLimitProbe => "Rate Limit Probe",
            FeatureId::PaginationAuditor => "Pagination Auditor",
            FeatureId::CacheSemanticsLens => "Cache Semantics Lens",
            FeatureId::MultipartWorkbench => "Multipart Analyzer (offline)",
            FeatureId::SseWsServerLab => "SSE Server Lab",
            FeatureId::OpenApiDiffGate => "OpenAPI Diff Gate",
            FeatureId::OauthFlowCapture => "OAuth Flow Capture",
            FeatureId::QueueAdjacency => "Queue Adjacency",
            FeatureId::GrpcStreamWorkbench => "gRPC Stream Analyzer (offline)",
            FeatureId::LiveStubServer => "Live Stub Server",
            FeatureId::HeaderContractFuzzer => "Header Contract Fuzzer",
            FeatureId::DomXssTaintMap => "DOM XSS Taint Analyzer (offline)",
            FeatureId::CorsDeepProbe => "CORS Deep Probe",
            FeatureId::CspPolicyForensics => "CSP Policy Forensics",
            FeatureId::OauthOidcMisconfig => "OAuth / OIDC Misconfig",
            FeatureId::CookieSessionForensics => "Cookie Session Forensics",
            FeatureId::SessionFixationLab => "Session Fixation Lab",
            FeatureId::GraphqlAuthzMatrix => "GraphQL Authz Matrix",
            FeatureId::MassAssignmentDiff => "Mass Assignment Diff",
            FeatureId::RaceWindowRepeater => "Race Window Analyzer",
            FeatureId::SsrfLabHarness => "SSRF Lab Harness",
            FeatureId::SriSupplyChainAuditor => "SRI Supply Chain Auditor",
            FeatureId::WebsocketAuthzProbe => "WebSocket Authz Probe",
            FeatureId::ToolCallForensics => "Tool Call Forensics",
            FeatureId::JsonSchemaGuard => "JSON Schema Guard",
            FeatureId::CacheHitLedger => "Cache Hit Ledger",
            FeatureId::TokenWasteAuditor => "Token Waste Auditor",
            FeatureId::StreamingSloGates => "Streaming SLO Gates",
            FeatureId::RouterDecisionTrace => "Router Decision Trace",
            FeatureId::McpTrafficLens => "MCP Traffic Lens",
            FeatureId::EvalDatasetMiner => "Eval Dataset Miner",
            FeatureId::HallucinationOracle => "Hallucination Oracle (evidence-gated)",
            FeatureId::EmbeddingRagBench => "Embedding / RAG Bench",
            FeatureId::ProtocolQuirkDiff => "Protocol Quirk Diff",
            FeatureId::LocalCloudParity => "Local / Cloud Parity",
        }
    }

    /// Domain the feature belongs to.
pub fn category(&self) -> FeatureCategory {
        use FeatureId::*;
        match self {
            ComponentTreeLens
            | ResponsiveMatrix
            | CssCascade
            | DesignTokens
            | A11yLens
            | StateTimeTravel
            | ClsHeatmap
            | ComponentSandbox
            | NetworkComponentBinding
            | FontAuditor
            | ZIndexVisualizer
            | HydrationDiffLens
            | CssCoverageMap
            | FocusOrderOverlay
            | EventListenerMap
            | CssVariableInspector
            | LayoutThrashDetector
            | MediaCqMatrix
            | ImageDeliveryAuditor
            | CssContainmentAdvisor
            | AriaLiveMonitor
            | SelectorSpecificityBudget
            | DomSnapshotCapture => FeatureCategory::Frontend,
            ApiWorkbench
            | AuthVault
            | ContractValidation
            | WebhookSink
            | IdorMatrix
            | RequestChaining
            | GrpcGraphql
            | TraceAdjacency
            | MockStub
            | ChaosInjection
            | IdempotencyAuditor
            | DbAdjacency
            | WebhookSignatureVault
            | RateLimitProbe
            | PaginationAuditor
            | CacheSemanticsLens
            | MultipartWorkbench
            | SseWsServerLab
            | OpenApiDiffGate
            | OauthFlowCapture
            | QueueAdjacency
            | GrpcStreamWorkbench
            | LiveStubServer
            | HeaderContractFuzzer => FeatureCategory::Backend,
            AuthzMatrix
            | ResponseDiff
            | HeaderAnalyzer
            | JwtDeep
            | PromoteFinding
            | SurfaceCrawler
            | StorageAuditor
            | FuzzRepeater
            | CsrfAnalyzer
            | SinkPollution
            | TlsInspector
            | OobListener
            | DomXssTaintMap
            | CorsDeepProbe
            | CspPolicyForensics
            | OauthOidcMisconfig
            | CookieSessionForensics
            | SessionFixationLab
            | GraphqlAuthzMatrix
            | MassAssignmentDiff
            | RaceWindowRepeater
            | SsrfLabHarness
            | SriSupplyChainAuditor
            | WebsocketAuthzProbe => FeatureCategory::Security,
            PerfBudgets
            | TraceDiff
            | WaterfallForensics
            | MemoryLeakHunter
            | SyntheticMonitor
            | RumLab
            | HdrHistogramHelper
            | InpAttribution
            | CdnCache
            | VitalsUnderLoad
            | FlameGraph
            | ThirdPartyAttribution
            | PerformanceSla
            | LcpElementForensics
            | LongTaskRadar
            | MainThreadBlockMap
            | BfCacheProbe
            | FreezeResumeBench
            | SoftNavVitals
            | SpeculationLab
            | ConnPoolInspector
            | Http3PathAudit
            | CompressionBudget
            | MediaWeightGate
            | ContinuousProfiler => FeatureCategory::Performance,
            ResilientLocators
            | FlakeRadar
            | VisualRegression
            | IdentityFixtures
            | DataDrivenScenarios
            | RealVitalsGates
            | ContractTesting
            | DeflakedExport
            | CiBundle
            | TimeTravelTrace
            | DeterminismHarness
            | AuthzMatrixTests
            | FlakeTriager
            | QuarantinePolicy
            | ContractDriftGate
            | HermeticFixturePack
            | A11yTestGen
            | SnapshotStrategyPack
            | CoverageAdjacencyMap
            | ParallelShardPlanner
            | TracePropertyMiner
            | RuntimeSelfHeal
            | OracleSynthesizer
            | MutationHookBridge => FeatureCategory::Qa,
            LlmInspector
            | StreamingWaterfall
            | PromptDiff
            | CostDashboard
            | EvalReplay
            | ModelComparison
            | AgentTrace
            | ContextHeatmap
            | RagInspector
            | InjectionScanner
            | LlmLoad
            | DeterministicReplay
            | ToolCallForensics
            | JsonSchemaGuard
            | CacheHitLedger
            | TokenWasteAuditor
            | StreamingSloGates
            | RouterDecisionTrace
            | McpTrafficLens
            | EvalDatasetMiner
            | HallucinationOracle
            | EmbeddingRagBench
            | ProtocolQuirkDiff
            | LocalCloudParity => FeatureCategory::Ai,
        }
    }

    /// Implementation maturity for this feature.
    pub fn status(&self) -> FeatureStatus {
        use FeatureId::*;
        match self {
            // Experimental: live Chromium / network / mutating lab surfaces.
            OobListener
            | ChaosInjection
            | AgentTrace
            | FlameGraph
            | RuntimeSelfHeal
            | DomSnapshotCapture
            | SseWsServerLab
            | GrpcStreamWorkbench
            | LiveStubServer
            | SessionFixationLab
            | RaceWindowRepeater
            | SsrfLabHarness
            | FreezeResumeBench
            | SpeculationLab
            | ContinuousProfiler
            | MutationHookBridge
            | SoftNavVitals
            | MultipartWorkbench
            | McpTrafficLens => FeatureStatus::Experimental,
            // Beta: newer analyzers still stabilizing heuristics.
            SinkPollution
            | RagInspector
            | InjectionScanner
            | VisualRegression
            | ThirdPartyAttribution
            | MemoryLeakHunter
            | FlakeTriager
            | ContractDriftGate
            | TracePropertyMiner
            | HydrationDiffLens
            | CssCoverageMap
            | FocusOrderOverlay
            | EventListenerMap
            | CssVariableInspector
            | LayoutThrashDetector
            | MediaCqMatrix
            | ImageDeliveryAuditor
            | CssContainmentAdvisor
            | AriaLiveMonitor
            | SelectorSpecificityBudget
            | WebhookSignatureVault
            | RateLimitProbe
            | PaginationAuditor
            | CacheSemanticsLens
            | OpenApiDiffGate
            | OauthFlowCapture
            | QueueAdjacency
            | HeaderContractFuzzer
            | DomXssTaintMap
            | CorsDeepProbe
            | CspPolicyForensics
            | OauthOidcMisconfig
            | CookieSessionForensics
            | GraphqlAuthzMatrix
            | MassAssignmentDiff
            | SriSupplyChainAuditor
            | WebsocketAuthzProbe
            | LcpElementForensics
            | LongTaskRadar
            | MainThreadBlockMap
            | BfCacheProbe
            | ConnPoolInspector
            | Http3PathAudit
            | CompressionBudget
            | MediaWeightGate
            | ToolCallForensics
            | JsonSchemaGuard
            | CacheHitLedger
            | TokenWasteAuditor
            | StreamingSloGates
            | RouterDecisionTrace
            | EvalDatasetMiner
            | HallucinationOracle
            | EmbeddingRagBench
            | ProtocolQuirkDiff
            | LocalCloudParity => FeatureStatus::Beta,
            _ => FeatureStatus::Stable,
        }
    }
}

/// Static descriptor for a registered feature.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureDescriptor {
    pub id: FeatureId,
    pub slug: String,
    pub name: String,
    pub category: FeatureCategory,
    pub status: FeatureStatus,
}

impl FeatureDescriptor {
    fn of(id: FeatureId) -> Self {
        Self {
            id,
            slug: id.slug().to_string(),
            name: id.name().to_string(),
            category: id.category(),
            status: id.status(),
        }
    }
}

/// Immutable catalog of every registered feature.
#[derive(Debug, Clone)]
pub struct FeatureRegistry {
    descriptors: Vec<FeatureDescriptor>,
}

impl Default for FeatureRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl FeatureRegistry {
    pub fn new() -> Self {
        let descriptors = FeatureId::ALL
            .iter()
            .map(|id| FeatureDescriptor::of(*id))
            .collect();
        Self { descriptors }
    }

    pub fn all(&self) -> &[FeatureDescriptor] {
        &self.descriptors
    }

    pub fn len(&self) -> usize {
        self.descriptors.len()
    }

    pub fn is_empty(&self) -> bool {
        self.descriptors.is_empty()
    }

    pub fn get(&self, id: FeatureId) -> Option<&FeatureDescriptor> {
        self.descriptors.iter().find(|d| d.id == id)
    }

    pub fn by_slug(&self, slug: &str) -> Option<&FeatureDescriptor> {
        self.descriptors.iter().find(|d| d.slug == slug)
    }

    pub fn by_category(&self, category: FeatureCategory) -> Vec<&FeatureDescriptor> {
        self.descriptors
            .iter()
            .filter(|d| d.category == category)
            .collect()
    }
}

/// Per-feature mutable runtime state.
#[derive(Debug, Clone)]
struct FeatureRuntime {
    enabled: bool,
    invocations: u64,
}

impl Default for FeatureRuntime {
    fn default() -> Self {
        Self {
            enabled: true,
            invocations: 0,
        }
    }
}

/// Outcome of routing a single feature invocation through the orchestrator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureInvocation {
    pub id: FeatureId,
    pub category: FeatureCategory,
    /// Running invocation count after this call.
    pub invocations: u64,
}

/// Serializable status snapshot for one feature.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureStatusReport {
    pub id: FeatureId,
    pub slug: String,
    pub name: String,
    pub category: FeatureCategory,
    pub status: FeatureStatus,
    pub enabled: bool,
    pub invocations: u64,
}

/// Process-wide engine exposing all registered features and their domain sub-engines.
///
/// Prefer the shared instance from `AppState::global().features()` over
/// constructing a new engine per IPC message.
pub struct FeatureEngine {
    registry: FeatureRegistry,
    runtime: Mutex<HashMap<FeatureId, FeatureRuntime>>,
    frontend: FrontendEngine,
    backend: BackendEngine,
    security: SecurityEngine,
    perf: PerfEngine,
    qa: QaEngine,
    ai: AiEngine,
    sla: SlaEngine,
    cdp: CdpBridge,
}

impl Default for FeatureEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl FeatureEngine {
    pub fn new() -> Self {
        let mut runtime = HashMap::with_capacity(FeatureId::ALL.len());
        for id in FeatureId::ALL {
            runtime.insert(id, FeatureRuntime::default());
        }
        let backend = BackendEngine::new();
        backend.store_auth(
            "anonymous",
            backend::AuthScheme::ApiKey {
                header: "X-Thor-Identity".into(),
                key: "anonymous".into(),
            },
        );
        backend.store_auth(
            "user",
            backend::AuthScheme::Bearer {
                token: "user-demo-token".into(),
            },
        );
        backend.store_auth(
            "admin",
            backend::AuthScheme::Bearer {
                token: "admin-demo-token".into(),
            },
        );
        Self {
            registry: FeatureRegistry::new(),
            runtime: Mutex::new(runtime),
            frontend: FrontendEngine::new(),
            backend,
            security: SecurityEngine::new(),
            perf: PerfEngine::new(),
            qa: QaEngine::new(),
            ai: AiEngine::new(),
            sla: SlaEngine::new(),
            cdp: CdpBridge::new(),
        }
    }

    pub fn registry(&self) -> &FeatureRegistry {
        &self.registry
    }

    pub fn frontend(&self) -> &FrontendEngine {
        &self.frontend
    }

    pub fn backend(&self) -> &BackendEngine {
        &self.backend
    }

    /// Security specialist sub-engine (distinct from `AppState::security()`
    /// shield toggles).
    pub fn security(&self) -> &SecurityEngine {
        &self.security
    }

    pub fn perf(&self) -> &PerfEngine {
        &self.perf
    }

    pub fn qa(&self) -> &QaEngine {
        &self.qa
    }

    pub fn ai(&self) -> &AiEngine {
        &self.ai
    }

    pub fn sla(&self) -> &SlaEngine {
        &self.sla
    }

    /// CDP capture bridge feeding real browser/network data into the engines.
    pub fn cdp(&self) -> &CdpBridge {
        &self.cdp
    }

    /// Evaluate JS on the active Chromium tab (via [`CdpBridge::evaluate_on_active`]).
    /// Missing runtime → clear `Err` (never empty success).
    pub fn evaluate_on_active(&self, script: &str) -> Result<String, String> {
        CdpBridge::evaluate_on_active(script)
    }

    pub fn is_enabled(&self, id: FeatureId) -> bool {
        self.runtime
            .lock()
            .get(&id)
            .map(|r| r.enabled)
            .unwrap_or(false)
    }

    pub fn set_enabled(&self, id: FeatureId, enabled: bool) {
        if let Some(r) = self.runtime.lock().get_mut(&id) {
            r.enabled = enabled;
        }
    }

    /// Record an invocation of a feature and return the new count. Returns
    /// `None` when the feature is disabled.
    pub fn record_invocation(&self, id: FeatureId) -> Option<u64> {
        let mut guard = self.runtime.lock();
        let r = guard.get_mut(&id)?;
        if !r.enabled {
            return None;
        }
        r.invocations += 1;
        Some(r.invocations)
    }

    /// Route an invocation by feature slug through the orchestrator: resolve the
    /// slug to its registered descriptor, verify the feature is enabled, and
    /// record the invocation. This is the hot path used by shell/IPC dispatch to
    /// fan a chrome command out to its backing engine, so it must stay fast
    /// (see `sla::SLA_FEATURE_INVOKE_ROUTING`).
    pub fn invoke(&self, slug: &str) -> Result<FeatureInvocation, String> {
        let descriptor = self
            .registry
            .by_slug(slug)
            .ok_or_else(|| format!("unknown feature slug: {slug}"))?;
        let id = descriptor.id;
        let category = descriptor.category;
        let invocations = self
            .record_invocation(id)
            .ok_or_else(|| format!("feature disabled: {slug}"))?;
        Ok(FeatureInvocation {
            id,
            category,
            invocations,
        })
    }

    pub fn invocation_count(&self, id: FeatureId) -> u64 {
        self.runtime
            .lock()
            .get(&id)
            .map(|r| r.invocations)
            .unwrap_or(0)
    }

    /// Full status snapshot across all registered features.
    pub fn status_report(&self) -> Vec<FeatureStatusReport> {
        let guard = self.runtime.lock();
        self.registry
            .all()
            .iter()
            .map(|d| {
                let rt = guard.get(&d.id).cloned().unwrap_or_default();
                FeatureStatusReport {
                    id: d.id,
                    slug: d.slug.clone(),
                    name: d.name.clone(),
                    category: d.category,
                    status: d.status,
                    enabled: rt.enabled,
                    invocations: rt.invocations,
                }
            })
            .collect()
    }

    pub fn status_report_json(&self) -> String {
        serde_json::to_string(&self.status_report()).unwrap_or_else(|_| "[]".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_has_all_features() {
        let engine = FeatureEngine::new();
        assert_eq!(engine.registry().len(), FeatureId::ALL.len());
        assert_eq!(FeatureId::ALL.len(), 144);
    }

    #[test]
    fn test_no_duplicate_slugs() {
        let mut seen = std::collections::HashSet::new();
        for id in FeatureId::ALL {
            assert!(seen.insert(id.slug()), "duplicate slug: {}", id.slug());
        }
        assert_eq!(seen.len(), FeatureId::ALL.len());
    }

    #[test]
    fn test_category_counts() {
        let reg = FeatureRegistry::new();
        assert_eq!(reg.by_category(FeatureCategory::Frontend).len(), 23);
        assert_eq!(reg.by_category(FeatureCategory::Backend).len(), 24);
        assert_eq!(reg.by_category(FeatureCategory::Security).len(), 24);
        assert_eq!(reg.by_category(FeatureCategory::Performance).len(), 25);
        assert_eq!(reg.by_category(FeatureCategory::Qa).len(), 24);
        assert_eq!(reg.by_category(FeatureCategory::Ai).len(), 24);
    }

    #[test]
    fn test_invocation_tracking_and_toggle() {
        let engine = FeatureEngine::new();
        assert_eq!(engine.record_invocation(FeatureId::JwtDeep), Some(1));
        assert_eq!(engine.record_invocation(FeatureId::JwtDeep), Some(2));
        assert_eq!(engine.invocation_count(FeatureId::JwtDeep), 2);

        engine.set_enabled(FeatureId::JwtDeep, false);
        assert_eq!(engine.record_invocation(FeatureId::JwtDeep), None);
        assert!(!engine.is_enabled(FeatureId::JwtDeep));
    }

    #[test]
    fn test_lookup_by_slug() {
        let reg = FeatureRegistry::new();
        assert_eq!(
            reg.by_slug("jwt-deep").map(|d| d.id),
            Some(FeatureId::JwtDeep)
        );
        assert!(reg.by_slug("nonexistent").is_none());
    }
}

use super::{
    BTreeMap, BackendEngine, OauthFlowReport, OauthFlowStep, QueueAdjacencyReport, QueueHop,
};

impl BackendEngine {
    pub fn oauth_flow_capture(&self, steps: &[OauthFlowStep]) -> OauthFlowReport {
        let mut issues = Vec::new();
        let mut has_pkce = false;
        let mut has_state = false;
        for s in steps {
            if s.params.contains_key("code_challenge") || s.params.contains_key("code_verifier") {
                has_pkce = true;
            }
            if s.params.contains_key("state") {
                has_state = true;
            }
            if s.step == "authorize" && !s.params.contains_key("redirect_uri") {
                issues.push("authorize missing redirect_uri".into());
            }
            if s.step == "token"
                && s.params.get("grant_type").map(|g| g.as_str()) == Some("authorization_code")
                && !s.params.contains_key("code")
            {
                issues.push("token exchange missing code".into());
            }
            if s.url.contains("http://")
                && !s.url.contains("localhost")
                && !s.url.contains("127.0.0.1")
            {
                issues.push(format!("insecure http endpoint: {}", s.url));
            }
        }
        if !has_state {
            issues.push("flow missing CSRF state parameter".into());
        }
        if !has_pkce {
            issues.push("flow missing PKCE".into());
        }
        OauthFlowReport {
            steps: steps.len(),
            issues,
            has_pkce,
            has_state,
        }
    }

    pub fn queue_adjacency(&self, hops: &[QueueHop]) -> QueueAdjacencyReport {
        let mut nodes = BTreeMap::<String, ()>::new();
        let mut edges = Vec::new();
        let mut topic_hits: BTreeMap<String, usize> = BTreeMap::new();
        let mut max_latency = 0.0f64;
        for h in hops {
            nodes.insert(h.from.clone(), ());
            nodes.insert(h.to.clone(), ());
            edges.push((h.from.clone(), h.to.clone()));
            *topic_hits.entry(h.topic.clone()).or_default() += 1;
            max_latency = max_latency.max(h.latency_ms);
        }
        let hottest_topic = topic_hits
            .into_iter()
            .max_by_key(|(_, c)| *c)
            .map(|(t, _)| t);
        QueueAdjacencyReport {
            nodes: nodes.into_keys().collect(),
            edges,
            hottest_topic,
            max_latency_ms: max_latency,
        }
    }
}

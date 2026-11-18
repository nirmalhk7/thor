use super::{SecurityEngine, WsAuthzProbe, WsAuthzReport};

impl SecurityEngine {
    pub fn websocket_authz_probe(&self, probes: &[WsAuthzProbe]) -> WsAuthzReport {
        let mut escalations = 0usize;
        let mut findings = Vec::new();
        for p in probes {
            match (p.should_allow, p.connected) {
                (false, true) => {
                    escalations += 1;
                    findings.push(format!(
                        "{} connected to {} but should be denied",
                        p.role, p.url
                    ));
                }
                (true, false) => {
                    findings.push(format!("{} denied from {} unexpectedly", p.role, p.url))
                }
                _ => {}
            }
        }
        WsAuthzReport {
            escalations,
            findings,
        }
    }
}

use super::{
    AuthzCell, AuthzMatrixReport, AuthzProbe, GraphqlAuthzProbe, GraphqlAuthzReport,
    MassAssignDiff, MassAssignReport, SecurityEngine,
};

impl SecurityEngine {
    // 1. Authorization Matrix ----------------------------------------------
    pub fn build_authz_matrix(&self, probes: &[AuthzProbe]) -> AuthzMatrixReport {
        let mut cells = Vec::new();
        let mut escalations = 0;
        let mut over_restrictions = 0;
        for p in probes {
            let allowed = (200..300).contains(&p.status);
            let verdict = match (allowed, p.should_allow) {
                (true, false) => {
                    escalations += 1;
                    "PRIVILEGE-ESCALATION"
                }
                (false, true) => {
                    over_restrictions += 1;
                    "OVER-RESTRICTED"
                }
                _ => "OK",
            };
            cells.push(AuthzCell {
                role: p.role.clone(),
                endpoint: p.endpoint.clone(),
                allowed,
                expected: p.should_allow,
                verdict: verdict.to_string(),
            });
        }
        AuthzMatrixReport {
            cells,
            escalations,
            over_restrictions,
        }
    }

    pub fn graphql_authz_matrix(&self, probes: &[GraphqlAuthzProbe]) -> GraphqlAuthzReport {
        let mut escalations = 0usize;
        let mut cells = Vec::new();
        for p in probes {
            let allowed = (200..300).contains(&p.status) && p.errors.is_empty();
            let verdict = match (p.should_allow, allowed) {
                (false, true) => {
                    escalations += 1;
                    "PRIVILEGE-ESCALATION"
                }
                (true, false) => "FALSE-DENY",
                (true, true) => "OK-ALLOW",
                (false, false) => "OK-DENY",
            };
            cells.push(format!("{} {} {}: {verdict}", p.role, p.operation, p.field));
        }
        GraphqlAuthzReport { escalations, cells }
    }

    pub fn mass_assignment_diff(&self, diff: &MassAssignDiff) -> MassAssignReport {
        let sent: std::collections::HashSet<_> = diff.sent_fields.iter().collect();
        let accepted: std::collections::HashSet<_> = diff.accepted_fields.iter().collect();
        let privileged: std::collections::HashSet<_> = diff.privileged_fields.iter().collect();
        let unexpected: Vec<String> = accepted.difference(&sent).map(|s| (*s).clone()).collect();
        let privileged_accepted: Vec<String> = accepted
            .intersection(&privileged)
            .map(|s| (*s).clone())
            .collect();
        let risk = if !privileged_accepted.is_empty() {
            "high"
        } else if !unexpected.is_empty() {
            "medium"
        } else {
            "low"
        };
        MassAssignReport {
            unexpected_writes: unexpected,
            privileged_accepted,
            risk: risk.to_string(),
        }
    }
}

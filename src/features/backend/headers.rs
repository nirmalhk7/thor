use super::{
    ApiRequest, BTreeMap, BackendEngine, HashMap, HeaderContract, HeaderFuzzCase, HeaderFuzzReport,
};

impl BackendEngine {
    pub fn header_contract_fuzzer(
        &self,
        base: &BTreeMap<String, String>,
        contract: &HeaderContract,
    ) -> HeaderFuzzReport {
        self.header_contract_fuzzer_gated(base, contract, None, false, false)
            .expect("non-execute header fuzz cannot fail")
    }

    /// Generate header mutation cases; when `execute` + `url`, consent-gated
    /// `send_blocking` records status per case.
    pub fn header_contract_fuzzer_gated(
        &self,
        base: &BTreeMap<String, String>,
        contract: &HeaderContract,
        url: Option<&str>,
        execute: bool,
        lab_consent: bool,
    ) -> Result<HeaderFuzzReport, String> {
        let mut cases = Vec::new();
        for reqh in &contract.required {
            let mut h = base.clone();
            h.remove(reqh);
            cases.push(HeaderFuzzCase {
                mutation: format!("drop:{reqh}"),
                headers: h,
                status: None,
                error: None,
            });
        }
        for forb in &contract.forbidden {
            let mut h = base.clone();
            h.insert(forb.clone(), "1".into());
            cases.push(HeaderFuzzCase {
                mutation: format!("inject:{forb}"),
                headers: h,
                status: None,
                error: None,
            });
        }
        for name in contract.patterns.keys() {
            let mut h = base.clone();
            h.insert(name.clone(), "!!!invalid!!!".into());
            cases.push(HeaderFuzzCase {
                mutation: format!("break-pattern:{name}"),
                headers: h,
                status: None,
                error: None,
            });
        }
        if let Some(ct) = base.get("content-type").cloned() {
            let mut h = base.clone();
            h.insert("Content-Type".into(), ct);
            h.remove("content-type");
            cases.push(HeaderFuzzCase {
                mutation: "case-smash:content-type".into(),
                headers: h,
                status: None,
                error: None,
            });
        }

        if !execute {
            return Ok(HeaderFuzzReport {
                cases,
                executed: false,
            });
        }
        let url = url
            .map(str::trim)
            .filter(|u| !u.is_empty())
            .ok_or_else(|| "header-contract-fuzzer execute requires url".to_string())?;
        crate::net::consent::require_lab_consent(url, lab_consent)?;
        for case in &mut cases {
            let mut headers = HashMap::new();
            for (k, v) in &case.headers {
                headers.insert(k.clone(), v.clone());
            }
            let req = ApiRequest {
                method: "GET".into(),
                url: url.to_string(),
                headers,
                body: None,
            };
            match self.send_blocking(&req) {
                Ok(resp) => {
                    case.status = Some(resp.status);
                    case.error = None;
                }
                Err(e) => {
                    case.status = None;
                    case.error = Some(e);
                }
            }
        }
        Ok(HeaderFuzzReport {
            cases,
            executed: true,
        })
    }
}

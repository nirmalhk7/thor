use super::{SecurityEngine, TlsInfo, TlsReport};

impl SecurityEngine {
    // 11. TLS Inspector -----------------------------------------------------
    pub fn inspect_tls(&self, info: &TlsInfo) -> TlsReport {
        let mut issues = Vec::new();
        let proto = info.protocol.to_uppercase().replace(['.', ' '], "");
        let weak_proto = matches!(
            proto.as_str(),
            "TLS10" | "TLS11" | "SSL30" | "SSLV3" | "TLSV10" | "TLSV11"
        );
        if weak_proto {
            issues.push(format!("deprecated protocol {}", info.protocol));
        }
        let cipher_l = info.cipher.to_lowercase();
        if cipher_l.contains("rc4")
            || cipher_l.contains("3des")
            || cipher_l.contains("null")
            || cipher_l.contains("md5")
        {
            issues.push(format!("weak cipher {}", info.cipher));
        }
        if info.self_signed {
            issues.push("self-signed certificate".into());
        }
        if !info.hostname_matches {
            issues.push("certificate hostname mismatch".into());
        }
        if info.cert_days_remaining < 0 {
            issues.push("certificate expired".into());
        } else if info.cert_days_remaining < 14 {
            issues.push(format!(
                "certificate expires in {} days",
                info.cert_days_remaining
            ));
        }
        let grade = if issues.is_empty() {
            if proto == "TLS13" {
                "A+"
            } else {
                "A"
            }
        } else if weak_proto || info.self_signed || info.cert_days_remaining < 0 {
            "F"
        } else {
            "C"
        };
        TlsReport {
            grade: grade.to_string(),
            issues,
        }
    }
}

//! MiSTer→APF service detection. Reports which MiSTer framework services the core
//! uses (by RTL signature) and the APF equivalent each must be re-implemented as.
//! Services with no signature (or that are universal) are not force-listed — the
//! plan reports what this specific core reveals it needs, plus a note that the scan
//! sees only what the RTL names.

use crate::refdata::ServiceMap;
use crate::source::CoreFiles;
use regex::Regex;
use serde::Serialize;

/// A detected MiSTer service and its APF mapping.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ServiceHit {
    /// The MiSTer service/interface.
    pub mister: String,
    /// The APF equivalent.
    pub apf: String,
    /// The signature token that fired.
    pub matched_signature: String,
    /// Porting note.
    pub note: String,
    /// Public provenance.
    pub provenance: String,
}

/// Detect the MiSTer services the core uses.
pub fn detect_services(files: &CoreFiles, map: &ServiceMap) -> Vec<ServiceHit> {
    let text = files.rtl_text().to_ascii_lowercase();
    let mut out = Vec::new();
    for svc in &map.services {
        if let Some(sig) = svc.signatures.iter().find(|s| {
            let s = s.to_ascii_lowercase();
            token_present(&text, &s)
        }) {
            out.push(ServiceHit {
                mister: svc.mister.clone(),
                apf: svc.apf.clone(),
                matched_signature: sig.clone(),
                note: svc.note.clone(),
                provenance: svc.provenance.clone(),
            });
        }
    }
    out
}

/// Word-ish presence: the signature bounded by non-identifier characters, so
/// `status` does not match inside `statusbar` etc.
fn token_present(text: &str, sig: &str) -> bool {
    let re = Regex::new(&format!(r"(?i)(^|[^a-z0-9_]){}([^a-z0-9_]|$)", regex::escape(sig)));
    match re {
        Ok(re) => re.is_match(text),
        Err(_) => text.contains(sig),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::refdata::RefData;

    #[test]
    fn detects_ioctl_and_video_services() {
        let files = CoreFiles::from_pairs([(
            "rtl/core_top.v",
            "module core_top(); wire ioctl_download; wire ce_pix; wire [7:0] joystick_0; endmodule",
        )]);
        let map = RefData::bundled().unwrap().services;
        let hits = detect_services(&files, &map);
        let misters: Vec<&str> = hits.iter().map(|h| h.mister.as_str()).collect();
        assert!(misters.iter().any(|m| m.contains("ioctl")), "ioctl service detected: {misters:?}");
        assert!(misters.iter().any(|m| m.contains("ce_pix")), "video service detected");
    }

    #[test]
    fn no_false_match_on_substring() {
        let files = CoreFiles::from_pairs([("rtl/x.v", "wire statusbar_thing;")]);
        let map = RefData::bundled().unwrap().services;
        let hits = detect_services(&files, &map);
        assert!(!hits.iter().any(|h| h.matched_signature == "status"), "no substring false match");
    }
}

//! Document-start anti-fingerprinting bootstrap for Thor's Stealth profile.
//!
//! The bootstrap runs before page scripts through CDP and uses Web Crypto for a
//! per-document seed. It deliberately leaves DevHost untouched.

use super::chromium_runtime::ChromiumResult;
use super::tab_session::TabSession;

pub struct FingerprintProtection;

impl FingerprintProtection {
    pub fn stealth_bootstrap() -> &'static str {
        r#"(() => {
  // Temporary defense-in-depth. Canvas/Audio script noise is deliberately
  // absent because per-document perturbation breaks API semantics and creates
  // a detectable instability signal. Native Blink/V8 owns coherent policy.
  const patchWebGl = (proto) => {
    if (!proto) return;
    const getExtension = proto.getExtension;
    proto.getExtension = function(name) {
      if (String(name).toLowerCase() === 'webgl_debug_renderer_info') return null;
      return getExtension.call(this, name);
    };
    const getSupportedExtensions = proto.getSupportedExtensions;
    proto.getSupportedExtensions = function() {
      const supported = getSupportedExtensions.call(this);
      return supported?.filter((name) => name.toLowerCase() !== 'webgl_debug_renderer_info') ?? supported;
    };
  };
  patchWebGl(globalThis.WebGLRenderingContext && WebGLRenderingContext.prototype);
  patchWebGl(globalThis.WebGL2RenderingContext && WebGL2RenderingContext.prototype);
})();"#
    }

    /// Install on future navigations and the current document.
    pub async fn install(session: &TabSession) -> ChromiumResult<()> {
        session
            .inject_script_on_new_document(Self::stealth_bootstrap())
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bootstrap_avoids_unstable_canvas_audio_and_hides_webgl_debug_info() {
        let script = FingerprintProtection::stealth_bootstrap();
        assert!(script.contains("webgl_debug_renderer_info"));
        assert!(!script.contains("HTMLCanvasElement.prototype.toDataURL"));
        assert!(!script.contains("AudioBuffer.prototype.getChannelData"));
        assert!(!script.contains("crypto.getRandomValues"));
        assert!(!script.contains("Thor GPU"));
        assert!(!script.contains("__thorFingerprint"));
    }
}

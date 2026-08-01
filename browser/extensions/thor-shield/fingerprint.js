(() => {
  // Temporary defense-in-depth only. Do not perturb Canvas/Audio in script:
  // per-document noise and copied live views are detectable and API-breaking.
  // Coherent site/session-scoped normalization belongs in native Blink/V8.
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

})();

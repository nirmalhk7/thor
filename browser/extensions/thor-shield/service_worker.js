const RULESET_ID = 'thor_shield_rules';
const SITE_PRIVACY_SETTINGS = ['cookies', 'popups', 'notifications', 'location', 'microphone', 'camera'];
const SITE_POLICY_STORAGE_KEY = 'sitePermissionBlocks';
const FILTER_STATUS_STORAGE_KEY = 'filterBundleStatus';
const FILTER_TRUST_KEYS = Object.freeze({
  'thor-filter-release-2026-02': 'H8NAbS0d4iubjJ17WXXbO5ZEOBVLcRFrRf4IAEJdXe0=',
});
let sitePolicyQueue = Promise.resolve();

const DEFAULT_CONTROLS = {
  doNotTrack: true,
  thirdPartyCookies: false,
  adMeasurement: false,
  fledge: false,
  relatedWebsiteSets: false,
  topics: false,
  networkPrediction: false,
  hyperlinkAuditing: false,
  webRtcPublicInterfaceOnly: true,
  alternateErrorPages: false,
  safeBrowsingExtendedReporting: false,
  searchSuggest: false,
  spellingService: false,
  translationService: false,
};

const CONTROL_SETTINGS = {
  doNotTrack: chrome.privacy.websites.doNotTrackEnabled,
  thirdPartyCookies: chrome.privacy.websites.thirdPartyCookiesAllowed,
  adMeasurement: chrome.privacy.websites.adMeasurementEnabled,
  fledge: chrome.privacy.websites.fledgeEnabled,
  relatedWebsiteSets: chrome.privacy.websites.relatedWebsiteSetsEnabled,
  topics: chrome.privacy.websites.topicsEnabled,
  networkPrediction: chrome.privacy.network.networkPredictionEnabled,
  hyperlinkAuditing: chrome.privacy.network.hyperlinkAuditingEnabled,
  alternateErrorPages: chrome.privacy.services.alternateErrorPagesEnabled,
  safeBrowsingExtendedReporting: chrome.privacy.services.safeBrowsingExtendedReportingEnabled,
  searchSuggest: chrome.privacy.services.searchSuggestEnabled,
  spellingService: chrome.privacy.services.spellingServiceEnabled,
  translationService: chrome.privacy.services.translationServiceEnabled,
};

function setIfControllable(setting, value) {
  if (!setting) return;
  setting.get({}, (details) => {
    if (chrome.runtime.lastError) return;
    if (details.levelOfControl === 'controllable_by_this_extension' ||
        details.levelOfControl === 'controlled_by_this_extension') {
      setting.set({ value });
    }
  });
}

function applyControls(controls) {
  setIfControllable(CONTROL_SETTINGS.doNotTrack, controls.doNotTrack);
  setIfControllable(CONTROL_SETTINGS.thirdPartyCookies, controls.thirdPartyCookies);
  setIfControllable(CONTROL_SETTINGS.adMeasurement, controls.adMeasurement);
  setIfControllable(CONTROL_SETTINGS.fledge, controls.fledge);
  setIfControllable(CONTROL_SETTINGS.relatedWebsiteSets, controls.relatedWebsiteSets);
  setIfControllable(CONTROL_SETTINGS.topics, controls.topics);
  setIfControllable(CONTROL_SETTINGS.networkPrediction, controls.networkPrediction);
  setIfControllable(CONTROL_SETTINGS.hyperlinkAuditing, controls.hyperlinkAuditing);
  setIfControllable(
    chrome.privacy.network.webRTCIPHandlingPolicy,
    controls.webRtcPublicInterfaceOnly ? 'default_public_interface_only' : 'default',
  );
  setIfControllable(CONTROL_SETTINGS.alternateErrorPages, controls.alternateErrorPages);
  setIfControllable(
    CONTROL_SETTINGS.safeBrowsingExtendedReporting,
    controls.safeBrowsingExtendedReporting,
  );
  setIfControllable(CONTROL_SETTINGS.searchSuggest, controls.searchSuggest);
  setIfControllable(CONTROL_SETTINGS.spellingService, controls.spellingService);
  setIfControllable(CONTROL_SETTINGS.translationService, controls.translationService);
}

function loadControls(callback) {
  chrome.storage.local.get({ controls: DEFAULT_CONTROLS }, ({ controls }) => {
    callback({ ...DEFAULT_CONTROLS, ...controls });
  });
}

function restoreControls() {
  loadControls(applyControls);
}

function validOrigin(input) {
  try {
    const url = new URL(input);
    if (url.protocol !== 'http:' && url.protocol !== 'https:') return null;
    return url.origin;
  } catch (_) {
    return null;
  }
}

function validPermissions(input) {
  if (!Array.isArray(input)) return [];
  return [...new Set(input.filter((name) => SITE_PRIVACY_SETTINGS.includes(name)))].sort();
}

function normalizedSitePolicies(input) {
  if (!input || typeof input !== 'object' || Array.isArray(input)) return {};
  const policies = {};
  for (const [candidate, permissions] of Object.entries(input)) {
    const origin = validOrigin(candidate);
    const valid = validPermissions(permissions);
    if (origin && valid.length) policies[origin] = valid;
  }
  return Object.fromEntries(Object.entries(policies).sort(([left], [right]) => left.localeCompare(right)));
}

async function replaceSitePrivacyPolicies(policies) {
  // contentSettings.clear() is extension-wide, not pattern-specific. Rebuild
  // each permission's block rules so reset restores Chromium's inherited/default
  // value instead of installing a persistent `allow` exception.
  for (const name of SITE_PRIVACY_SETTINGS) {
    await chrome.contentSettings[name].clear({ scope: 'regular' });
  }
  for (const [origin, permissions] of Object.entries(policies)) {
    const primaryPattern = `${origin}/*`;
    for (const name of permissions) {
      await chrome.contentSettings[name].set({
        primaryPattern,
        setting: 'block',
        scope: 'regular',
      });
    }
  }
}

async function loadSitePrivacyPolicies() {
  const stored = await chrome.storage.local.get({
    [SITE_POLICY_STORAGE_KEY]: {},
    blockedSiteOrigins: [],
  });
  let policies = normalizedSitePolicies(stored[SITE_POLICY_STORAGE_KEY]);
  // One-time migration from the earlier all-permissions-per-origin format.
  if (!Object.keys(policies).length && Array.isArray(stored.blockedSiteOrigins)) {
    for (const candidate of stored.blockedSiteOrigins) {
      const origin = validOrigin(candidate);
      if (origin) policies[origin] = [...SITE_PRIVACY_SETTINGS];
    }
  }
  policies = normalizedSitePolicies(policies);
  await chrome.storage.local.set({ [SITE_POLICY_STORAGE_KEY]: policies });
  await chrome.storage.local.remove('blockedSiteOrigins');
  return policies;
}

async function setSitePrivacyPolicy(origin, permissions, block) {
  const selected = validPermissions(permissions);
  if (!selected.length) throw new Error('Select at least one site permission.');
  const policies = await loadSitePrivacyPolicies();
  const current = new Set(policies[origin] || []);
  for (const name of selected) {
    if (block) current.add(name);
    else current.delete(name);
  }
  if (current.size) policies[origin] = [...current].sort();
  else delete policies[origin];
  const normalized = normalizedSitePolicies(policies);
  await chrome.storage.local.set({ [SITE_POLICY_STORAGE_KEY]: normalized });
  await replaceSitePrivacyPolicies(normalized);
  return normalized[origin] || [];
}

function enqueueSitePolicyUpdate(operation) {
  const result = sitePolicyQueue.then(operation, operation);
  sitePolicyQueue = result.catch(() => {});
  return result;
}

async function restoreSitePrivacyPolicies() {
  const policies = await loadSitePrivacyPolicies();
  await replaceSitePrivacyPolicies(policies);
}

function base64Bytes(value) {
  const binary = atob(value);
  return Uint8Array.from(binary, (character) => character.charCodeAt(0));
}

function hex(bytes) {
  return [...new Uint8Array(bytes)].map((byte) => byte.toString(16).padStart(2, '0')).join('');
}

function filterBundlePayload(bundle) {
  return JSON.stringify({
    schemaVersion: bundle.schemaVersion,
    bundleVersion: bundle.bundleVersion,
    rulesetId: bundle.rulesetId,
    rulesPath: bundle.rulesPath,
    rulesSha256: bundle.rulesSha256,
    ruleCount: bundle.ruleCount,
    keyId: bundle.keyId,
    issuedAt: bundle.issuedAt,
    expiresAt: bundle.expiresAt,
    issuedAtUnix: bundle.issuedAtUnix,
    expiresAtUnix: bundle.expiresAtUnix,
    source: bundle.source,
    sourceRevision: bundle.sourceRevision,
  });
}

async function verifyFilterBundle() {
  const bundleResponse = await fetch(chrome.runtime.getURL('filter_bundle.json'), { cache: 'no-store' });
  if (!bundleResponse.ok) throw new Error(`filter manifest read failed (${bundleResponse.status})`);
  const bundle = await bundleResponse.json();
  if (bundle.schemaVersion !== 1 || bundle.rulesetId !== RULESET_ID || bundle.rulesPath !== 'rules.json') {
    throw new Error('filter manifest identity is invalid');
  }
  if (!Number.isSafeInteger(bundle.ruleCount) || bundle.ruleCount < 1 ||
      typeof bundle.bundleVersion !== 'string' || !bundle.bundleVersion ||
      typeof bundle.rulesSha256 !== 'string' || !/^[a-f0-9]{64}$/.test(bundle.rulesSha256) ||
      bundle.signatureAlgorithm !== 'Ed25519' || !FILTER_TRUST_KEYS[bundle.keyId] ||
      typeof bundle.issuedAt !== 'string' || typeof bundle.expiresAt !== 'string' ||
      !Number.isSafeInteger(bundle.issuedAtUnix) || !Number.isSafeInteger(bundle.expiresAtUnix) ||
      typeof bundle.source !== 'string' || !bundle.source ||
      typeof bundle.sourceRevision !== 'string' || !bundle.sourceRevision ||
      typeof bundle.signature !== 'string') {
    throw new Error('filter manifest fields are invalid');
  }
  const issuedAt = Date.parse(bundle.issuedAt);
  const expiresAt = Date.parse(bundle.expiresAt);
  if (!Number.isFinite(issuedAt) || !Number.isFinite(expiresAt) ||
      Math.floor(issuedAt / 1000) !== bundle.issuedAtUnix ||
      Math.floor(expiresAt / 1000) !== bundle.expiresAtUnix ||
      issuedAt > Date.now() + 300_000 || expiresAt <= Date.now() || expiresAt <= issuedAt) {
    throw new Error('filter manifest validity window is invalid or expired');
  }

  const rulesResponse = await fetch(chrome.runtime.getURL(bundle.rulesPath), { cache: 'no-store' });
  if (!rulesResponse.ok) throw new Error(`filter rules read failed (${rulesResponse.status})`);
  const rulesBytes = new Uint8Array(await rulesResponse.arrayBuffer());
  const rules = JSON.parse(new TextDecoder().decode(rulesBytes));
  if (!Array.isArray(rules) || rules.length !== bundle.ruleCount) {
    throw new Error('filter rule count does not match signed manifest');
  }
  const digest = hex(await crypto.subtle.digest('SHA-256', rulesBytes));
  if (digest !== bundle.rulesSha256) throw new Error('filter rules digest mismatch');

  const publicKey = await crypto.subtle.importKey(
    'raw',
    base64Bytes(FILTER_TRUST_KEYS[bundle.keyId]),
    { name: 'Ed25519' },
    false,
    ['verify'],
  );
  const verified = await crypto.subtle.verify(
    { name: 'Ed25519' },
    publicKey,
    base64Bytes(bundle.signature),
    new TextEncoder().encode(filterBundlePayload(bundle)),
  );
  if (!verified) throw new Error('filter manifest signature is invalid');
  return { verified: true, bundleVersion: bundle.bundleVersion, ruleCount: bundle.ruleCount };
}

async function ensureFilterBundleIntegrity() {
  try {
    const status = await verifyFilterBundle();
    await chrome.declarativeNetRequest.updateEnabledRulesets({
      enableRulesetIds: [RULESET_ID],
      disableRulesetIds: [],
    });
    const result = { ...status, checkedAt: new Date().toISOString() };
    await chrome.storage.local.set({ [FILTER_STATUS_STORAGE_KEY]: result });
    return result;
  } catch (error) {
    await chrome.declarativeNetRequest.updateEnabledRulesets({
      enableRulesetIds: [],
      disableRulesetIds: [RULESET_ID],
    });
    const result = {
      verified: false,
      error: error?.message || String(error),
      checkedAt: new Date().toISOString(),
    };
    await chrome.storage.local.set({ [FILTER_STATUS_STORAGE_KEY]: result });
    console.error('Tracker rules disabled: signed filter bundle verification failed', error);
    return result;
  }
}

function readPrivacySetting(setting) {
  return new Promise((resolve) => {
    if (!setting) return resolve({ available: false });
    setting.get({}, (details) => {
      if (chrome.runtime.lastError) {
        resolve({ available: false, error: chrome.runtime.lastError.message });
      } else {
        resolve({
          available: true,
          value: details.value,
          levelOfControl: details.levelOfControl,
        });
      }
    });
  });
}

async function privacyStatus() {
  const filterBundle = await ensureFilterBundleIntegrity();
  const enabledRulesets = await chrome.declarativeNetRequest.getEnabledRulesets();
  return {
    implementation: 'extension-transitional',
    filterBundle,
    filterRulesetEnabled: enabledRulesets.includes(RULESET_ID),
    // The extension cannot prove native Blink preference state (and does not
    // run in non-primary OTR identities). Keep this explicitly unverified;
    // packaged-browser acceptance tests own the authoritative readback.
    globalPrivacyControl: { state: 'unverified-native-readback-required' },
    webRtcIpHandlingPolicy: await readPrivacySetting(chrome.privacy.network.webRTCIPHandlingPolicy),
  };
}

function clearSiteData(origin, done) {
  chrome.browsingData.remove(
    { origins: [origin] },
    {
      appcache: true,
      cache: true,
      cacheStorage: true,
      cookies: true,
      fileSystems: true,
      indexedDB: true,
      localStorage: true,
      serviceWorkers: true,
      webSQL: true,
    },
    () => done(chrome.runtime.lastError?.message || null),
  );
}

chrome.runtime.onInstalled.addListener(() => {
  ensureFilterBundleIntegrity();
  restoreControls();
  enqueueSitePolicyUpdate(restoreSitePrivacyPolicies)
    .catch((error) => console.error('Could not restore site privacy rules', error));
});
chrome.runtime.onStartup.addListener(() => {
  ensureFilterBundleIntegrity();
  restoreControls();
  enqueueSitePolicyUpdate(restoreSitePrivacyPolicies)
    .catch((error) => console.error('Could not restore site privacy rules', error));
});
chrome.action.onClicked.addListener(() => chrome.runtime.openOptionsPage());

chrome.runtime.onMessage.addListener((message, _sender, sendResponse) => {
  if (message?.type === 'thor-shield-get-controls') {
    loadControls((controls) => sendResponse({ controls }));
    return true;
  }
  if (message?.type === 'thor-shield-set-controls') {
    const controls = { ...DEFAULT_CONTROLS, ...message.controls };
    chrome.storage.local.set({ controls }, () => {
      applyControls(controls);
      sendResponse({ controls });
    });
    return true;
  }
  if (message?.type === 'thor-shield-set-site-policy') {
    const origin = validOrigin(message.origin);
    if (!origin) {
      sendResponse({ error: 'Enter an http(s) site origin, for example https://example.com.' });
      return false;
    }
    enqueueSitePolicyUpdate(() => setSitePrivacyPolicy(
      origin,
      message.permissions,
      Boolean(message.block),
    ))
      .then((blockedPermissions) => sendResponse({
        origin,
        blocked: Boolean(message.block),
        blockedPermissions,
      }))
      .catch((error) => sendResponse({ error: error.message || String(error) }));
    return true;
  }
  if (message?.type === 'thor-shield-get-status') {
    privacyStatus()
      .then((result) => sendResponse({ status: result }))
      .catch((error) => sendResponse({ error: error.message || String(error) }));
    return true;
  }
  if (message?.type === 'thor-shield-clear-site-data') {
    const origin = validOrigin(message.origin);
    if (!origin) {
      sendResponse({ error: 'Enter an http(s) site origin, for example https://example.com.' });
      return false;
    }
    clearSiteData(origin, (error) => sendResponse(error ? { error } : { origin, cleared: true }));
    return true;
  }
  return false;
});

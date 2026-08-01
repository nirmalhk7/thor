const CONTROL_LABELS = [
  ['doNotTrack', 'Send Do Not Track'],
  ['thirdPartyCookies', 'Allow third-party cookies'],
  ['adMeasurement', 'Allow ad measurement'],
  ['fledge', 'Allow Protected Audience / FLEDGE'],
  ['relatedWebsiteSets', 'Allow Related Website Sets'],
  ['topics', 'Allow Privacy Sandbox Topics'],
  ['networkPrediction', 'Allow network prediction'],
  ['hyperlinkAuditing', 'Allow hyperlink auditing / ping'],
  ['webRtcPublicInterfaceOnly', 'Limit WebRTC to the public network interface'],
  ['alternateErrorPages', 'Allow alternate error pages'],
  ['safeBrowsingExtendedReporting', 'Allow Safe Browsing extended reporting'],
  ['searchSuggest', 'Allow search suggestions'],
  ['spellingService', 'Allow cloud spelling service'],
  ['translationService', 'Allow translation service'],
];

const controls = document.querySelector('#controls');
const status = document.querySelector('#status');
const enforcementStatus = document.querySelector('#enforcement-status');
let current = {};

const SITE_PERMISSION_LABELS = [
  ['cookies', 'Cookies'],
  ['popups', 'Pop-ups'],
  ['notifications', 'Notifications'],
  ['location', 'Location'],
  ['microphone', 'Microphone'],
  ['camera', 'Camera'],
];

for (const [key, label] of CONTROL_LABELS) {
  const row = document.createElement('label');
  const checkbox = document.createElement('input');
  checkbox.type = 'checkbox';
  checkbox.id = key;
  checkbox.addEventListener('change', save);
  row.append(checkbox, document.createTextNode(label));
  controls.append(row);
}

function render(next) {
  current = next;
  for (const [key] of CONTROL_LABELS) document.querySelector(`#${key}`).checked = Boolean(current[key]);
}

function save() {
  const next = { ...current };
  for (const [key] of CONTROL_LABELS) next[key] = document.querySelector(`#${key}`).checked;
  chrome.runtime.sendMessage({ type: 'thor-shield-set-controls', controls: next }, (response) => {
    if (chrome.runtime.lastError) {
      status.textContent = `Could not save: ${chrome.runtime.lastError.message}`;
      return;
    }
    render(response.controls);
    status.textContent = 'Saved. Thor applied every setting Chromium permits this extension to control.';
  });
}

chrome.runtime.sendMessage({ type: 'thor-shield-get-controls' }, (response) => {
  if (chrome.runtime.lastError) {
    status.textContent = `Could not load controls: ${chrome.runtime.lastError.message}`;
    return;
  }
  render(response.controls);
});

const sitePermissions = document.querySelector('#site-permissions');
for (const [key, label] of SITE_PERMISSION_LABELS) {
  const row = document.createElement('label');
  const checkbox = document.createElement('input');
  checkbox.type = 'checkbox';
  checkbox.value = key;
  checkbox.checked = true;
  row.append(checkbox, document.createTextNode(label));
  sitePermissions.append(row);
}

function selectedPermissions() {
  return [...sitePermissions.querySelectorAll('input:checked')].map((input) => input.value);
}

function requireSelectedPermissions() {
  const permissions = selectedPermissions();
  if (!permissions.length) status.textContent = 'Select at least one site permission.';
  return permissions;
}

function siteOrigin() {
  return document.querySelector('#site-origin').value.trim();
}

function reportSiteResponse(response, success) {
  if (chrome.runtime.lastError) {
    status.textContent = `Could not apply site setting: ${chrome.runtime.lastError.message}`;
  } else if (response?.error) {
    status.textContent = response.error;
  } else {
    status.textContent = success;
  }
}

document.querySelector('#block-site').addEventListener('click', () => {
  const permissions = requireSelectedPermissions();
  if (!permissions.length) return;
  chrome.runtime.sendMessage(
    { type: 'thor-shield-set-site-policy', origin: siteOrigin(), permissions, block: true },
    (response) => reportSiteResponse(response, 'Blocked selected site permissions.'),
  );
});
document.querySelector('#reset-site').addEventListener('click', () => {
  const permissions = requireSelectedPermissions();
  if (!permissions.length) return;
  chrome.runtime.sendMessage(
    { type: 'thor-shield-set-site-policy', origin: siteOrigin(), permissions, block: false },
    (response) => reportSiteResponse(response, 'Removed selected Thor Shield overrides for this site.'),
  );
});

chrome.runtime.sendMessage({ type: 'thor-shield-get-status' }, (response) => {
  if (chrome.runtime.lastError) {
    enforcementStatus.textContent = `Status unavailable: ${chrome.runtime.lastError.message}`;
    return;
  }
  if (response?.error) {
    enforcementStatus.textContent = `Status unavailable: ${response.error}`;
    return;
  }
  const privacy = response.status;
  const filter = privacy?.filterBundle;
  const rtc = privacy?.webRtcIpHandlingPolicy;
  const filterText = filter?.verified && privacy.filterRulesetEnabled
    ? `signed filter ${filter.bundleVersion} verified (${filter.ruleCount} rules)`
    : `filter not enforced${filter?.error ? `: ${filter.error}` : ''}`;
  const rtcText = rtc?.available
    ? `WebRTC ${rtc.value} (${rtc.levelOfControl})`
    : 'WebRTC status unavailable';
  const gpcText = privacy?.globalPrivacyControl?.state === 'unverified-native-readback-required'
    ? 'GPC native readback required'
    : 'GPC status unavailable';
  enforcementStatus.textContent = `${filterText}; ${gpcText}; ${rtcText}. Extension-transitional enforcement.`;
});
document.querySelector('#clear-site').addEventListener('click', () => {
  chrome.runtime.sendMessage(
    { type: 'thor-shield-clear-site-data', origin: siteOrigin() },
    (response) => reportSiteResponse(response, 'Cleared cookies, storage, service workers, and cache for this site.'),
  );
});

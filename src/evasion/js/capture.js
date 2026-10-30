// Reads what this page can see of the browser and returns it as a plain object.
//
// Written as the body of an async function (see `evasion::capture`). Every reading is
// guarded on its own, so one that throws or never answers costs that reading and
// nothing else: the reason goes to `errors`. An API that the browser does not have is
// `{ supported: false }`; an API that only a secure context has, asked in an insecure
// one, is `{ unknown: ... }`, because its absence says nothing about the browser.
// Nothing here asks for a permission, loads a resource or writes to the page.
const errors = {};
const note = (name, error) => {
  errors[name] = String((error && error.message) || error);
};
const attempt = (name, read, fallback = null) => {
  try {
    const value = read();
    return value === undefined ? fallback : value;
  } catch (error) {
    note(name, error);
    return fallback;
  }
};
const attemptAsync = async (name, read, fallback = null) => {
  try {
    const value = await read();
    return value === undefined ? fallback : value;
  } catch (error) {
    note(name, error);
    return fallback;
  }
};
const within = (promise, ms, what) =>
  Promise.race([
    promise,
    new Promise((resolve, reject) => setTimeout(() => reject(new Error(what + ' did not answer in ' + ms + ' ms')), ms)),
  ]);
const finite = (number) => (typeof number === 'number' && Number.isFinite(number) ? number : null);
const secure = window.isSecureContext === true;
// The answer for an API that is not there: absent for the browser, or unknown when
// only a secure context would have it.
const missing = (secureOnly) =>
  secureOnly && !secure ? { unknown: 'the API needs a secure context' } : { supported: false };
// FNV-1a over the UTF-16 code units, 32 bits, in hex.
const fnv = (text) => {
  let hash = 0x811c9dc5;
  for (let i = 0; i < text.length; i++) {
    hash ^= text.charCodeAt(i);
    hash = Math.imul(hash, 0x01000193) >>> 0;
  }
  return hash.toString(16).padStart(8, '0');
};

const nav = navigator;
const ua = attempt('navigator.userAgent', () => nav.userAgent, '');

const navigatorReading = {
  userAgent: ua,
  platform: attempt('navigator.platform', () => nav.platform),
  language: attempt('navigator.language', () => nav.language),
  languages: attempt('navigator.languages', () => Array.from(nav.languages || [])),
  hardwareConcurrency: attempt('navigator.hardwareConcurrency', () => nav.hardwareConcurrency),
  deviceMemory: attempt('navigator.deviceMemory', () => (typeof nav.deviceMemory === 'number' ? nav.deviceMemory : null)),
  maxTouchPoints: attempt('navigator.maxTouchPoints', () => nav.maxTouchPoints),
  vendor: attempt('navigator.vendor', () => nav.vendor),
  productSub: attempt('navigator.productSub', () => nav.productSub),
  appVersion: attempt('navigator.appVersion', () => nav.appVersion),
  oscpu: attempt('navigator.oscpu', () => (typeof nav.oscpu === 'string' ? nav.oscpu : null)),
  buildID: attempt('navigator.buildID', () => (typeof nav.buildID === 'string' ? nav.buildID : null)),
  webdriver: attempt('navigator.webdriver', () => (typeof nav.webdriver === 'boolean' ? nav.webdriver : null)),
  pluginsLength: attempt('navigator.plugins', () => (nav.plugins ? nav.plugins.length : null)),
  mimeTypesLength: attempt('navigator.mimeTypes', () => (nav.mimeTypes ? nav.mimeTypes.length : null)),
  cookieEnabled: attempt('navigator.cookieEnabled', () => nav.cookieEnabled),
  doNotTrack: attempt('navigator.doNotTrack', () => (nav.doNotTrack === undefined ? null : nav.doNotTrack)),
  pdfViewerEnabled: attempt('navigator.pdfViewerEnabled', () => (typeof nav.pdfViewerEnabled === 'boolean' ? nav.pdfViewerEnabled : null)),
};

const screenReading = attempt('screen', () => ({
  width: screen.width,
  height: screen.height,
  availWidth: screen.availWidth,
  availHeight: screen.availHeight,
  colorDepth: screen.colorDepth,
  pixelDepth: screen.pixelDepth,
  devicePixelRatio: window.devicePixelRatio,
  orientation: screen.orientation ? { type: screen.orientation.type, angle: screen.orientation.angle } : null,
}));

const windowReading = attempt('window', () => ({
  innerWidth: window.innerWidth,
  innerHeight: window.innerHeight,
  outerWidth: window.outerWidth,
  outerHeight: window.outerHeight,
  screenX: window.screenX,
  screenY: window.screenY,
}));

const intl = attempt('intl', () => {
  const resolved = Intl.DateTimeFormat().resolvedOptions();
  return { timeZone: resolved.timeZone, locale: resolved.locale, offsetMinutes: new Date().getTimezoneOffset() };
});

const webglReading = attempt('webgl', () => {
  const canvas = document.createElement('canvas');
  const gl = canvas.getContext('webgl') || canvas.getContext('experimental-webgl');
  if (!gl) {
    return { supported: false };
  }
  const reading = {
    supported: true,
    vendor: gl.getParameter(gl.VENDOR),
    renderer: gl.getParameter(gl.RENDERER),
    version: gl.getParameter(gl.VERSION),
    shadingLanguageVersion: gl.getParameter(gl.SHADING_LANGUAGE_VERSION),
  };
  const debug = gl.getExtension('WEBGL_debug_renderer_info');
  if (debug) {
    reading.unmaskedVendor = gl.getParameter(debug.UNMASKED_VENDOR_WEBGL);
    reading.unmaskedRenderer = gl.getParameter(debug.UNMASKED_RENDERER_WEBGL);
  }
  return reading;
}, { unknown: 'WebGL could not be read' });

const userAgentData = await attemptAsync('userAgentData', async () => {
  const data = nav.userAgentData;
  if (!data) {
    return missing(true);
  }
  const reading = {
    supported: true,
    brands: Array.from(data.brands || []).map((b) => ({ brand: b.brand, version: b.version })),
    mobile: data.mobile,
    platform: data.platform,
  };
  const high = await within(
    data.getHighEntropyValues([
      'architecture', 'bitness', 'model', 'platformVersion', 'uaFullVersion', 'fullVersionList', 'wow64', 'formFactors',
    ]),
    2000,
    'getHighEntropyValues'
  );
  reading.architecture = high.architecture;
  reading.bitness = high.bitness;
  reading.model = high.model;
  reading.platformVersion = high.platformVersion;
  reading.uaFullVersion = high.uaFullVersion;
  reading.fullVersionList = Array.from(high.fullVersionList || []).map((b) => ({ brand: b.brand, version: b.version }));
  reading.wow64 = high.wow64;
  reading.formFactors = high.formFactors === undefined ? null : Array.from(high.formFactors);
  return reading;
}, { unknown: 'Client Hints could not be read' });

const mediaDevices = await attemptAsync('mediaDevices', async () => {
  if (!nav.mediaDevices || typeof nav.mediaDevices.enumerateDevices !== 'function') {
    return missing(true);
  }
  const list = await within(nav.mediaDevices.enumerateDevices(), 2000, 'enumerateDevices');
  const counts = { audioinput: 0, audiooutput: 0, videoinput: 0 };
  for (const device of list) {
    if (device.kind in counts) {
      counts[device.kind] += 1;
    }
  }
  return { supported: true, audioInputs: counts.audioinput, audioOutputs: counts.audiooutput, videoInputs: counts.videoinput };
}, { unknown: 'media devices could not be listed' });

const battery = await attemptAsync('battery', async () => {
  if (typeof nav.getBattery !== 'function') {
    return missing(true);
  }
  const state = await within(nav.getBattery(), 2000, 'getBattery');
  return {
    supported: true,
    charging: state.charging,
    level: finite(state.level),
    chargingTime: finite(state.chargingTime),
    dischargingTime: finite(state.dischargingTime),
  };
}, { unknown: 'the battery could not be read' });

const connection = attempt('connection', () => {
  const link = nav.connection || nav.mozConnection || nav.webkitConnection;
  if (!link) {
    return { supported: false };
  }
  return {
    supported: true,
    effectiveType: link.effectiveType === undefined ? null : link.effectiveType,
    rtt: finite(link.rtt),
    downlink: finite(link.downlink),
    saveData: link.saveData === undefined ? null : link.saveData,
    type: link.type === undefined ? null : link.type,
  };
}, { unknown: 'the connection could not be read' });

const speech = await attemptAsync('speech', async () => {
  if (!('speechSynthesis' in window) || !window.speechSynthesis) {
    return { supported: false };
  }
  let voices = window.speechSynthesis.getVoices();
  if (voices.length === 0) {
    // Voices load after the first call; give them a moment to arrive.
    await new Promise((resolve) => {
      const done = () => resolve();
      window.speechSynthesis.addEventListener('voiceschanged', done, { once: true });
      setTimeout(done, 800);
    });
    voices = window.speechSynthesis.getVoices();
  }
  return {
    supported: true,
    voices: Array.from(voices).map((v) => ({
      name: v.name,
      lang: v.lang,
      default: v.default,
      localService: v.localService,
      voiceURI: v.voiceURI,
    })),
  };
}, { unknown: 'the voices could not be listed' });

const bluetooth = await attemptAsync('bluetooth', async () => {
  if (!nav.bluetooth) {
    return missing(true);
  }
  const available = typeof nav.bluetooth.getAvailability === 'function'
    ? await within(nav.bluetooth.getAvailability(), 2000, 'getAvailability')
    : null;
  return { supported: true, available };
}, { unknown: 'Bluetooth could not be read' });

// States only: asking is not requesting, so no prompt is shown.
const permissions = {};
if (nav.permissions && typeof nav.permissions.query === 'function') {
  const names = [
    'geolocation', 'notifications', 'camera', 'microphone', 'midi', 'clipboard-read', 'clipboard-write',
    'persistent-storage', 'background-sync', 'accelerometer', 'gyroscope', 'magnetometer',
  ];
  await Promise.all(names.map(async (name) => {
    try {
      const status = await within(nav.permissions.query({ name }), 1000, 'permissions.query');
      permissions[name] = status.state;
    } catch (error) {
      permissions[name] = null;
    }
  }));
}

const canvasHash = attempt('canvasHash', () => {
  const canvas = document.createElement('canvas');
  canvas.width = 240;
  canvas.height = 48;
  const context = canvas.getContext('2d');
  if (!context) {
    return null;
  }
  context.textBaseline = 'top';
  context.font = '14px Arial';
  context.fillStyle = '#f60';
  context.fillRect(100, 1, 62, 20);
  context.fillStyle = '#069';
  context.fillText('sbrs fingerprint capture', 2, 15);
  context.fillStyle = 'rgba(102, 204, 0, 0.7)';
  context.fillText('sbrs fingerprint capture', 4, 17);
  return fnv(canvas.toDataURL());
});

const audioHash = await attemptAsync('audioHash', async () => {
  const Offline = window.OfflineAudioContext || window.webkitOfflineAudioContext;
  if (!Offline) {
    return null;
  }
  const context = new Offline(1, 5000, 44100);
  const oscillator = context.createOscillator();
  oscillator.type = 'triangle';
  oscillator.frequency.value = 10000;
  const compressor = context.createDynamicsCompressor();
  const settings = { threshold: -50, knee: 40, ratio: 12, attack: 0, release: 0.25 };
  for (const key of Object.keys(settings)) {
    if (compressor[key] && compressor[key].setValueAtTime) {
      compressor[key].setValueAtTime(settings[key], context.currentTime);
    }
  }
  oscillator.connect(compressor);
  compressor.connect(context.destination);
  oscillator.start(0);
  const rendered = await within(context.startRendering(), 3000, 'startRendering');
  const samples = rendered.getChannelData(0);
  let sum = 0;
  for (let i = 4500; i < samples.length; i++) {
    sum += Math.abs(samples[i]);
  }
  return String(sum);
});

// What a script that looks for an automated or headless browser would find.
const headless = [];
const mark = (name, condition) => {
  try {
    if (condition()) {
      headless.push(name);
    }
  } catch (error) {
    note('headless.' + name, error);
  }
};
const isChromium = /Chrome\//.test(ua);
mark('user_agent_says_headless', () => /Headless/i.test(ua));
mark('navigator_webdriver_true', () => nav.webdriver === true);
mark('no_window_chrome', () => isChromium && !window.chrome);
mark('no_plugins', () => isChromium && nav.plugins && nav.plugins.length === 0);
mark('no_languages', () => !nav.languages || nav.languages.length === 0);
mark('zero_outer_size', () => window.outerWidth === 0 && window.outerHeight === 0);
mark('notification_permission_mismatch', () =>
  typeof Notification !== 'undefined' && Notification.permission === 'denied' && permissions.notifications === 'prompt');
mark('software_renderer', () =>
  webglReading && /SwiftShader|llvmpipe|Mesa OffScreen/i.test(String(webglReading.unmaskedRenderer || webglReading.renderer || '')));
mark('zero_rtt', () => connection && connection.rtt === 0);

return {
  version: 1,
  url: attempt('url', () => location.href, ''),
  secureContext: secure,
  navigator: navigatorReading,
  userAgentData,
  screen: screenReading,
  window: windowReading,
  intl,
  webgl: webglReading,
  mediaDevices,
  battery,
  connection,
  speech,
  bluetooth,
  permissions,
  canvasHash,
  audioHash,
  headless,
  errors,
};

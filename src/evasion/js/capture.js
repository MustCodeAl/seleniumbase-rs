// Reads what this page can see of the browser and returns it as a plain object.
//
// Written as the body of an async function (see `evasion::capture`). Every reading is
// guarded on its own, so one that throws or never answers costs that reading and
// nothing else: the reason goes to `errors`. An API that the browser does not have is
// `{ supported: false }`; an API that only a secure context has, asked in an insecure
// one, is `{ unknown: ... }`, because its absence says nothing about the browser.
// Nothing here asks for a permission or loads a resource. Three readings add a
// hidden frame, a style sheet or a span to the page for a moment and remove it
// again, and one starts a short-lived worker.
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
  globalPrivacyControl: attempt('navigator.globalPrivacyControl', () =>
    (typeof nav.globalPrivacyControl === 'boolean' ? nav.globalPrivacyControl : null)),
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
  isExtended: typeof screen.isExtended === 'boolean' ? screen.isExtended : null,
}));

const windowReading = attempt('window', () => ({
  innerWidth: window.innerWidth,
  innerHeight: window.innerHeight,
  outerWidth: window.outerWidth,
  outerHeight: window.outerHeight,
  screenX: window.screenX,
  screenY: window.screenY,
  visualViewportWidth: window.visualViewport ? window.visualViewport.width : null,
  visualViewportHeight: window.visualViewport ? window.visualViewport.height : null,
}));

const intl = attempt('intl', () => {
  const resolved = Intl.DateTimeFormat().resolvedOptions();
  // Every Intl constructor defaults to the browser's own locale; a spoof that
  // changes one and not the others is a lie a detector reads in one line.
  const locales = {};
  for (const [name, Ctor] of [
    ['Collator', Intl.Collator], ['NumberFormat', Intl.NumberFormat], ['PluralRules', Intl.PluralRules],
    ['RelativeTimeFormat', Intl.RelativeTimeFormat], ['ListFormat', Intl.ListFormat],
  ]) {
    try {
      if (typeof Ctor === 'function') { locales[name] = new Ctor().resolvedOptions().locale; }
    } catch (error) {
      note('intl.' + name, error);
    }
  }
  // Two fixed instants, so the offsets can be checked against the zone later,
  // daylight saving time included.
  const january = new Date(Date.UTC(2024, 0, 15, 12));
  const july = new Date(Date.UTC(2024, 6, 15, 12));
  return {
    timeZone: resolved.timeZone,
    locale: resolved.locale,
    offsetMinutes: new Date().getTimezoneOffset(),
    januaryOffsetMinutes: january.getTimezoneOffset(),
    julyOffsetMinutes: july.getTimezoneOffset(),
    dateString: january.toString(),
    numberSample: (1234567.891).toLocaleString(),
    locales,
  };
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
    maxTextureSize: gl.getParameter(gl.MAX_TEXTURE_SIZE),
    maxRenderbufferSize: gl.getParameter(gl.MAX_RENDERBUFFER_SIZE),
    maxViewportDims: Array.from(gl.getParameter(gl.MAX_VIEWPORT_DIMS) || []),
    maxVertexUniformVectors: gl.getParameter(gl.MAX_VERTEX_UNIFORM_VECTORS),
    aliasedLineWidthRange: Array.from(gl.getParameter(gl.ALIASED_LINE_WIDTH_RANGE) || []),
    extensions: gl.getSupportedExtensions() || [],
    webgl2: !!document.createElement('canvas').getContext('webgl2'),
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

// The engine: V8 prints eval in 33 characters, SpiderMonkey and JavaScriptCore in 37.
const engine = attempt('engine', () => ({ etsl: eval.toString().length }));

const notificationPermission = attempt('notificationPermission', () =>
  (typeof Notification !== 'undefined' && Notification.permission ? Notification.permission : null));

// The CSS media features, read twice: through matchMedia, and through @media
// rules in a style sheet, which a patched matchMedia does not reach. In each
// list the last value that matches wins, so a wider gamut beats a narrower one.
const MEDIA_FEATURES = {
  pointer: ['none', 'coarse', 'fine'],
  'any-pointer': ['none', 'coarse', 'fine'],
  hover: ['none', 'hover'],
  'any-hover': ['none', 'hover'],
  'color-gamut': ['srgb', 'p3', 'rec2020'],
  'prefers-color-scheme': ['light', 'dark'],
  'prefers-reduced-motion': ['no-preference', 'reduce'],
  'prefers-reduced-transparency': ['no-preference', 'reduce'],
  'prefers-contrast': ['no-preference', 'less', 'more', 'custom'],
  'forced-colors': ['none', 'active'],
  'inverted-colors': ['none', 'inverted'],
  'dynamic-range': ['standard', 'high'],
  update: ['none', 'slow', 'fast'],
  'display-mode': ['browser', 'minimal-ui', 'standalone', 'fullscreen'],
  orientation: ['portrait', 'landscape'],
};
const mediaFeatures = attempt('mediaFeatures', () => {
  const out = {};
  for (const feature of Object.keys(MEDIA_FEATURES)) {
    out[feature] = null;
    for (const value of MEDIA_FEATURES[feature]) {
      if (window.matchMedia('(' + feature + ': ' + value + ')').matches) { out[feature] = value; }
    }
  }
  out.monochrome = window.matchMedia('(monochrome)').matches;
  let bits = 0;
  for (let c = 1; c <= 16; c++) {
    if (window.matchMedia('(color: ' + c + ')').matches) { bits = c; }
  }
  out.color = bits;
  out.deviceWidth = screenReading ? window.matchMedia('(device-width: ' + screenReading.width + 'px)').matches : null;
  return out;
}, { unknown: 'the media features could not be read' });
const cssMedia = attempt('cssMedia', () => {
  const read = ['pointer', 'any-pointer', 'hover', 'any-hover', 'color-gamut', 'prefers-color-scheme', 'prefers-reduced-motion', 'forced-colors'];
  let css = '';
  for (const feature of read) {
    for (const value of MEDIA_FEATURES[feature]) {
      css += '@media (' + feature + ': ' + value + ') { :root { --sbrs-' + feature + ': ' + value + '; } }\n';
    }
  }
  const style = document.createElement('style');
  style.textContent = css;
  (document.head || document.documentElement).appendChild(style);
  try {
    const computed = getComputedStyle(document.documentElement);
    const out = {};
    for (const feature of read) {
      const value = computed.getPropertyValue('--sbrs-' + feature).trim();
      out[feature] = value || null;
    }
    return out;
  } finally {
    style.remove();
  }
}, { unknown: 'the style sheet media features could not be read' });

// Codecs, through the three answers one media engine gives: canPlayType,
// MediaSource.isTypeSupported and mediaCapabilities.decodingInfo, plus the
// codecs WebRTC can receive.
const CODEC_TYPES = {
  h264: 'video/mp4; codecs="avc1.42E01E"',
  h264Aac: 'video/mp4; codecs="avc1.42E01E, mp4a.40.2"',
  hevc: 'video/mp4; codecs="hev1.1.6.L93.B0"',
  vp8: 'video/webm; codecs="vp8"',
  vp9: 'video/webm; codecs="vp9"',
  av1: 'video/mp4; codecs="av01.0.05M.08"',
  theora: 'video/ogg; codecs="theora"',
  mp3: 'audio/mpeg',
  aac: 'audio/mp4; codecs="mp4a.40.2"',
  aacRaw: 'audio/aac',
  m4a: 'audio/x-m4a',
  vorbis: 'audio/ogg; codecs="vorbis"',
  flac: 'audio/ogg; codecs="flac"',
  wav: 'audio/wav; codecs="1"',
  opus: 'audio/webm; codecs="opus"',
};
const codecs = await attemptAsync('codecs', async () => {
  const out = { canPlayType: {}, isTypeSupported: {}, decodingInfo: {}, rtcVideo: null, rtcAudio: null };
  const element = document.createElement('video');
  const mediaSource = window.MediaSource && typeof window.MediaSource.isTypeSupported === 'function';
  for (const name of Object.keys(CODEC_TYPES)) {
    const type = CODEC_TYPES[name];
    out.canPlayType[name] = typeof element.canPlayType === 'function' ? element.canPlayType(type) : null;
    out.isTypeSupported[name] = mediaSource ? window.MediaSource.isTypeSupported(type) : null;
  }
  if (nav.mediaCapabilities && typeof nav.mediaCapabilities.decodingInfo === 'function') {
    const video = (contentType) => ({ type: 'file', video: { contentType, width: 1920, height: 1080, bitrate: 4000000, framerate: 30 } });
    const audio = (contentType) => ({ type: 'file', audio: { contentType, channels: '2', bitrate: 128000, samplerate: 44100 } });
    const asked = {
      h264: video(CODEC_TYPES.h264),
      vp9: video('video/webm; codecs="vp09.00.10.08"'),
      hevc: video(CODEC_TYPES.hevc),
      aac: audio(CODEC_TYPES.aac),
    };
    for (const name of Object.keys(asked)) {
      try {
        const info = await within(nav.mediaCapabilities.decodingInfo(asked[name]), 2000, 'decodingInfo');
        out.decodingInfo[name] = { supported: info.supported, smooth: info.smooth, powerEfficient: info.powerEfficient };
      } catch (error) {
        note('codecs.decodingInfo.' + name, error);
        out.decodingInfo[name] = null;
      }
    }
  }
  if (window.RTCRtpReceiver && typeof window.RTCRtpReceiver.getCapabilities === 'function') {
    const mimes = (kind) => {
      const capabilities = window.RTCRtpReceiver.getCapabilities(kind);
      return capabilities ? Array.from(new Set(capabilities.codecs.map((codec) => codec.mimeType))).sort() : null;
    };
    out.rtcVideo = mimes('video');
    out.rtcAudio = mimes('audio');
  }
  return out;
}, { unknown: 'the codecs could not be read' });

// WebGPU: the adapter names the GPU's maker, which the WebGL renderer names too.
const webgpu = await attemptAsync('webgpu', async () => {
  if (!nav.gpu || typeof nav.gpu.requestAdapter !== 'function') {
    return missing(true);
  }
  const adapter = await within(nav.gpu.requestAdapter(), 3000, 'requestAdapter');
  if (!adapter) {
    return { supported: true, adapter: false };
  }
  let info = adapter.info || null;
  if (!info && typeof adapter.requestAdapterInfo === 'function') {
    info = await within(adapter.requestAdapterInfo(), 2000, 'requestAdapterInfo');
  }
  return {
    supported: true,
    adapter: true,
    vendor: info ? info.vendor : null,
    architecture: info ? info.architecture : null,
    device: info ? info.device : null,
    description: info ? info.description : null,
    isFallbackAdapter: info && typeof info.isFallbackAdapter === 'boolean' ? info.isFallbackAdapter : null,
    features: adapter.features ? Array.from(adapter.features).length : null,
    maxTextureDimension2D: adapter.limits ? adapter.limits.maxTextureDimension2D : null,
  };
}, { unknown: 'WebGPU could not be read' });

const keyboard = await attemptAsync('keyboard', async () => {
  if (!nav.keyboard || typeof nav.keyboard.getLayoutMap !== 'function') {
    return { supported: false };
  }
  const map = await within(nav.keyboard.getLayoutMap(), 2000, 'getLayoutMap');
  const sample = {};
  for (const code of ['KeyQ', 'KeyW', 'KeyY', 'KeyZ', 'KeyA', 'KeyM', 'Semicolon', 'Quote', 'BracketLeft', 'Minus', 'Equal', 'Backslash', 'Digit2']) {
    sample[code] = map.get(code) === undefined ? null : map.get(code);
  }
  return { supported: true, size: map.size, sample };
}, { unknown: 'the keyboard map could not be read' });

const storage = await attemptAsync('storage', async () => {
  if (!nav.storage || typeof nav.storage.estimate !== 'function') {
    return missing(true);
  }
  const estimate = await within(nav.storage.estimate(), 2000, 'storage.estimate');
  return { supported: true, quota: finite(estimate.quota), usage: finite(estimate.usage) };
}, { unknown: 'the storage quota could not be read' });

// Which APIs are there: the engine and the operating system decide, so a
// persona of another system shows the wrong set.
const features = attempt('features', () => ({
  chrome: 'chrome' in window,
  chromeRuntime: !!(window.chrome && window.chrome.runtime),
  chromeCsi: !!(window.chrome && typeof window.chrome.csi === 'function'),
  chromeLoadTimes: !!(window.chrome && typeof window.chrome.loadTimes === 'function'),
  userAgentData: 'userAgentData' in nav,
  deviceMemory: 'deviceMemory' in nav,
  getBattery: typeof nav.getBattery === 'function',
  connection: 'connection' in nav,
  oscpu: 'oscpu' in nav,
  share: typeof nav.share === 'function',
  canShare: typeof nav.canShare === 'function',
  hid: 'hid' in nav,
  serial: 'serial' in nav,
  usb: 'usb' in nav,
  xr: 'xr' in nav,
  contacts: 'contacts' in nav,
  bluetooth: 'bluetooth' in nav,
  gpu: 'gpu' in nav,
  keyboard: 'keyboard' in nav,
  virtualKeyboard: 'virtualKeyboard' in nav,
  windowControlsOverlay: 'windowControlsOverlay' in nav,
  getGamepads: typeof nav.getGamepads === 'function',
  vibrate: typeof nav.vibrate === 'function',
  setAppBadge: typeof nav.setAppBadge === 'function',
  audioSession: 'audioSession' in nav,
  eyeDropper: 'EyeDropper' in window,
  documentPictureInPicture: 'documentPictureInPicture' in window,
  barcodeDetector: 'BarcodeDetector' in window,
  notification: 'Notification' in window,
  applePaySession: 'ApplePaySession' in window,
  safari: 'safari' in window,
  webkitSpeechRecognition: 'webkitSpeechRecognition' in window,
  pressureObserver: 'PressureObserver' in window,
  sharedArrayBuffer: 'SharedArrayBuffer' in window,
  touchEvent: typeof TouchEvent !== 'undefined',
  onTouchStart: 'ontouchstart' in window,
}));

// A dedicated worker reads the same machine through its own navigator: a
// patched window next to an unpatched worker is the plainest lie there is.
const worker = await attemptAsync('worker', async () => {
  if (typeof Worker === 'undefined' || typeof Blob === 'undefined' || typeof URL === 'undefined') {
    return { supported: false };
  }
  const source = [
    'const out = { errors: {} };',
    'const put = (key, read) => {',
    '  try { const value = read(); out[key] = value === undefined ? null : value; }',
    '  catch (error) { out.errors[key] = String((error && error.message) || error); }',
    '};',
    "put('userAgent', () => navigator.userAgent);",
    "put('platform', () => navigator.platform);",
    "put('language', () => navigator.language);",
    "put('languages', () => Array.from(navigator.languages || []));",
    "put('hardwareConcurrency', () => navigator.hardwareConcurrency);",
    "put('deviceMemory', () => (typeof navigator.deviceMemory === 'number' ? navigator.deviceMemory : null));",
    "put('timeZone', () => Intl.DateTimeFormat().resolvedOptions().timeZone);",
    "put('locale', () => Intl.DateTimeFormat().resolvedOptions().locale);",
    "put('offsetMinutes', () => new Date().getTimezoneOffset());",
    "put('uaPlatform', () => (navigator.userAgentData ? navigator.userAgentData.platform : null));",
    "put('uaMobile', () => (navigator.userAgentData ? navigator.userAgentData.mobile : null));",
    "put('webgl', () => {",
    "  if (typeof OffscreenCanvas === 'undefined') { return null; }",
    "  const gl = new OffscreenCanvas(1, 1).getContext('webgl');",
    '  if (!gl) { return null; }',
    "  const debug = gl.getExtension('WEBGL_debug_renderer_info');",
    '  return debug ? { vendor: gl.getParameter(debug.UNMASKED_VENDOR_WEBGL), renderer: gl.getParameter(debug.UNMASKED_RENDERER_WEBGL) } : null;',
    '});',
    'self.postMessage(out);',
  ].join('\n');
  const url = URL.createObjectURL(new Blob([source], { type: 'application/javascript' }));
  let instance = null;
  try {
    instance = new Worker(url);
    const reading = await within(new Promise((resolve, reject) => {
      instance.onmessage = (event) => resolve(event.data);
      instance.onerror = (event) => reject(new Error((event && event.message) || 'the worker failed'));
    }), 3000, 'the worker');
    return Object.assign({ supported: true }, reading);
  } finally {
    if (instance) { instance.terminate(); }
    URL.revokeObjectURL(url);
  }
}, { unknown: 'a worker could not be started' });

// An about:blank frame is the same browser again; a page patched in the top
// window alone shows the truth in the frame.
const frame = attempt('frame', () => {
  const host = document.body || document.documentElement;
  const iframe = document.createElement('iframe');
  iframe.style.display = 'none';
  iframe.src = 'about:blank';
  host.appendChild(iframe);
  try {
    const inner = iframe.contentWindow;
    const n = inner.navigator;
    return {
      supported: true,
      userAgent: n.userAgent,
      platform: n.platform,
      language: n.language,
      hardwareConcurrency: n.hardwareConcurrency,
      deviceMemory: typeof n.deviceMemory === 'number' ? n.deviceMemory : null,
      webdriver: typeof n.webdriver === 'boolean' ? n.webdriver : null,
      chrome: 'chrome' in inner,
      screenWidth: inner.screen.width,
      timeZone: inner.Intl.DateTimeFormat().resolvedOptions().timeZone,
    };
  } finally {
    iframe.remove();
  }
}, { unknown: 'a frame could not be read' });

// Fonts by measurement, as FingerprintJS and CreepJS find them: a family that
// renders at another width than the generic fallback is installed. The list
// holds the fonts that ship with one system and no other, plus three common ones.
const FONT_PROBES = [
  'Segoe UI', 'Calibri', 'Cambria', 'Consolas', 'Tahoma', 'Microsoft Sans Serif',
  'Helvetica Neue', 'Menlo', 'Monaco', 'Geneva', 'Lucida Grande', 'Avenir Next',
  'DejaVu Sans', 'Liberation Sans', 'Ubuntu', 'Cantarell', 'Noto Sans',
  'Roboto', 'Droid Sans', 'Arial', 'Times New Roman', 'Courier New',
];
const fonts = attempt('fonts', () => {
  const host = document.body || document.documentElement;
  const span = document.createElement('span');
  span.textContent = 'mmmmmmmmmmlli';
  span.style.cssText = 'position:absolute;left:-9999px;top:0;font-size:72px;visibility:hidden;white-space:nowrap';
  host.appendChild(span);
  try {
    const generics = ['monospace', 'sans-serif', 'serif'];
    const widths = {};
    for (const generic of generics) {
      span.style.fontFamily = generic;
      widths[generic] = span.getBoundingClientRect().width;
    }
    const present = [];
    for (const family of FONT_PROBES) {
      const installed = generics.some((generic) => {
        span.style.fontFamily = "'" + family + "', " + generic;
        return Math.abs(span.getBoundingClientRect().width - widths[generic]) > 0.1;
      });
      if (installed) { present.push(family); }
    }
    return { probed: FONT_PROBES.length, present };
  } finally {
    span.remove();
  }
}, { unknown: 'the fonts could not be measured' });


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
mark('screen_800x600', () => !!screenReading && screenReading.width === 800 && screenReading.height === 600);
mark('no_pointing_device', () => !!mediaFeatures && mediaFeatures['any-pointer'] === 'none');
mark('brands_say_headless', () =>
  !!userAgentData && Array.isArray(userAgentData.brands) && userAgentData.brands.some((b) => /Headless/i.test(String(b.brand))));
mark('window_larger_than_screen', () =>
  !!windowReading && !!screenReading && (windowReading.outerWidth > screenReading.width || windowReading.outerHeight > screenReading.height));
mark('software_webgpu', () => !!webgpu && webgpu.isFallbackAdapter === true);

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
  engine,
  notificationPermission,
  mediaFeatures,
  cssMedia,
  codecs,
  webgpu,
  keyboard,
  storage,
  features,
  worker,
  frame,
  fonts,
  errors,
};

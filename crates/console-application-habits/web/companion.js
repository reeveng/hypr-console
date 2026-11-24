const api = globalThis.browser ?? globalThis.chrome;
let port;
let revision = 0;

function connection() {
  if (!port) {
    const native = api.runtime.connectNative("org.console.habits");
    native.onDisconnect.addListener(() => {
      // Read Chrome's lastError so a missing host can be retried quietly.
      void api.runtime.lastError;
      if (port === native) port = undefined;
    });
    port = native;
  }
  return port;
}

async function sendActiveDomain() {
  const checking = ++revision;
  try {
    const window = await api.windows.getLastFocused({ windowTypes: ["normal"] });
    if (!window.focused || window.incognito) return;
    const [tab] = await api.tabs.query({ active: true, windowId: window.id });
    if (checking !== revision || !tab || tab.incognito) return;

    let domain = null;
    if (tab.url) {
      const url = new URL(tab.url);
      if (url.protocol === "http:" || url.protocol === "https:") {
        domain = url.hostname;
      }
    }
    connection().postMessage({ domain });
  } catch {
    // The window or the native port may close during an asynchronous lookup.
  }
}

api.windows.onFocusChanged.addListener(sendActiveDomain);
api.tabs.onActivated.addListener(sendActiveDomain);
api.tabs.onUpdated.addListener((_id, change, tab) => {
  if (tab.active && change.url !== undefined) sendActiveDomain();
});
api.runtime.onStartup.addListener(sendActiveDomain);
api.alarms.onAlarm.addListener(sendActiveDomain);
api.alarms.create("active-domain", { periodInMinutes: 0.5 });
setInterval(sendActiveDomain, 10_000);
sendActiveDomain();

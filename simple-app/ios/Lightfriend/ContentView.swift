import SwiftUI
import WebKit

let defaultServerURL = "https://lightfriend.ai"
let legacyLocalServerURL = "http://localhost:3000"

private func normalizedServerURL(_ rawValue: String?) -> String? {
    guard var url = rawValue?.trimmingCharacters(in: .whitespacesAndNewlines),
          !url.isEmpty else {
        return nil
    }

    if !url.hasPrefix("http://") && !url.hasPrefix("https://") {
        url = "http://" + url
    }
    if url.hasSuffix("/") {
        url.removeLast()
    }

    return url
}

private func resolvedLaunchOverrideServerURL() -> String? {
    let processInfo = ProcessInfo.processInfo

    if let envURL = normalizedServerURL(processInfo.environment["LIGHTFRIEND_SERVER_URL"]) {
        return envURL
    }

    let arguments = processInfo.arguments
    for (index, argument) in arguments.enumerated() {
        guard argument == "-serverURL" || argument == "--server-url" else { continue }
        let valueIndex = index + 1
        guard valueIndex < arguments.count else { break }
        return normalizedServerURL(arguments[valueIndex])
    }

    return nil
}

private func resolvedInitialServerURL() -> String {
    if let launchOverride = resolvedLaunchOverrideServerURL() {
        UserDefaults.standard.set(launchOverride, forKey: "server_url")
        return launchOverride
    }

    let savedURL = UserDefaults.standard.string(forKey: "server_url")?.trimmingCharacters(in: .whitespacesAndNewlines)

    guard let savedURL, !savedURL.isEmpty else {
        return defaultServerURL
    }

    if savedURL == legacyLocalServerURL {
        UserDefaults.standard.set(defaultServerURL, forKey: "server_url")
        return defaultServerURL
    }

    return normalizedServerURL(savedURL) ?? defaultServerURL
}

struct ContentView: View {
    @State private var serverURL: String = resolvedInitialServerURL()
    @State private var showSettings = false
    @State private var loadError: String?

    var body: some View {
        ZStack(alignment: .topTrailing) {
            LightfriendWebView(serverURL: serverURL, loadError: $loadError)
                .id(serverURL)
                .ignoresSafeArea(.container, edges: .bottom)

            if let loadError {
                VStack(alignment: .leading, spacing: 10) {
                    Label("App failed to load", systemImage: "exclamationmark.triangle.fill")
                        .font(.system(size: 16, weight: .semibold))
                        .foregroundColor(.white)

                    Text(loadError)
                        .font(.system(size: 13))
                        .foregroundColor(.white.opacity(0.78))
                        .fixedSize(horizontal: false, vertical: true)

                    Text("You can still open Settings and change the server URL if needed.")
                        .font(.system(size: 12))
                        .foregroundColor(.white.opacity(0.55))
                        .fixedSize(horizontal: false, vertical: true)
                }
                .padding(16)
                .background(Color.black.opacity(0.82))
                .overlay(
                    RoundedRectangle(cornerRadius: 16)
                        .stroke(Color.red.opacity(0.28), lineWidth: 1)
                )
                .cornerRadius(16)
                .padding(.horizontal, 16)
                .padding(.top, 72)
            }

            Button {
                showSettings = true
            } label: {
                Image(systemName: "gearshape.fill")
                    .font(.system(size: 16, weight: .semibold))
                    .foregroundColor(.white.opacity(0.9))
                    .padding(12)
                    .background(Color.black.opacity(0.45))
                    .clipShape(Circle())
                    .overlay(
                        Circle()
                            .stroke(Color.white.opacity(0.08), lineWidth: 1)
                    )
            }
            .padding(.top, 12)
            .padding(.trailing, 16)
        }
        .sheet(isPresented: $showSettings) {
            SettingsView(serverURL: $serverURL)
        }
    }
}

// MARK: - Settings
struct SettingsView: View {
    @Binding var serverURL: String
    @State private var inputURL: String = ""
    @Environment(\.dismiss) var dismiss

    var body: some View {
        NavigationView {
            ZStack {
                Color(red: 0.067, green: 0.067, blue: 0.067)
                    .ignoresSafeArea()

                VStack(spacing: 16) {
                    VStack(alignment: .leading, spacing: 8) {
                        Text("Server URL")
                            .font(.system(size: 13))
                            .foregroundColor(.white.opacity(0.4))

                        TextField(defaultServerURL, text: $inputURL)
                            .textFieldStyle(.plain)
                            .padding(14)
                            .background(Color.black.opacity(0.3))
                            .cornerRadius(10)
                            .overlay(
                                RoundedRectangle(cornerRadius: 10)
                                    .stroke(Color(red: 0.118, green: 0.565, blue: 1.0).opacity(0.2), lineWidth: 1)
                            )
                            .foregroundColor(.white)
                            .autocorrectionDisabled()
                            .textInputAutocapitalization(.never)
                            .keyboardType(.URL)
                    }
                    .padding(16)
                    .background(Color.white.opacity(0.04))
                    .cornerRadius(12)

                    Button(action: save) {
                        Text("Save & Reload")
                            .frame(maxWidth: .infinity)
                            .padding(14)
                            .background(Color(red: 0.118, green: 0.565, blue: 1.0).opacity(0.2))
                            .foregroundColor(Color(red: 0.494, green: 0.698, blue: 1.0))
                            .cornerRadius(10)
                            .overlay(
                                RoundedRectangle(cornerRadius: 10)
                                    .stroke(Color(red: 0.118, green: 0.565, blue: 1.0).opacity(0.2), lineWidth: 1)
                            )
                    }

                    Spacer()
                }
                .padding(16)
            }
            .navigationTitle("Settings")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .navigationBarTrailing) {
                    Button("Done") { dismiss() }
                        .foregroundColor(Color(red: 0.494, green: 0.698, blue: 1.0))
                }
            }
        }
        .onAppear {
            inputURL = serverURL
        }
    }

    func save() {
        let url = normalizedServerURL(inputURL) ?? defaultServerURL
        serverURL = url
        UserDefaults.standard.set(url, forKey: "server_url")
        dismiss()
    }
}

// MARK: - API Proxy via WKScriptMessageHandler
// All fetch() calls from JS are intercepted and routed through Swift's URLSession.
// This completely bypasses CORS — no custom schemes, no browser networking.
class APIProxy: NSObject, WKScriptMessageHandler, URLSessionDataDelegate {
    let serverURL: String
    weak var webView: WKWebView?
    private var taskMap: [Int: String] = [:]                 // URLSession taskID -> request ID
    private var activeRequests: [String: URLSessionDataTask] = [:] // request ID -> task
    private let lock = NSLock()
    private lazy var session: URLSession = {
        URLSession(configuration: .default, delegate: self, delegateQueue: nil)
    }()
    private var pushTokenRegistered = false

    init(serverURL: String) {
        self.serverURL = serverURL
    }

    /// Register the APNs device token with the backend.
    /// Called from JS via window._registerPushToken() once the user is logged in.
    func registerPushToken(accessToken: String) {
        guard !pushTokenRegistered else { return }
        guard let token = UserDefaults.standard.string(forKey: "apns_device_token"),
              !token.isEmpty else { return }

        guard let url = URL(string: "\(serverURL)/api/push/register") else { return }
        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.setValue("Bearer \(accessToken)", forHTTPHeaderField: "Authorization")

        let deviceName = UIDevice.current.name
        let body: [String: Any] = [
            "token": token,
            "platform": "ios",
            "device_name": deviceName
        ]
        request.httpBody = try? JSONSerialization.data(withJSONObject: body)

        URLSession.shared.dataTask(with: request) { [weak self] _, response, error in
            if let error = error {
                print("Push token registration failed: \(error)")
                return
            }
            if let http = response as? HTTPURLResponse, http.statusCode == 200 {
                self?.pushTokenRegistered = true
                print("Push token registered with backend")
            }
        }.resume()
    }

    // JS sends: { id, url, method, headers, body } or { cancel: id }
    // Also handles pushToken messages for push notification registration
    func userContentController(_ controller: WKUserContentController,
                               didReceive message: WKScriptMessage) {
        // Handle push token registration
        if message.name == "pushToken" {
            if let body = message.body as? [String: String],
               let accessToken = body["accessToken"] {
                registerPushToken(accessToken: accessToken)
            }
            return
        }

        guard let body = message.body as? [String: Any] else { return }

        // Handle cancel
        if let cancelId = body["cancel"] as? String {
            lock.lock()
            if let task = activeRequests.removeValue(forKey: cancelId) {
                taskMap.removeValue(forKey: task.taskIdentifier)
                lock.unlock()
                task.cancel()
            } else {
                lock.unlock()
            }
            return
        }

        guard let id = body["id"] as? String,
              let urlString = body["url"] as? String,
              let method = body["method"] as? String else { return }

        guard let url = URL(string: urlString) else {
            callJS("window._apiError('\(id)','Invalid URL')")
            return
        }

        var request = URLRequest(url: url)
        request.httpMethod = method

        if let headers = body["headers"] as? [String: String] {
            for (k, v) in headers {
                request.setValue(v, forHTTPHeaderField: k)
            }
        }
        if let bodyStr = body["body"] as? String {
            request.httpBody = bodyStr.data(using: .utf8)
        }

        let task = session.dataTask(with: request)

        lock.lock()
        taskMap[task.taskIdentifier] = id
        activeRequests[id] = task
        lock.unlock()

        task.resume()
    }

    func callJS(_ js: String) {
        DispatchQueue.main.async { [weak self] in
            self?.webView?.evaluateJavaScript(js, completionHandler: nil)
        }
    }

    // MARK: URLSessionDataDelegate — streams data to JS incrementally
    func urlSession(_ session: URLSession, dataTask: URLSessionDataTask,
                    didReceive response: URLResponse,
                    completionHandler: @escaping (URLSession.ResponseDisposition) -> Void) {
        lock.lock()
        guard let id = taskMap[dataTask.taskIdentifier] else {
            lock.unlock()
            completionHandler(.cancel)
            return
        }
        lock.unlock()

        let http = response as? HTTPURLResponse
        let statusCode = http?.statusCode ?? 200

        var headers: [String: String] = [:]
        for (key, value) in (http?.allHeaderFields ?? [:]) {
            if let k = key as? String, let v = value as? String {
                headers[k] = v
            }
        }

        let headersJSON = (try? JSONSerialization.data(withJSONObject: headers))
            .flatMap { String(data: $0, encoding: .utf8) } ?? "{}"

        callJS("window._apiResponse('\(id)',\(statusCode),\(headersJSON))")
        completionHandler(.allow)
    }

    func urlSession(_ session: URLSession, dataTask: URLSessionDataTask, didReceive data: Data) {
        lock.lock()
        guard let id = taskMap[dataTask.taskIdentifier] else {
            lock.unlock()
            return
        }
        lock.unlock()

        let base64 = data.base64EncodedString()
        callJS("window._apiData('\(id)','\(base64)')")
    }

    func urlSession(_ session: URLSession, task: URLSessionTask,
                    didCompleteWithError error: Error?) {
        lock.lock()
        guard let id = taskMap.removeValue(forKey: task.taskIdentifier) else {
            lock.unlock()
            return
        }
        activeRequests.removeValue(forKey: id)
        lock.unlock()

        if let error = error {
            if (error as NSError).code != NSURLErrorCancelled {
                let msg = error.localizedDescription
                    .replacingOccurrences(of: "\\", with: "\\\\")
                    .replacingOccurrences(of: "'", with: "\\'")
                callJS("window._apiError('\(id)','\(msg)')")
            }
        } else {
            callJS("window._apiDone('\(id)')")
        }
    }
}

// MARK: - WebView
final class WebViewNavigationDelegate: NSObject, WKNavigationDelegate {
    let onError: (String) -> Void

    init(onError: @escaping (String) -> Void) {
        self.onError = onError
    }

    func webView(_ webView: WKWebView, didFail navigation: WKNavigation!, withError error: Error) {
        let message = "WebView navigation failed: \(error.localizedDescription)"
        print(message)
        onError(message)
    }

    func webView(_ webView: WKWebView, didFailProvisionalNavigation navigation: WKNavigation!, withError error: Error) {
        let message = "WebView provisional navigation failed: \(error.localizedDescription)"
        print(message)
        onError(message)
    }
}

struct LightfriendWebView: UIViewRepresentable {
    let serverURL: String
    @Binding var loadError: String?

    final class Coordinator {
        let apiProxy: APIProxy
        let navigationDelegate: WebViewNavigationDelegate

        init(serverURL: String, onError: @escaping (String) -> Void) {
            self.apiProxy = APIProxy(serverURL: serverURL)
            self.navigationDelegate = WebViewNavigationDelegate(onError: onError)
        }
    }

    func makeCoordinator() -> Coordinator {
        Coordinator(serverURL: serverURL) { message in
            DispatchQueue.main.async {
                self.loadError = message
            }
        }
    }

    func makeUIView(context: Context) -> WKWebView {
        let config = WKWebViewConfiguration()
        config.allowsInlineMediaPlayback = true
        config.mediaTypesRequiringUserActionForPlayback = []

        // Register JS message handlers
        config.userContentController.add(context.coordinator.apiProxy, name: "api")
        config.userContentController.add(context.coordinator.apiProxy, name: "pushToken")

        // Inject fetch override: intercepts all API calls → routes through Swift
        let escapedURL = serverURL
            .replacingOccurrences(of: "\\", with: "\\\\")
            .replacingOccurrences(of: "'", with: "\\'")
        let js = """
        (function() {
            const SERVER = '\(escapedURL)';
            window.__LIGHTFRIEND_API__ = SERVER;
            const _fetch = window.fetch;
            let _id = 0;
            window._apiCalls = {};

            window._apiResponse = function(id, status, headers) {
                var c = window._apiCalls[id];
                if (!c) return;
                c._status = status;
                c._headers = headers;
                c.resolve(new Response(c.stream, { status: status, headers: headers }));
            };
            window._apiData = function(id, b64) {
                var c = window._apiCalls[id];
                if (!c) return;
                var bin = atob(b64), arr = new Uint8Array(bin.length);
                for (var i = 0; i < bin.length; i++) arr[i] = bin.charCodeAt(i);
                try { c.ctrl.enqueue(arr); } catch(e) {}
            };
            window._apiDone = function(id) {
                var c = window._apiCalls[id];
                if (!c) return;
                try { c.ctrl.close(); } catch(e) {}
                delete window._apiCalls[id];
            };
            window._apiError = function(id, msg) {
                var c = window._apiCalls[id];
                if (!c) return;
                if (!c._resolved) c.reject(new TypeError(msg));
                try { c.ctrl.error(new Error(msg)); } catch(e) {}
                delete window._apiCalls[id];
            };

            window.fetch = function(url, opts) {
                var urlStr = typeof url === 'string' ? url : url.url;
                if (!urlStr.startsWith(SERVER)) {
                    return _fetch.call(this, url, opts);
                }
                return new Promise(function(resolve, reject) {
                    var id = String(++_id);
                    var ctrl;
                    var stream = new ReadableStream({ start: function(c) { ctrl = c; } });
                    window._apiCalls[id] = {
                        resolve: function(r) { this._resolved = true; resolve(r); },
                        reject: reject,
                        stream: stream,
                        ctrl: ctrl,
                        _resolved: false
                    };

                    var hdrs = {};
                    if (opts && opts.headers) {
                        if (opts.headers instanceof Headers) {
                            opts.headers.forEach(function(v, k) { hdrs[k] = v; });
                        } else if (typeof opts.headers === 'object') {
                            Object.keys(opts.headers).forEach(function(k) { hdrs[k] = opts.headers[k]; });
                        }
                    }

                    var bodyStr = null;
                    if (opts && opts.body) {
                        bodyStr = typeof opts.body === 'string' ? opts.body : JSON.stringify(opts.body);
                    }

                    window.webkit.messageHandlers.api.postMessage({
                        id: id,
                        url: urlStr,
                        method: (opts && opts.method) || 'GET',
                        headers: hdrs,
                        body: bodyStr
                    });

                    if (opts && opts.signal) {
                        opts.signal.addEventListener('abort', function() {
                            window.webkit.messageHandlers.api.postMessage({ cancel: id });
                            reject(new DOMException('The operation was aborted.', 'AbortError'));
                            try { ctrl.error(new DOMException('Aborted', 'AbortError')); } catch(e) {}
                            delete window._apiCalls[id];
                        });
                    }
                });
            };
        })();
        """
        let script = WKUserScript(source: js, injectionTime: .atDocumentStart,
                                  forMainFrameOnly: false)
        config.userContentController.addUserScript(script)

        let webView = WKWebView(frame: .zero, configuration: config)
        webView.navigationDelegate = context.coordinator.navigationDelegate
        webView.isOpaque = false
        webView.backgroundColor = UIColor(red: 0.067, green: 0.067, blue: 0.067, alpha: 1)
        webView.scrollView.backgroundColor = UIColor(red: 0.067, green: 0.067, blue: 0.067, alpha: 1)
        webView.scrollView.bounces = false

        // Set webView reference for JS callbacks
        context.coordinator.apiProxy.webView = webView

        // Push token registration: sends device token to backend once user is logged in
        let pushTokenJS = """
        (function() {
            function tryRegisterPushToken() {
                var token = localStorage.getItem('lf_token');
                if (token && window.webkit && window.webkit.messageHandlers.pushToken) {
                    window.webkit.messageHandlers.pushToken.postMessage({accessToken: token});
                }
            }
            // Try on load and after a delay (in case login happens asynchronously)
            setTimeout(tryRegisterPushToken, 1500);
            // Also expose for manual trigger after login
            window._registerPushToken = tryRegisterPushToken;
        })();
        """
        let pushTokenScript = WKUserScript(source: pushTokenJS, injectionTime: .atDocumentEnd,
                                           forMainFrameOnly: true)
        config.userContentController.addUserScript(pushTokenScript)

        // Load bundled HTML from file://
        if let htmlURL = Bundle.main.url(forResource: "frontend", withExtension: "html") {
            print("Loading bundled frontend from: \(htmlURL.path)")
            webView.loadFileURL(htmlURL, allowingReadAccessTo: htmlURL.deletingLastPathComponent())
        } else {
            let message = "frontend.html missing from app bundle"
            print(message)
            DispatchQueue.main.async {
                self.loadError = message
            }
        }

        return webView
    }

    func updateUIView(_ webView: WKWebView, context: Context) {}
}

struct ContentView_Previews: PreviewProvider {
    static var previews: some View {
        ContentView()
            .preferredColorScheme(.dark)
    }
}

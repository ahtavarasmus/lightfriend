import SwiftUI
import WebKit

struct ContentView: View {
    @State private var serverURL: String = UserDefaults.standard.string(forKey: "server_url") ?? ""
    @State private var isConnected = false
    @State private var showSettings = false

    var body: some View {
        if isConnected {
            ZStack {
                WebView(url: serverURL)
                    .ignoresSafeArea(.container, edges: .bottom)

                VStack {
                    Spacer()
                    HStack {
                        Spacer()
                        Button(action: { showSettings = true }) {
                            Image(systemName: "gearshape.fill")
                                .font(.system(size: 16))
                                .foregroundColor(.white.opacity(0.5))
                                .padding(10)
                                .background(Color.black.opacity(0.3))
                                .clipShape(Circle())
                        }
                        .padding(.trailing, 16)
                        .padding(.bottom, 80)
                    }
                }
            }
            .sheet(isPresented: $showSettings) {
                SettingsView(serverURL: $serverURL, isConnected: $isConnected)
            }
        } else {
            SetupView(serverURL: $serverURL, isConnected: $isConnected)
        }
    }
}

// MARK: - Setup Screen
struct SetupView: View {
    @Binding var serverURL: String
    @Binding var isConnected: Bool
    @State private var inputURL: String = ""
    @State private var isLoading = false
    @State private var errorMessage: String?

    var body: some View {
        ZStack {
            Color(red: 0.067, green: 0.067, blue: 0.067)
                .ignoresSafeArea()

            VStack(spacing: 24) {
                Spacer()

                // Logo
                Text("lightfriend")
                    .font(.system(size: 32, weight: .semibold))
                    .foregroundStyle(
                        LinearGradient(
                            colors: [.white, Color(red: 0.494, green: 0.698, blue: 1.0)],
                            startPoint: .topLeading,
                            endPoint: .bottomTrailing
                        )
                    )

                Text("Connect to your server")
                    .font(.system(size: 16))
                    .foregroundColor(.white.opacity(0.5))

                // URL Input
                VStack(alignment: .leading, spacing: 8) {
                    Text("Server URL")
                        .font(.system(size: 14))
                        .foregroundColor(.white.opacity(0.6))

                    TextField("http://your-server:3000", text: $inputURL)
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
                .padding(.horizontal, 32)

                if let error = errorMessage {
                    Text(error)
                        .font(.system(size: 13))
                        .foregroundColor(.red.opacity(0.8))
                        .padding(.horizontal, 32)
                }

                // Connect button
                Button(action: connect) {
                    HStack {
                        if isLoading {
                            ProgressView()
                                .tint(Color(red: 0.1, green: 0.1, blue: 0.1))
                                .scaleEffect(0.8)
                        }
                        Text(isLoading ? "Connecting..." : "Connect")
                            .fontWeight(.semibold)
                    }
                    .frame(maxWidth: .infinity)
                    .padding(14)
                    .background(
                        LinearGradient(
                            colors: [
                                Color(white: 0.83),
                                Color(white: 0.66),
                                Color(white: 0.91),
                                Color(white: 0.66),
                                Color(white: 0.75)
                            ],
                            startPoint: .topLeading,
                            endPoint: .bottomTrailing
                        )
                    )
                    .foregroundColor(Color(red: 0.1, green: 0.1, blue: 0.18))
                    .cornerRadius(10)
                }
                .disabled(isLoading || inputURL.isEmpty)
                .padding(.horizontal, 32)

                Spacer()
                Spacer()
            }
        }
        .onAppear {
            inputURL = serverURL
        }
    }

    func connect() {
        var url = inputURL.trimmingCharacters(in: .whitespacesAndNewlines)
        if !url.hasPrefix("http://") && !url.hasPrefix("https://") {
            url = "http://" + url
        }
        if url.hasSuffix("/") { url.removeLast() }

        isLoading = true
        errorMessage = nil

        // Test connection
        guard let testURL = URL(string: url + "/api/health") else {
            errorMessage = "Invalid URL"
            isLoading = false
            return
        }

        URLSession.shared.dataTask(with: testURL) { _, response, error in
            DispatchQueue.main.async {
                isLoading = false
                if let error = error {
                    errorMessage = "Cannot connect: \(error.localizedDescription)"
                    return
                }
                if let http = response as? HTTPURLResponse, http.statusCode == 200 {
                    serverURL = url
                    UserDefaults.standard.set(url, forKey: "server_url")
                    isConnected = true
                } else {
                    errorMessage = "Server not responding correctly"
                }
            }
        }.resume()
    }
}

// MARK: - Settings
struct SettingsView: View {
    @Binding var serverURL: String
    @Binding var isConnected: Bool
    @Environment(\.dismiss) var dismiss

    var body: some View {
        NavigationView {
            ZStack {
                Color(red: 0.067, green: 0.067, blue: 0.067)
                    .ignoresSafeArea()

                VStack(spacing: 16) {
                    VStack(alignment: .leading, spacing: 8) {
                        Text("Connected to")
                            .font(.system(size: 13))
                            .foregroundColor(.white.opacity(0.4))
                        Text(serverURL)
                            .font(.system(size: 15, design: .monospaced))
                            .foregroundColor(.white.opacity(0.7))
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(16)
                    .background(Color.white.opacity(0.04))
                    .cornerRadius(12)

                    Button(action: {
                        serverURL = ""
                        UserDefaults.standard.removeObject(forKey: "server_url")
                        isConnected = false
                        dismiss()
                    }) {
                        Text("Disconnect")
                            .frame(maxWidth: .infinity)
                            .padding(14)
                            .background(Color.red.opacity(0.12))
                            .foregroundColor(.red)
                            .cornerRadius(10)
                            .overlay(
                                RoundedRectangle(cornerRadius: 10)
                                    .stroke(Color.red.opacity(0.2), lineWidth: 1)
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
    }
}

// MARK: - WebView
struct WebView: UIViewRepresentable {
    let url: String

    func makeUIView(context: Context) -> WKWebView {
        let config = WKWebViewConfiguration()
        config.allowsInlineMediaPlayback = true
        config.mediaTypesRequiringUserActionForPlayback = []

        let webView = WKWebView(frame: .zero, configuration: config)
        webView.isOpaque = false
        webView.backgroundColor = UIColor(red: 0.067, green: 0.067, blue: 0.067, alpha: 1)
        webView.scrollView.backgroundColor = UIColor(red: 0.067, green: 0.067, blue: 0.067, alpha: 1)
        webView.navigationDelegate = context.coordinator

        if let webURL = URL(string: url) {
            webView.load(URLRequest(url: webURL))
        }

        return webView
    }

    func updateUIView(_ webView: WKWebView, context: Context) {}

    func makeCoordinator() -> Coordinator { Coordinator() }

    class Coordinator: NSObject, WKNavigationDelegate {
        func webView(_ webView: WKWebView, didFail navigation: WKNavigation!, withError error: Error) {
            print("WebView error: \(error)")
        }
    }
}

#Preview {
    ContentView()
        .preferredColorScheme(.dark)
}

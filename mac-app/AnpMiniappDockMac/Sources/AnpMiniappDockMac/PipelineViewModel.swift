import Foundation

@MainActor
final class ChatbotViewModel: ObservableObject {
    @Published var inputText = "我要点一杯咖啡"
    @Published var isRunning = false
    @Published var statusText = "ready"
    @Published var repoRootDisplay: String
    @Published var messages: [ChatMessage] = [
        ChatMessage(
            role: .assistant,
            text: "你好，我是 ANP MiniApp Dock Chatbot。输入你的需求后，我会先用 OpenAI 兼容接口做意图识别，再调用本地小程序容器和 Coffee Skill，并把 Skill 返回的组件渲染到对话里。",
            detail: "环境变量从 shell 读取：OPENAI_BASE_URL / OPENAI_API_KEY / OPENAI_MODEL。若 API Key 为空，会使用本地 fallback 意图识别，便于演示。"
        )
    ]
    @Published var errorMessage: String?

    init() {
        repoRootDisplay = RepoLocator.findRepoRoot()?.path(percentEncoded: false) ?? "未找到仓库根目录，可设置 ANP_DOCK_REPO_ROOT"
    }

    func sendCurrentMessage() {
        let text = inputText.trimmed()
        guard !text.isEmpty, !isRunning else { return }
        inputText = ""
        messages.append(ChatMessage(role: .user, text: text))
        runChatTurn(userText: text)
    }

    func runExamplePrompt() {
        inputText = "我要点一杯咖啡"
        sendCurrentMessage()
    }

    func reset() {
        guard !isRunning else { return }
        errorMessage = nil
        statusText = "ready"
        messages = [
            ChatMessage(
                role: .assistant,
                text: "对话已重置。你可以输入：我要点一杯咖啡。",
                detail: "我会执行：意图识别 → 调用小程序容器/Skill → 渲染 Skill 返回组件。"
            )
        ]
    }

    private func runChatTurn(userText: String) {
        isRunning = true
        statusText = "recognizing"
        errorMessage = nil

        Task {
            do {
                let runner = ChatbotTurnRunner()
                let result = try await runner.run(userText: userText)
                repoRootDisplay = result.repoRoot.path(percentEncoded: false)
                messages.append(contentsOf: result.messages)
                statusText = "ok"
            } catch {
                errorMessage = error.localizedDescription
                messages.append(ChatMessage(
                    role: .assistant,
                    text: "执行失败：\(error.localizedDescription)",
                    detail: "请检查 Rust workspace、localhost coffee service、以及 OpenAI 环境变量配置。"
                ))
                statusText = "failed"
            }
            isRunning = false
        }
    }
}

struct ChatbotTurnResult: Sendable {
    let repoRoot: URL
    let messages: [ChatMessage]
}

struct ChatbotTurnRunner {
    func runHeadless(userText: String) throws -> ChatbotTurnResult {
        guard let repoRoot = RepoLocator.findRepoRoot() else {
            throw DemoPipelineError.repoRootNotFound
        }
        let intent = IntentResult(
            intent: userText.lowercased().contains("咖啡") || userText.lowercased().contains("coffee") ? .coffeeOrder : .unknown,
            apiName: "searchDrinks",
            arguments: ["query": "latte"],
            confidence: 0.72,
            userFacingSummary: "Headless smoke 使用本地 fallback 意图识别。",
            source: .fallback
        )
        var output = [ChatMessage(
            role: .assistant,
            text: "意图识别完成：\(intent.intent.displayName)",
            detail: "来源：\(intent.source.displayName)；置信度：\(Int(intent.confidence * 100))%。\(intent.userFacingSummary)"
        )]
        if intent.intent == .coffeeOrder {
            let run = try DemoPipelineRunner().run()
            output.append(ChatMessage(
                role: .assistant,
                text: "已调用本地小程序容器并执行 Coffee Skill。下面是 Skill 返回的组件渲染结果。",
                detail: "Skill: examples/coffee-skill；服务：\(run.snapshot.serverBaseURL)。",
                snapshot: run.snapshot,
                logLines: run.logLines
            ))
            return ChatbotTurnResult(repoRoot: run.repoRoot, messages: output)
        }
        return ChatbotTurnResult(repoRoot: repoRoot, messages: output)
    }

    func run(userText: String) async throws -> ChatbotTurnResult {
        guard let repoRoot = RepoLocator.findRepoRoot() else {
            throw DemoPipelineError.repoRootNotFound
        }

        let recognizer = IntentRecognizer()
        let intent = try await recognizer.recognize(userText: userText)
        var output = [ChatMessage(
            role: .assistant,
            text: "意图识别完成：\(intent.intent.displayName)",
            detail: "来源：\(intent.source.displayName)；置信度：\(Int(intent.confidence * 100))%。\(intent.userFacingSummary)"
        )]

        switch intent.intent {
        case .coffeeOrder:
            let run = try await DemoPipelineRunner().runAsync()
            output.append(ChatMessage(
                role: .assistant,
                text: "已调用本地小程序容器并执行 Coffee Skill。下面是 Skill 返回的组件渲染结果。",
                detail: "Skill: examples/coffee-skill；服务：\(run.snapshot.serverBaseURL)；流程：searchDrinks → confirmOrder → payOrder → expire。",
                snapshot: run.snapshot,
                logLines: run.logLines
            ))
            return ChatbotTurnResult(repoRoot: run.repoRoot, messages: output)
        case .unknown:
            output.append(ChatMessage(
                role: .assistant,
                text: "当前 Demo 只接入了咖啡点单 Skill。你可以试试：我要点一杯咖啡。",
                detail: "没有调用小程序容器。"
            ))
            return ChatbotTurnResult(repoRoot: repoRoot, messages: output)
        }
    }
}

enum ChatRole: String, Sendable {
    case user
    case assistant
}

struct ChatMessage: Identifiable, Sendable {
    let id = UUID()
    let role: ChatRole
    let text: String
    let detail: String?
    let snapshot: PipelineSnapshot?
    let logLines: [String]

    init(role: ChatRole, text: String, detail: String? = nil, snapshot: PipelineSnapshot? = nil, logLines: [String] = []) {
        self.role = role
        self.text = text
        self.detail = detail
        self.snapshot = snapshot
        self.logLines = logLines
    }
}

enum IntentKind: String, Codable, Sendable {
    case coffeeOrder = "coffee_order"
    case unknown

    var displayName: String {
        switch self {
        case .coffeeOrder: return "咖啡点单"
        case .unknown: return "未知/暂不支持"
        }
    }
}

enum IntentSource: String, Sendable {
    case openAI
    case fallback

    var displayName: String {
        switch self {
        case .openAI: return "OpenAI-compatible API"
        case .fallback: return "本地 fallback"
        }
    }
}

struct IntentResult: Sendable {
    let intent: IntentKind
    let apiName: String
    let arguments: [String: String]
    let confidence: Double
    let userFacingSummary: String
    let source: IntentSource
}

struct IntentRecognizer {
    func recognize(userText: String) async throws -> IntentResult {
        let config = ShellEnvironmentLoader.loadOpenAIConfig()
        if config.isUsable {
            do {
                return try await OpenAIIntentClient(config: config).recognize(userText: userText)
            } catch {
                return fallback(userText: userText, reason: "OpenAI 调用失败，已使用本地 fallback：\(error.localizedDescription)")
            }
        }
        return fallback(userText: userText, reason: "OPENAI_API_KEY 为空或未配置，已使用本地 fallback。")
    }

    private func fallback(userText: String, reason: String) -> IntentResult {
        let normalized = userText.lowercased()
        let coffeeKeywords = ["咖啡", "拿铁", "latte", "美式", "americano", "mocha", "摩卡", "coffee", "点一杯"]
        if coffeeKeywords.contains(where: { normalized.contains($0) }) {
            return IntentResult(
                intent: .coffeeOrder,
                apiName: "searchDrinks",
                arguments: ["query": normalized.contains("美式") || normalized.contains("americano") ? "americano" : "latte"],
                confidence: 0.72,
                userFacingSummary: reason,
                source: .fallback
            )
        }
        return IntentResult(
            intent: .unknown,
            apiName: "",
            arguments: [:],
            confidence: 0.4,
            userFacingSummary: reason,
            source: .fallback
        )
    }
}

struct OpenAIConfig: Sendable {
    let baseURL: String
    let apiKey: String
    let model: String

    var isUsable: Bool {
        !baseURL.trimmed().isEmpty && !apiKey.trimmed().isEmpty && !model.trimmed().isEmpty
    }
}

enum ShellEnvironmentLoader {
    static func loadOpenAIConfig() -> OpenAIConfig {
        var environment = ProcessInfo.processInfo.environment
        if environment["ANP_DOCK_DISABLE_OPENAI"] == "1" {
            return OpenAIConfig(
                baseURL: environment["OPENAI_BASE_URL"] ?? "https://api.openai.com",
                apiKey: "",
                model: environment["OPENAI_MODEL"] ?? "gpt-4.1-mini"
            )
        }
        if let shell = sourceZshrcEnvironment() {
            for (key, value) in shell where key.hasPrefix("OPENAI_") {
                environment[key] = value
            }
        }
        return OpenAIConfig(
            baseURL: environment["OPENAI_BASE_URL"] ?? "https://api.openai.com",
            apiKey: environment["OPENAI_API_KEY"] ?? "",
            model: environment["OPENAI_MODEL"] ?? "gpt-4.1-mini"
        )
    }

    private static func sourceZshrcEnvironment() -> [String: String]? {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/bin/zsh")
        process.arguments = ["-lc", "source ~/.zshrc >/dev/null 2>&1; env"]
        let pipe = Pipe()
        process.standardOutput = pipe
        process.standardError = Pipe()
        do {
            try process.run()
            process.waitUntilExit()
        } catch {
            return nil
        }
        guard process.terminationStatus == 0 else { return nil }
        let data = pipe.fileHandleForReading.readDataToEndOfFile()
        let text = String(data: data, encoding: .utf8) ?? ""
        var env: [String: String] = [:]
        for line in text.split(separator: "\n", omittingEmptySubsequences: true) {
            guard let separator = line.firstIndex(of: "=") else { continue }
            let key = String(line[..<separator])
            let value = String(line[line.index(after: separator)...])
            env[key] = value
        }
        return env
    }
}

struct OpenAIIntentClient {
    let config: OpenAIConfig

    func recognize(userText: String) async throws -> IntentResult {
        let requestBody: [String: Any] = [
            "model": config.model,
            "messages": [
                [
                    "role": "system",
                    "content": """
                    You are an intent router for a desktop chatbot that can call MiniApp Skills.
                    Return JSON only, no markdown. Schema:
                    {"intent":"coffee_order|unknown","apiName":"searchDrinks|","arguments":{"query":"latte"},"confidence":0.0,"userFacingSummary":"Chinese summary"}
                    If the user wants coffee, drinks, latte, americano, mocha, or ordering a cup, use coffee_order and apiName searchDrinks.
                    Otherwise use unknown. Keep userFacingSummary short Chinese.
                    """
                ],
                ["role": "user", "content": userText]
            ],
            "temperature": 0
        ]
        let requestData = try JSONSerialization.data(withJSONObject: requestBody)
        var request = URLRequest(url: chatCompletionsURL())
        request.httpMethod = "POST"
        request.setValue("Bearer \(config.apiKey)", forHTTPHeaderField: "Authorization")
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.timeoutInterval = 20
        request.httpBody = requestData

        let (data, response) = try await URLSession.shared.data(for: request)
        guard let http = response as? HTTPURLResponse else {
            throw OpenAIIntentError.invalidResponse
        }
        guard (200..<300).contains(http.statusCode) else {
            let body = String(data: data, encoding: .utf8)?.redactedForDisplay().prefixText(300) ?? ""
            throw OpenAIIntentError.http(status: http.statusCode, body: body)
        }
        let content = try Self.extractMessageContent(from: data)
        let jsonText = Self.extractJSONObject(from: content)
        guard let jsonData = jsonText.data(using: .utf8),
              let object = try JSONSerialization.jsonObject(with: jsonData) as? [String: Any]
        else {
            throw OpenAIIntentError.invalidJSON(content.prefixText(300))
        }

        let intent = IntentKind(rawValue: object.string("intent", default: "unknown")) ?? .unknown
        let arguments = object.dictionary("arguments").reduce(into: [String: String]()) { partial, item in
            partial[item.key] = JSONObject.describe(item.value)
        }
        return IntentResult(
            intent: intent,
            apiName: object.string("apiName", default: intent == .coffeeOrder ? "searchDrinks" : ""),
            arguments: arguments,
            confidence: object["confidence"] as? Double ?? (object["confidence"] as? NSNumber)?.doubleValue ?? 0.6,
            userFacingSummary: object.string("userFacingSummary", default: "已完成意图识别。"),
            source: .openAI
        )
    }

    private func chatCompletionsURL() -> URL {
        let trimmed = config.baseURL.trimmed().trimmingCharacters(in: CharacterSet(charactersIn: "/"))
        let path = trimmed.hasSuffix("/v1") ? "/chat/completions" : "/v1/chat/completions"
        return URL(string: trimmed + path) ?? URL(string: "https://api.openai.com/v1/chat/completions")!
    }

    private static func extractMessageContent(from data: Data) throws -> String {
        guard let object = try JSONSerialization.jsonObject(with: data) as? [String: Any],
              let choices = object["choices"] as? [[String: Any]],
              let first = choices.first,
              let message = first["message"] as? [String: Any],
              let content = message["content"] as? String
        else {
            throw OpenAIIntentError.invalidResponse
        }
        return content
    }

    private static func extractJSONObject(from text: String) -> String {
        var cleaned = text.trimmed()
        if cleaned.hasPrefix("```") {
            cleaned = cleaned.replacingOccurrences(of: "```json", with: "")
                .replacingOccurrences(of: "```", with: "")
                .trimmed()
        }
        guard let start = cleaned.firstIndex(of: "{"), let end = cleaned.lastIndex(of: "}") else {
            return cleaned
        }
        return String(cleaned[start...end])
    }
}

enum OpenAIIntentError: LocalizedError {
    case invalidResponse
    case invalidJSON(String)
    case http(status: Int, body: String)

    var errorDescription: String? {
        switch self {
        case .invalidResponse:
            return "OpenAI-compatible API returned an unexpected response shape."
        case let .invalidJSON(content):
            return "Intent JSON could not be parsed: \(content)"
        case let .http(status, body):
            return "OpenAI-compatible API returned HTTP \(status): \(body)"
        }
    }
}

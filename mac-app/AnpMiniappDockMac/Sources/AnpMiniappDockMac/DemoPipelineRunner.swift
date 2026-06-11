import Foundation

struct PipelineRunResult: Sendable {
    let repoRoot: URL
    let snapshot: PipelineSnapshot
    let logLines: [String]
}

struct DemoPipelineRunner {
    func startInteractiveSessionAsync() async throws -> CoffeeInteractiveRuntime {
        try await withCheckedThrowingContinuation { continuation in
            DispatchQueue.global(qos: .userInitiated).async {
                do {
                    continuation.resume(returning: try startInteractiveSession())
                } catch {
                    continuation.resume(throwing: error)
                }
            }
        }
    }

    func startInteractiveSession() throws -> CoffeeInteractiveRuntime {
        guard let repoRoot = RepoLocator.findRepoRoot() else {
            throw DemoPipelineError.repoRootNotFound
        }

        var logLines = [
            "repo root: \(repoRoot.path(percentEncoded: false))",
            "loading Skill with dock-cli validate..."
        ]
        let validate = try ProcessRunner.runCargo(
            ["run", "-q", "-p", "dock-cli", "--", "validate", "examples/coffee-skill"],
            currentDirectory: repoRoot,
            timeout: 180
        )
        logLines.append("validate: ok")

        let server = try startDemoServer(repoRoot: repoRoot)
        logLines.append("localhost coffee service: \(server.url)")
        logLines.append("auth provider: \(server.authProvider)")
        logLines.append("interactive mode: waiting for user card actions")

        if !validate.stderr.trimmed().isEmpty {
            logLines.append("validate stderr: \(validate.stderr.trimmed().redactedForDisplay().prefixText(220))")
        }

        return CoffeeInteractiveRuntime(
            repoRoot: repoRoot,
            server: server,
            validateOutput: validate.stdout,
            logLines: logLines
        )
    }

    func runAsync() async throws -> PipelineRunResult {
        try await withCheckedThrowingContinuation { continuation in
            DispatchQueue.global(qos: .userInitiated).async {
                do {
                    continuation.resume(returning: try run())
                } catch {
                    continuation.resume(throwing: error)
                }
            }
        }
    }

    func run() throws -> PipelineRunResult {
        guard let repoRoot = RepoLocator.findRepoRoot() else {
            throw DemoPipelineError.repoRootNotFound
        }

        var logLines = [
            "repo root: \(repoRoot.path(percentEncoded: false))",
            "loading Skill with dock-cli validate..."
        ]

        let validate = try ProcessRunner.runCargo(
            ["run", "-q", "-p", "dock-cli", "--", "validate", "examples/coffee-skill"],
            currentDirectory: repoRoot,
            timeout: 180
        )
        logLines.append("validate: ok")

        var server = try startDemoServer(repoRoot: repoRoot)
        defer { server.stop() }
        logLines.append("localhost coffee service: \(server.url)")
        logLines.append("auth provider: \(server.authProvider)")
        logLines.append("running dock-cli run-demo through the local container...")

        let demo: CommandResult
        do {
            demo = try runDemoCommand(serverURL: server.url, repoRoot: repoRoot)
        } catch {
            guard server.authProvider == "fastapi-anp" else {
                throw error
            }
            logLines.append("FastAPI auth run failed, falling back to Rust demo-server: \(error.localizedDescription.redactedForDisplay().prefixText(220))")
            server.stop()
            server = try startRustDemoServer(repoRoot: repoRoot)
            logLines.append("localhost coffee service: \(server.url)")
            logLines.append("auth provider: \(server.authProvider)")
            demo = try runDemoCommand(serverURL: server.url, repoRoot: repoRoot)
        }
        logLines.append("run-demo: ok")

        if !validate.stderr.trimmed().isEmpty {
            logLines.append("validate stderr: \(validate.stderr.trimmed().redactedForDisplay().prefixText(220))")
        }
        if !demo.stderr.trimmed().isEmpty {
            logLines.append("run-demo stderr: \(demo.stderr.trimmed().redactedForDisplay().prefixText(220))")
        }

        let snapshot = try PipelineSnapshot.parse(
            validateOutput: validate.stdout,
            demoOutput: demo.stdout,
            fallbackAuthProvider: server.authProvider
        )
        return PipelineRunResult(repoRoot: repoRoot, snapshot: snapshot, logLines: logLines)
    }

    private func runDemoCommand(serverURL: String, repoRoot: URL) throws -> CommandResult {
        try ProcessRunner.runCargo(
            [
                "run", "-q", "-p", "dock-cli", "--", "run-demo",
                "--skill", "examples/coffee-skill",
                "--server", serverURL
            ],
            currentDirectory: repoRoot,
            timeout: 240
        )
    }

    private func startDemoServer(repoRoot: URL) throws -> RunningDemoServer {
        if let fastAPI = try? startFastAPIServer(repoRoot: repoRoot) {
            return fastAPI
        }
        return try startRustDemoServer(repoRoot: repoRoot)
    }

    private func startFastAPIServer(repoRoot: URL) throws -> RunningDemoServer {
        let serverDir = repoRoot.appendingPathComponent("examples/coffee-fastapi-server")
        let uvicorn = serverDir.appendingPathComponent(".venv/bin/uvicorn")
        guard FileManager.default.isExecutableFile(atPath: uvicorn.path(percentEncoded: false)) else {
            throw DemoPipelineError.fastAPINotInstalled
        }

        let process = Process()
        process.executableURL = uvicorn
        process.arguments = ["app:app", "--host", "127.0.0.1", "--port", "8008"]
        process.currentDirectoryURL = serverDir
        process.environment = try coffeeAuthEnvironment(repoRoot: repoRoot)

        return try waitForServerURL(
            process: process,
            authProvider: "fastapi-anp",
            timeout: 60,
            timeoutMessage: "FastAPI coffee service did not print its listening URL"
        )
    }

    private func startRustDemoServer(repoRoot: URL) throws -> RunningDemoServer {
        let process = Process()
        let authEnvironment = try coffeeAuthEnvironment(repoRoot: repoRoot)
        let identity = try defaultIdentity(repoRoot: repoRoot)
        let trustedDIDDocumentPath = authEnvironment["ANP_COFFEE_TRUSTED_DID_DOCUMENT"]
            ?? identity.didDocumentURL.path(percentEncoded: false)
        let trustedDIDDocument = trustedDIDDocumentPath.contains("=")
            ? trustedDIDDocumentPath
            : "\(identity.did)=\(trustedDIDDocumentPath)"
        let tokenIssuerSecret = authEnvironment["ANP_COFFEE_TOKEN_ISSUER_SECRET"]
            ?? "test-only-local-secret"
        let merchantDID = authEnvironment["ANP_COFFEE_MERCHANT_DID"]
            ?? "did:wba:coffee-merchant.example"
        ProcessRunner.configureCargoProcess(
            process,
            arguments: [
                "run", "-q", "-p", "demo-server", "--",
                "--host", "127.0.0.1",
                "--port", "0",
                "--skill", "examples/coffee-skill",
                "--merchant-did", merchantDID,
                "--token-issuer-secret", tokenIssuerSecret,
                "--trusted-did-document", trustedDIDDocument
            ],
            currentDirectory: repoRoot
        )

        return try waitForServerURL(
            process: process,
            authProvider: "rust-demo-server",
            timeout: 180,
            timeoutMessage: "demo-server did not print its listening URL"
        )
    }

    private func defaultIdentityDID(repoRoot: URL) throws -> String {
        try defaultIdentity(repoRoot: repoRoot).did
    }

    private func defaultIdentity(repoRoot: URL) throws -> DemoIdentity {
        let documentURL = repoRoot.appendingPathComponent("examples/identity/did_document.json")
        let data = try Data(contentsOf: documentURL)
        guard let object = try JSONSerialization.jsonObject(with: data) as? [String: Any],
              let did = object["id"] as? String,
              !did.trimmed().isEmpty
        else {
            throw DemoPipelineError.identityDocumentInvalid
        }
        return DemoIdentity(did: did, didDocumentURL: documentURL)
    }

    private func coffeeAuthEnvironment(repoRoot: URL) throws -> [String: String] {
        var environment = ProcessInfo.processInfo.environment
        let identity = try defaultIdentity(repoRoot: repoRoot)
        environment["ANP_COFFEE_MERCHANT_DID"] = environment["ANP_COFFEE_MERCHANT_DID"] ?? "did:wba:coffee-merchant.example"
        environment["ANP_COFFEE_PUBLIC_BASE_URL"] = environment["ANP_COFFEE_PUBLIC_BASE_URL"] ?? "http://127.0.0.1:8008"
        environment["ANP_COFFEE_TRUSTED_DID_DOCUMENT"] = environment["ANP_COFFEE_TRUSTED_DID_DOCUMENT"]
            ?? identity.didDocumentURL.path(percentEncoded: false)
        environment["ANP_COFFEE_TOKEN_ISSUER_SECRET"] = environment["ANP_COFFEE_TOKEN_ISSUER_SECRET"] ?? "test-only-local-secret"
        environment["ANP_COFFEE_AUTH_MODE"] = environment["ANP_COFFEE_AUTH_MODE"] ?? "anp-http-signature/v1"
        return environment
    }

    private func waitForServerURL(process: Process, authProvider: String, timeout: TimeInterval, timeoutMessage: String) throws -> RunningDemoServer {
        let stdoutPipe = Pipe()
        let stderrPipe = Pipe()
        let stdout = LockedTextBuffer()
        let stderr = LockedTextBuffer()
        process.standardOutput = stdoutPipe
        process.standardError = stderrPipe
        stdoutPipe.fileHandleForReading.readabilityHandler = { handle in
            stdout.append(handle.availableData)
        }
        stderrPipe.fileHandleForReading.readabilityHandler = { handle in
            stderr.append(handle.availableData)
        }

        try process.run()

        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline {
            let output = stdout.string() + "\n" + stderr.string()
            if let url = Self.firstServerURL(in: output) {
                return RunningDemoServer(
                    process: process,
                    url: url,
                    authProvider: authProvider,
                    stdoutPipe: stdoutPipe,
                    stderrPipe: stderrPipe,
                    stdout: stdout,
                    stderr: stderr
                )
            }

            if !process.isRunning {
                stdoutPipe.fileHandleForReading.readabilityHandler = nil
                stderrPipe.fileHandleForReading.readabilityHandler = nil
                throw DemoPipelineError.serverExited(
                    stdout: stdout.string().redactedForDisplay(),
                    stderr: stderr.string().redactedForDisplay()
                )
            }
            Thread.sleep(forTimeInterval: 0.1)
        }

        process.terminate()
        throw DemoPipelineError.timeout(timeoutMessage)
    }

    private static func firstServerURL(in output: String) -> String? {
        guard let range = output.range(of: #"http://[^\s]+"#, options: .regularExpression) else {
            return nil
        }
        return String(output[range])
    }
}

final class CoffeeInteractiveRuntime: @unchecked Sendable {
    let repoRoot: URL
    let server: RunningDemoServer
    let validateOutput: String
    private(set) var logLines: [String]
    private var isStopped = false

    var serverURL: String {
        server.url
    }

    var authProvider: String {
        server.authProvider
    }

    init(repoRoot: URL, server: RunningDemoServer, validateOutput: String, logLines: [String]) {
        self.repoRoot = repoRoot
        self.server = server
        self.validateOutput = validateOutput
        self.logLines = logLines
    }

    deinit {
        stop()
    }

    func stop() {
        guard !isStopped else { return }
        isStopped = true
        server.stop()
    }

    func searchDrinks(query: String) async throws -> CoffeeDrinkListCard {
        try await runOnWorker {
            try self.searchDrinksSync(query: query)
        }
    }

    func confirmOrder(drinkId: String) async throws -> CoffeeOrderCard {
        try await runOnWorker {
            try self.confirmOrderSync(drinkId: drinkId)
        }
    }

    func payOrder(orderId: String) async throws -> CoffeePaymentCard {
        try await runOnWorker {
            try self.payOrderSync(orderId: orderId)
        }
    }

    func searchDrinksSync(query: String) throws -> CoffeeDrinkListCard {
        let output = try callAPI(
            name: "searchDrinks",
            arguments: [
                "query": query,
                "serverUrl": server.url
            ]
        )
        return try Self.parseDrinkList(output)
    }

    func confirmOrderSync(drinkId: String) throws -> CoffeeOrderCard {
        let output = try callAPI(
            name: "confirmOrder",
            arguments: [
                "drinkId": drinkId,
                "remoteBaseUrl": server.url
            ]
        )
        return try Self.parseOrder(output)
    }

    func payOrderSync(orderId: String) throws -> CoffeePaymentCard {
        let output = try callAPI(
            name: "payOrder",
            arguments: [
                "orderId": orderId,
                "remoteBaseUrl": server.url
            ]
        )
        return try Self.parsePayment(output)
    }

    private func runOnWorker<T>(_ work: @escaping () throws -> T) async throws -> T {
        try await withCheckedThrowingContinuation { continuation in
            DispatchQueue.global(qos: .userInitiated).async {
                do {
                    continuation.resume(returning: try work())
                } catch {
                    continuation.resume(throwing: error)
                }
            }
        }
    }

    private func callAPI(name: String, arguments: [String: Any]) throws -> [String: Any] {
        let data = try JSONSerialization.data(withJSONObject: arguments, options: [.sortedKeys])
        let jsonArgs = String(data: data, encoding: .utf8) ?? "{}"
        logLines.append("calling Skill API interactively: \(name)")
        let result = try ProcessRunner.runCargo(
            [
                "run", "-q", "-p", "dock-cli", "--", "call-api",
                "examples/coffee-skill",
                name,
                jsonArgs
            ],
            currentDirectory: repoRoot,
            timeout: 240
        )
        if !result.stderr.trimmed().isEmpty {
            logLines.append("\(name) stderr: \(result.stderr.trimmed().redactedForDisplay().prefixText(220))")
        }
        let output = try JSONObject.parse(result.stdout, label: "dock-cli call-api \(name)")
        if output.string("status", default: "unknown") != "ok" {
            throw DemoPipelineError.commandFailed("dock-cli call-api \(name) returned non-ok status")
        }
        return output
    }

    private static func parseDrinkList(_ output: [String: Any]) throws -> CoffeeDrinkListCard {
        let result = output.dictionary("result")
        let structured = result.dictionary("structuredContent")
        let drinks = structured.array("drinks").compactMap { item -> CoffeeDrink? in
            guard let drink = item as? [String: Any] else { return nil }
            let id = drink.string("id", default: "")
            guard !id.trimmed().isEmpty else { return nil }
            return CoffeeDrink(
                id: id,
                name: drink.string("name", default: id),
                price: drink["price"] as? Int ?? (drink["price"] as? NSNumber)?.intValue ?? 0,
                image: drink.string("image", default: "")
            )
        }
        return CoffeeDrinkListCard(
            drinks: drinks,
            contentText: contentText(result, fallback: "请选择一杯咖啡。"),
            componentPath: componentPath(output, fallback: "components/drink-list/index"),
            authSummary: authSummary(result)
        )
    }

    private static func parseOrder(_ output: [String: Any]) throws -> CoffeeOrderCard {
        let result = output.dictionary("result")
        let structured = result.dictionary("structuredContent")
        return CoffeeOrderCard(
            orderId: structured.string("orderId", default: "unknown"),
            drinkId: structured.string("drinkId", default: "unknown"),
            payable: structured["payable"] as? Int ?? (structured["payable"] as? NSNumber)?.intValue ?? 0,
            contentText: contentText(result, fallback: "请确认订单。"),
            componentPath: componentPath(output, fallback: "components/order-confirm/index"),
            authSummary: authSummary(result)
        )
    }

    private static func parsePayment(_ output: [String: Any]) throws -> CoffeePaymentCard {
        let result = output.dictionary("result")
        let structured = result.dictionary("structuredContent")
        return CoffeePaymentCard(
            orderId: structured.string("orderId", default: "unknown"),
            status: structured.string("status", default: "unknown"),
            contentText: contentText(result, fallback: "支付完成。"),
            componentPath: componentPath(output, fallback: "components/payment-result/index"),
            authSummary: authSummary(result)
        )
    }

    private static func contentText(_ result: [String: Any], fallback: String) -> String {
        result.array("content")
            .compactMap { ($0 as? [String: Any])?.string("text", default: "") }
            .first { !$0.trimmed().isEmpty } ?? fallback
    }

    private static func componentPath(_ output: [String: Any], fallback: String) -> String {
        output.dictionary("render").string("componentPath", default: fallback)
    }

    private static func authSummary(_ result: [String: Any]) -> String {
        let meta = result.dictionary("_meta")
        let boundary = meta.string("authBoundary", default: "container-managed")
        let mode = meta.string("requestAuthMode", default: "host-managed-bearer")
        return "\(boundary), \(mode), tokenVisible=\(meta.string("tokenVisibleToSkill", default: "false"))"
    }
}

final class RunningDemoServer {
    let process: Process
    let url: String
    let authProvider: String
    private let stdoutPipe: Pipe
    private let stderrPipe: Pipe
    private let stdout: LockedTextBuffer
    private let stderr: LockedTextBuffer

    init(process: Process, url: String, authProvider: String, stdoutPipe: Pipe, stderrPipe: Pipe, stdout: LockedTextBuffer, stderr: LockedTextBuffer) {
        self.process = process
        self.url = url
        self.authProvider = authProvider
        self.stdoutPipe = stdoutPipe
        self.stderrPipe = stderrPipe
        self.stdout = stdout
        self.stderr = stderr
    }

    func stop() {
        stdoutPipe.fileHandleForReading.readabilityHandler = nil
        stderrPipe.fileHandleForReading.readabilityHandler = nil
        if process.isRunning {
            process.terminate()
            process.waitUntilExit()
        }
    }

    deinit {
        stop()
    }
}

private struct DemoIdentity {
    let did: String
    let didDocumentURL: URL
}

enum DemoPipelineError: LocalizedError {
    case repoRootNotFound
    case fastAPINotInstalled
    case identityDocumentInvalid
    case serverExited(stdout: String, stderr: String)
    case timeout(String)
    case commandFailed(String)

    var errorDescription: String? {
        switch self {
        case .repoRootNotFound:
            return "Could not find anp-miniapp-dock repo root. Set ANP_DOCK_REPO_ROOT to the repository path."
        case .fastAPINotInstalled:
            return "FastAPI venv was not found at examples/coffee-fastapi-server/.venv/bin/uvicorn."
        case .identityDocumentInvalid:
            return "Default DID document examples/identity/did_document.json is missing or does not contain an id."
        case let .serverExited(stdout, stderr):
            return "localhost coffee service exited before becoming ready. stdout=\(stdout.prefixText(300)) stderr=\(stderr.prefixText(300))"
        case let .timeout(message):
            return message
        case let .commandFailed(message):
            return message
        }
    }
}

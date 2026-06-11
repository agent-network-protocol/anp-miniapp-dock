import Foundation

struct PipelineRunResult: Sendable {
    let repoRoot: URL
    let snapshot: PipelineSnapshot
    let logLines: [String]
}

struct DemoPipelineRunner {
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
        }
    }
}

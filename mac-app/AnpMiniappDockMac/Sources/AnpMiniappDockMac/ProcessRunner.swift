import Foundation

struct CommandResult: Sendable {
    let exitCode: Int32
    let stdout: String
    let stderr: String
}

enum ProcessRunnerError: LocalizedError {
    case cargoNotFound
    case commandFailed(command: String, exitCode: Int32, stdout: String, stderr: String)
    case timeout(command: String, seconds: TimeInterval, stdout: String, stderr: String)

    var errorDescription: String? {
        switch self {
        case .cargoNotFound:
            return "cargo was not found. Install Rust or add ~/.cargo/bin to PATH."
        case let .commandFailed(command, exitCode, stdout, stderr):
            return "command failed (\(exitCode)): \(command)\nstdout: \(stdout.prefixText(500))\nstderr: \(stderr.prefixText(500))"
        case let .timeout(command, seconds, stdout, stderr):
            return "command timed out after \(Int(seconds))s: \(command)\nstdout: \(stdout.prefixText(500))\nstderr: \(stderr.prefixText(500))"
        }
    }
}

final class ProcessRunner {
    static func runCargo(_ arguments: [String], currentDirectory: URL, timeout: TimeInterval) throws -> CommandResult {
        let process = Process()
        configureCargoProcess(process, arguments: arguments, currentDirectory: currentDirectory)
        let displayCommand = ("cargo " + arguments.joined(separator: " ")).redactedForDisplay()
        return try run(process, timeout: timeout, displayCommand: displayCommand)
    }

    static func configureCargoProcess(_ process: Process, arguments: [String], currentDirectory: URL) {
        if let cargoURL = findCargoExecutable() {
            process.executableURL = cargoURL
            process.arguments = arguments
        } else {
            process.executableURL = URL(fileURLWithPath: "/usr/bin/env")
            process.arguments = ["cargo"] + arguments
        }
        process.currentDirectoryURL = currentDirectory
        process.environment = environmentWithDeveloperPaths()
    }

    private static func run(_ process: Process, timeout: TimeInterval, displayCommand: String) throws -> CommandResult {
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

        let semaphore = DispatchSemaphore(value: 0)
        process.terminationHandler = { _ in semaphore.signal() }

        try process.run()
        if semaphore.wait(timeout: .now() + timeout) == .timedOut {
            process.terminate()
            stdoutPipe.fileHandleForReading.readabilityHandler = nil
            stderrPipe.fileHandleForReading.readabilityHandler = nil
            throw ProcessRunnerError.timeout(
                command: displayCommand,
                seconds: timeout,
                stdout: stdout.string().redactedForDisplay(),
                stderr: stderr.string().redactedForDisplay()
            )
        }

        stdoutPipe.fileHandleForReading.readabilityHandler = nil
        stderrPipe.fileHandleForReading.readabilityHandler = nil
        stdout.append(stdoutPipe.fileHandleForReading.readDataToEndOfFile())
        stderr.append(stderrPipe.fileHandleForReading.readDataToEndOfFile())

        let result = CommandResult(
            exitCode: process.terminationStatus,
            stdout: stdout.string(),
            stderr: stderr.string()
        )
        guard result.exitCode == 0 else {
            throw ProcessRunnerError.commandFailed(
                command: displayCommand,
                exitCode: result.exitCode,
                stdout: result.stdout.redactedForDisplay(),
                stderr: result.stderr.redactedForDisplay()
            )
        }
        return result
    }

    private static func findCargoExecutable() -> URL? {
        let fm = FileManager.default
        let home = URL(fileURLWithPath: NSHomeDirectory())
        let candidates = [
            home.appendingPathComponent(".cargo/bin/cargo"),
            URL(fileURLWithPath: "/opt/homebrew/bin/cargo"),
            URL(fileURLWithPath: "/usr/local/bin/cargo")
        ]
        return candidates.first { fm.isExecutableFile(atPath: $0.path(percentEncoded: false)) }
    }

    private static func environmentWithDeveloperPaths() -> [String: String] {
        var environment = ProcessInfo.processInfo.environment
        let homeCargo = URL(fileURLWithPath: NSHomeDirectory()).appendingPathComponent(".cargo/bin").path(percentEncoded: false)
        let additions = [homeCargo, "/opt/homebrew/bin", "/usr/local/bin", "/usr/bin", "/bin"]
        let existingPath = environment["PATH"] ?? ""
        environment["PATH"] = (additions + [existingPath]).filter { !$0.isEmpty }.joined(separator: ":")
        environment["RUST_BACKTRACE"] = environment["RUST_BACKTRACE"] ?? "1"
        return environment
    }
}

final class LockedTextBuffer {
    private let lock = NSLock()
    private var data = Data()

    func append(_ chunk: Data) {
        guard !chunk.isEmpty else { return }
        lock.lock()
        data.append(chunk)
        lock.unlock()
    }

    func string() -> String {
        lock.lock()
        let copy = data
        lock.unlock()
        return String(data: copy, encoding: .utf8) ?? String(decoding: copy, as: UTF8.self)
    }
}

import Foundation

enum RepoLocator {
    static func findRepoRoot() -> URL? {
        let fm = FileManager.default
        if let configured = ProcessInfo.processInfo.environment["ANP_DOCK_REPO_ROOT"], !configured.isEmpty {
            let url = URL(fileURLWithPath: configured).standardizedFileURL
            if isRepoRoot(url, fileManager: fm) { return url }
        }

        var starts = [URL(fileURLWithPath: fm.currentDirectoryPath).standardizedFileURL]
        starts.append(Bundle.main.bundleURL.standardizedFileURL)
        if let executable = Bundle.main.executableURL?.standardizedFileURL {
            starts.append(executable.deletingLastPathComponent())
        }

        for start in starts {
            if let found = walkUp(from: start, fileManager: fm) {
                return found
            }
        }
        return nil
    }

    private static func walkUp(from start: URL, fileManager: FileManager) -> URL? {
        var current = start
        var isDirectory: ObjCBool = false
        if fileManager.fileExists(atPath: current.path(percentEncoded: false), isDirectory: &isDirectory), !isDirectory.boolValue {
            current.deleteLastPathComponent()
        }

        for _ in 0..<14 {
            if isRepoRoot(current, fileManager: fileManager) {
                return current.standardizedFileURL
            }
            let parent = current.deletingLastPathComponent()
            if parent.path == current.path { break }
            current = parent
        }
        return nil
    }

    private static func isRepoRoot(_ url: URL, fileManager: FileManager) -> Bool {
        fileManager.fileExists(atPath: url.appendingPathComponent("Cargo.toml").path(percentEncoded: false)) &&
        fileManager.fileExists(atPath: url.appendingPathComponent("examples/coffee-skill/mcp.json").path(percentEncoded: false)) &&
        fileManager.fileExists(atPath: url.appendingPathComponent("crates/dock-cli/Cargo.toml").path(percentEncoded: false))
    }
}

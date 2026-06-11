import Foundation
import SwiftUI

@main
struct AnpMiniappDockMacApp: App {
    init() {
        if ProcessInfo.processInfo.environment["ANP_DOCK_MAC_HEADLESS"] == "1" {
            runHeadlessSmoke()
        }
    }

    var body: some Scene {
        WindowGroup {
            ContentView()
                .frame(minWidth: 980, minHeight: 720)
        }
        .windowStyle(.titleBar)
    }

    private func runHeadlessSmoke() -> Never {
        let prompt = ProcessInfo.processInfo.environment["ANP_DOCK_CHAT_PROMPT"] ?? "我要点一杯咖啡"
        do {
            let result = try ChatbotTurnRunner().runHeadless(userText: prompt)
            let snapshot = result.messages.compactMap(\.snapshot).last
            let summary: [String: Any] = [
                "status": snapshot?.status ?? "unknown",
                "repoRoot": result.repoRoot.path(percentEncoded: false),
                "messages": result.messages.map(\.text),
                "components": snapshot?.components ?? [],
                "serverBaseUrl": snapshot?.serverBaseURL ?? "",
                "auth": snapshot?.authEvidence.jsonValue ?? [:],
                "steps": snapshot?.steps.map(\.name) ?? [],
                "paymentStatus": snapshot?.paymentStatus ?? "",
                "auditCount": snapshot?.auditCount ?? 0
            ]
            let data = try JSONSerialization.data(withJSONObject: summary, options: [.prettyPrinted, .sortedKeys])
            FileHandle.standardOutput.write(data)
            FileHandle.standardOutput.write(Data("\n".utf8))
            exit(0)
        } catch {
            FileHandle.standardError.write(Data("headless chat smoke failed: \(error.localizedDescription)\n".utf8))
            exit(1)
        }
    }
}

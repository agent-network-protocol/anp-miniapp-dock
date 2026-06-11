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
            if ProcessInfo.processInfo.environment["ANP_DOCK_MAC_HEADLESS_INTERACTIVE"] == "1" {
                try runHeadlessInteractiveSmoke(prompt: prompt)
            }
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

    private func runHeadlessInteractiveSmoke(prompt: String) throws -> Never {
        let runtime = try DemoPipelineRunner().startInteractiveSession()
        defer { runtime.stop() }

        let drinks = try runtime.searchDrinksSync(query: prompt.lowercased().contains("美式") ? "americano" : "latte")
        guard let firstDrink = drinks.drinks.first else {
            throw DemoPipelineError.commandFailed("interactive drink-list card has no selectable drinks")
        }
        let order = try runtime.confirmOrderSync(drinkId: firstDrink.id)
        let payment = try runtime.payOrderSync(orderId: order.orderId)
        let summary: [String: Any] = [
            "status": "ok",
            "repoRoot": runtime.repoRoot.path(percentEncoded: false),
            "serverBaseUrl": runtime.serverURL,
            "authProvider": runtime.authProvider,
            "cards": [
                [
                    "type": "drink-list",
                    "componentPath": drinks.componentPath,
                    "buttonCount": drinks.drinks.count,
                    "buttons": drinks.drinks.map { "选择 \($0.name)" }
                ],
                [
                    "type": "order-confirm",
                    "componentPath": order.componentPath,
                    "buttonCount": 1,
                    "buttons": ["支付 ¥\(order.payable)"],
                    "orderId": order.orderId
                ],
                [
                    "type": "payment-result",
                    "componentPath": payment.componentPath,
                    "buttonCount": 0,
                    "orderId": payment.orderId,
                    "status": payment.status
                ]
            ],
            "paymentStatus": payment.status
        ]
        let data = try JSONSerialization.data(withJSONObject: summary, options: [.prettyPrinted, .sortedKeys])
        FileHandle.standardOutput.write(data)
        FileHandle.standardOutput.write(Data("\n".utf8))
        runtime.stop()
        exit(0)
    }
}

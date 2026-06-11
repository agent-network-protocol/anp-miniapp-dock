import Foundation

struct PipelineSnapshot: Identifiable, Sendable {
    let id = UUID()
    let status: String
    let loadedApis: [String]
    let components: [String]
    let serverBaseURL: String
    let authEvidence: PipelineAuthEvidence
    let merchantDID: String
    let tokenReceived: Bool
    let firstDrinkID: String
    let orderID: String
    let paymentStatus: String
    let auditCount: Int
    let componentActions: [String: String]
    let steps: [PipelineStep]
    let rawJSON: String

    static func parse(validateOutput: String, demoOutput: String, fallbackAuthProvider: String? = nil) throws -> PipelineSnapshot {
        let validate = try JSONObject.parse(validateOutput, label: "dock-cli validate")
        let demo = try JSONObject.parse(demoOutput, label: "dock-cli run-demo")

        let server = demo.dictionary("server")
        let auth = demo.dictionary("auth").merging(server.dictionary("auth")) { _, serverValue in serverValue }
        let business = server.dictionary("business")
        let audit = demo.array("audit").compactMap { $0 as? [String: Any] }
        let flow = demo.array("flow").compactMap { item -> PipelineStep? in
            guard let object = item as? [String: Any] else { return nil }
            return PipelineStep(object: object)
        }
        let authEvidence = PipelineAuthEvidence.parse(
            server: server,
            auth: auth,
            business: business,
            audit: audit,
            fallbackAuthProvider: fallbackAuthProvider
        )

        return PipelineSnapshot(
            status: demo.string("status", default: "unknown"),
            loadedApis: validate.stringArray("apis"),
            components: validate.stringArray("components"),
            serverBaseURL: server.string("baseUrl", default: "unknown"),
            authEvidence: authEvidence,
            merchantDID: authEvidence.merchantDid,
            tokenReceived: authEvidence.tokenReceived,
            firstDrinkID: business.string("firstDrinkId", default: "unknown"),
            orderID: business.string("orderId", default: "unknown"),
            paymentStatus: business.string("paymentStatus", default: "unknown"),
            auditCount: demo.array("audit").count,
            componentActions: Self.parseComponentActions(demo.dictionary("componentActions")),
            steps: flow,
            rawJSON: demoOutput.trimmed().redactedForDisplay()
        )
    }

    private static func parseComponentActions(_ object: [String: Any]) -> [String: String] {
        var actions: [String: String] = [:]
        for (key, value) in object {
            guard let action = value as? [String: Any] else { continue }
            actions[key] = action.string("name", default: action.string("type", default: "action"))
        }
        return actions
    }
}

struct PipelineAuthEvidence: Sendable {
    let authProvider: String
    let userDid: String
    let agentDid: String
    let merchantDid: String
    let challengeVerified: Bool?
    let tokenScopes: [String]
    let wxLoginStatus: String
    let requestAuthMode: String
    let tokenReceived: Bool

    static func parse(
        server: [String: Any],
        auth: [String: Any],
        business: [String: Any],
        audit: [[String: Any]],
        fallbackAuthProvider: String?
    ) -> PipelineAuthEvidence {
        let credential = auth.dictionary("credential")
        let firstAudit = audit.first ?? [:]
        let tokenReceived = auth.bool("tokenReceived") || auth.bool("token")
        let provider = firstNonEmpty([
            auth.string("authProvider", default: ""),
            auth.string("provider", default: ""),
            server.string("authProvider", default: ""),
            server.dictionary("health").string("authProvider", default: ""),
            providerFromHealth(server.dictionary("health")),
            fallbackAuthProvider ?? ""
        ], default: "not reported")

        return PipelineAuthEvidence(
            authProvider: provider,
            userDid: firstNonEmpty([
                auth.string("userDid", default: ""),
                credential.string("userDid", default: ""),
                firstAudit.string("userDid", default: "")
            ]),
            agentDid: firstNonEmpty([
                auth.string("agentDid", default: ""),
                credential.string("agentDid", default: ""),
                firstAudit.string("agentDid", default: "")
            ]),
            merchantDid: firstNonEmpty([
                auth.string("merchantDid", default: ""),
                credential.string("merchantDid", default: ""),
                firstAudit.string("merchantDid", default: "")
            ]),
            challengeVerified: firstBool(auth, keys: ["challengeVerified", "didChallengeVerified", "proofVerified"]),
            tokenScopes: tokenScopes(from: auth, business: business),
            wxLoginStatus: firstNonEmpty([
                auth.string("wxLoginStatus", default: ""),
                auth.string("loginStatus", default: ""),
                auth.dictionary("wxLogin").string("status", default: ""),
                auth.dictionary("didAuth").string("status", default: ""),
                auth.string("errMsg", default: "")
            ], default: tokenReceived ? "login:ok" : "not reported"),
            requestAuthMode: firstNonEmpty([
                auth.string("requestAuthMode", default: ""),
                auth.string("requestAuth", default: ""),
                auth.string("authMode", default: "")
            ], default: tokenReceived ? "host-managed-bearer" : "not reported"),
            tokenReceived: tokenReceived
        )
    }

    var challengeVerifiedDisplay: String {
        switch challengeVerified {
        case .some(true): return "verified"
        case .some(false): return "failed"
        case .none: return "not reported"
        }
    }

    var tokenDisplay: String {
        tokenReceived ? "[REDACTED] received" : "not received"
    }

    var scopesDisplay: String {
        tokenScopes.isEmpty ? "not reported" : tokenScopes.joined(separator: ", ")
    }

    var overviewStatus: String {
        if challengeVerified == true && tokenReceived {
            return "DID verified + token received"
        }
        if tokenReceived {
            return "token received; challenge not reported"
        }
        return "auth evidence missing"
    }

    var jsonValue: [String: Any] {
        [
            "authProvider": authProvider,
            "userDid": userDid,
            "agentDid": agentDid,
            "merchantDid": merchantDid,
            "challengeVerified": challengeVerified.map { $0 as Any } ?? NSNull(),
            "tokenReceived": tokenReceived,
            "tokenScopes": tokenScopes,
            "scopes": tokenScopes,
            "wxLoginStatus": wxLoginStatus,
            "requestAuthMode": requestAuthMode
        ]
    }

    private static func firstNonEmpty(_ values: [String], default fallback: String = "unknown") -> String {
        values.map { $0.trimmed() }.first { !$0.isEmpty } ?? fallback
    }

    private static func firstBool(_ object: [String: Any], keys: [String]) -> Bool? {
        for key in keys {
            if let bool = object.boolOptional(key) {
                return bool
            }
        }
        return nil
    }

    private static func providerFromHealth(_ health: [String: Any]) -> String {
        switch health.string("service", default: "").trimmed() {
        case "coffee-fastapi-server":
            return "fastapi-anp"
        case "demo-server":
            return "rust-demo-server"
        case let service where !service.isEmpty:
            return service
        default:
            return ""
        }
    }

    private static func tokenScopes(from auth: [String: Any], business: [String: Any]) -> [String] {
        for key in ["tokenScopes", "scopes"] {
            let array = auth.stringArray(key)
            if !array.isEmpty {
                return array
            }
            let split = splitScopes(auth.string(key, default: ""))
            if !split.isEmpty {
                return split
            }
        }

        let tokenObject = auth.dictionary("capabilityToken")
        for key in ["tokenScopes", "scopes"] {
            let array = tokenObject.stringArray(key)
            if !array.isEmpty {
                return array
            }
        }

        var derived: [String] = []
        if business.string("firstDrinkId", default: "").trimmed().isEmpty == false {
            derived.append("coffee:drinks:read")
        }
        if business.string("orderId", default: "").trimmed().isEmpty == false {
            derived.append("coffee:order:confirm")
        }
        if business.string("paymentStatus", default: "").trimmed().isEmpty == false {
            derived.append("coffee:order:pay")
        }
        return derived
    }

    private static func splitScopes(_ text: String) -> [String] {
        text
            .split { character in
                character == "," || character == " " || character == "\n" || character == "\t"
            }
            .map { String($0).trimmed() }
            .filter { !$0.isEmpty }
    }
}

struct PipelineStep: Identifiable, Sendable {
    let id = UUID()
    let name: String
    let renderRootKind: String
    let contentTexts: [String]
    let details: [String: String]

    init(object: [String: Any]) {
        name = object.string("name", default: "step")
        renderRootKind = object.string("renderRootKind", default: object.string("renderRootId", default: "runtime"))
        contentTexts = object.array("content").compactMap { item in
            (item as? [String: Any])?.string("text", default: "")
        }.filter { !$0.isEmpty }

        if name == "expire" {
            details = [
                "state": JSONObject.describe(object["state"]),
                "actions": JSONObject.describe(object["actions"]),
                "trace": JSONObject.describe(object["trace"])
            ]
        } else {
            let structured = object.dictionary("structuredContent")
            details = Self.details(for: name, structured: structured, renderRootKind: renderRootKind, actions: object["actions"])
        }
    }

    private static func details(for name: String, structured: [String: Any], renderRootKind: String, actions: Any?) -> [String: String] {
        var output: [String: String] = ["render": renderRootKind]
        switch name {
        case "searchDrinks":
            let drinks = structured.array("drinks").compactMap { item -> String? in
                guard let drink = item as? [String: Any] else { return nil }
                let name = drink.string("name", default: drink.string("id", default: "drink"))
                let price = JSONObject.describe(drink["price"])
                return "\(name) ¥\(price)"
            }
            output["drinks"] = drinks.joined(separator: ", ")
        case "confirmOrder":
            output["orderId"] = structured.string("orderId", default: "unknown")
            output["drink"] = structured.string("drinkId", default: "unknown")
            output["payable"] = "¥\(JSONObject.describe(structured["payable"]))"
        case "payOrder":
            output["orderId"] = structured.string("orderId", default: "unknown")
            output["status"] = structured.string("status", default: "unknown")
        default:
            output["structured"] = JSONObject.describe(structured)
        }
        output["actions"] = JSONObject.describe(actions)
        return output
    }
}

struct CoffeeDrink: Identifiable, Sendable, Equatable {
    let id: String
    let name: String
    let price: Int
    let image: String
}

struct CoffeeDrinkListCard: Sendable, Equatable {
    let drinks: [CoffeeDrink]
    let contentText: String
    let componentPath: String
    let authSummary: String
}

struct CoffeeOrderCard: Sendable, Equatable {
    let orderId: String
    let drinkId: String
    let payable: Int
    let contentText: String
    let componentPath: String
    let authSummary: String
}

struct CoffeePaymentCard: Sendable, Equatable {
    let orderId: String
    let status: String
    let contentText: String
    let componentPath: String
    let authSummary: String
}

enum CoffeeInteractiveCard: Sendable, Equatable {
    case drinkList(CoffeeDrinkListCard)
    case orderConfirm(CoffeeOrderCard)
    case paymentResult(CoffeePaymentCard)
}

enum SnapshotParseError: LocalizedError {
    case invalidJSON(label: String)

    var errorDescription: String? {
        switch self {
        case let .invalidJSON(label):
            return "Could not parse JSON output from \(label)."
        }
    }
}

enum JSONObject {
    static func parse(_ text: String, label: String) throws -> [String: Any] {
        guard let data = text.data(using: .utf8),
              let object = try JSONSerialization.jsonObject(with: data) as? [String: Any]
        else {
            throw SnapshotParseError.invalidJSON(label: label)
        }
        return object
    }

    static func describe(_ value: Any?) -> String {
        guard let value else { return "-" }
        if let string = value as? String { return string }
        if let number = value as? NSNumber { return number.stringValue }
        if let array = value as? [Any], array.isEmpty { return "[]" }
        if let dictionary = value as? [String: Any], dictionary.isEmpty { return "{}" }
        if JSONSerialization.isValidJSONObject(value),
           let data = try? JSONSerialization.data(withJSONObject: value, options: [.sortedKeys]),
           let text = String(data: data, encoding: .utf8) {
            return text
        }
        return String(describing: value)
    }
}

extension Dictionary where Key == String, Value == Any {
    func dictionary(_ key: String) -> [String: Any] {
        self[key] as? [String: Any] ?? [:]
    }

    func array(_ key: String) -> [Any] {
        self[key] as? [Any] ?? []
    }

    func stringArray(_ key: String) -> [String] {
        array(key).compactMap { $0 as? String }
    }

    func string(_ key: String, default fallback: String) -> String {
        if let string = self[key] as? String { return string }
        if let number = self[key] as? NSNumber { return number.stringValue }
        return fallback
    }

    func bool(_ key: String) -> Bool {
        if let bool = self[key] as? Bool { return bool }
        if let number = self[key] as? NSNumber { return number.boolValue }
        if let string = self[key] as? String {
            switch string.lowercased() {
            case "true", "yes", "1", "ok", "verified":
                return true
            default:
                return false
            }
        }
        return false
    }

    func boolOptional(_ key: String) -> Bool? {
        if let bool = self[key] as? Bool { return bool }
        if let number = self[key] as? NSNumber { return number.boolValue }
        if let string = self[key] as? String {
            switch string.lowercased() {
            case "true", "yes", "1", "ok", "verified":
                return true
            case "false", "no", "0", "failed", "denied":
                return false
            default:
                return nil
            }
        }
        return nil
    }
}

extension String {
    func trimmed() -> String {
        trimmingCharacters(in: .whitespacesAndNewlines)
    }

    func prefixText(_ maxLength: Int) -> String {
        if count <= maxLength { return self }
        return String(prefix(maxLength)) + "…"
    }

    func redactedForDisplay() -> String {
        var text = self
        let patterns = [
            #"(?i)(--token-issuer-secret\s+)[^\s\"]+"#,
            #"(?i)(authorization:\s*bearer\s+)[^\s\"]+"#,
            #"(?i)(\"authorization\"\s*:\s*\"bearer\s+)[^\"]+"#,
            #"(?i)(signature-input\"\s*:\s*\")[^\"]+"#,
            #"(?i)(signature\"\s*:\s*\")[^\"]+"#,
            #"(?i)(content-digest\"\s*:\s*\")[^\"]+"#,
            #"(?i)(signedChallenge\"\s*:\s*)\{[^\n]*\}"#,
            #"(?i)(proof\"\s*:\s*)\{[^\n]*\}"#,
            #"(?i)(accessToken\"\s*:\s*\")[^\"]+"#,
            #"(?i)(capabilityToken\"\s*:\s*\")[^\"]+"#,
            #"(?i)(token\"\s*:\s*\")[^\"]+"#,
            #"(?i)(secret\"\s*:\s*\")[^\"]+"#,
            #"(?i)(privateKey\"\s*:\s*\")[^\"]+"#,
            #"(?i)(privateKeyPath\"\s*:\s*\")[^\"]+"#
        ]
        for pattern in patterns {
            text = text.replacingOccurrences(of: pattern, with: "$1[REDACTED]", options: .regularExpression)
        }
        return text
    }
}

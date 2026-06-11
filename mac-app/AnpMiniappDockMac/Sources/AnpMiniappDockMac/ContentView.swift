import SwiftUI

struct ContentView: View {
    @StateObject private var viewModel = ChatbotViewModel()
    @FocusState private var inputFocused: Bool

    var body: some View {
        NavigationStack {
            VStack(spacing: 0) {
                header
                    .padding(.horizontal, 24)
                    .padding(.top, 20)
                    .padding(.bottom, 14)

                Divider()

                chatTranscript

                Divider()

                composer
                    .padding(18)
            }
            .navigationTitle("ANP MiniApp Chatbot Demo")
            .toolbar {
                ToolbarItemGroup(placement: .primaryAction) {
                    Button {
                        viewModel.runExamplePrompt()
                    } label: {
                        Label("示例需求", systemImage: "sparkles")
                    }
                    .disabled(viewModel.isRunning)

                    Button {
                        viewModel.reset()
                    } label: {
                        Label("清空", systemImage: "trash")
                    }
                    .disabled(viewModel.isRunning)
                }
            }
        }
    }

    private var header: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(alignment: .top) {
                VStack(alignment: .leading, spacing: 6) {
                    Text("Chatbot + 小程序容器")
                        .font(.largeTitle.bold())
                    Text("输入自然语言需求，系统使用 OpenAI 兼容 API 做意图识别，然后调用本地 MiniApp MCP 容器和 Coffee Skill，把 Skill 返回的组件渲染在对话中。")
                        .foregroundStyle(.secondary)
                }
                Spacer()
                StatusBadge(text: viewModel.statusText, isRunning: viewModel.isRunning, hasError: viewModel.errorMessage != nil)
            }

            HStack(spacing: 10) {
                Label(viewModel.repoRootDisplay, systemImage: "folder")
                    .font(.callout)
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
                    .truncationMode(.middle)
                Spacer()
                if let errorMessage = viewModel.errorMessage {
                    Label(errorMessage, systemImage: "exclamationmark.triangle.fill")
                        .foregroundStyle(.red)
                        .font(.callout)
                        .lineLimit(2)
                }
            }
        }
    }

    private var chatTranscript: some View {
        ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(spacing: 16) {
                    ForEach(viewModel.messages) { message in
                        ChatMessageBubble(
                            message: message,
                            isRunning: viewModel.isRunning,
                            onSelectDrink: { drink in viewModel.selectDrink(drink) },
                            onPayOrder: { order in viewModel.payOrder(order) }
                        )
                            .id(message.id)
                    }
                    if viewModel.isRunning {
                        TypingBubble()
                            .id("typing")
                    }
                }
                .padding(24)
            }
            .background(Color(nsColor: .textBackgroundColor).opacity(0.35))
            .onChange(of: viewModel.messages.count) { _ in
                scrollToBottom(proxy)
            }
            .onChange(of: viewModel.isRunning) { _ in
                scrollToBottom(proxy)
            }
        }
    }

    private var composer: some View {
        HStack(alignment: .bottom, spacing: 12) {
            VStack(alignment: .leading, spacing: 6) {
                Text("输入需求")
                    .font(.caption.weight(.semibold))
                    .foregroundStyle(.secondary)
                TextField("例如：我要点一杯咖啡", text: $viewModel.inputText, axis: .vertical)
                    .textFieldStyle(.plain)
                    .lineLimit(1...4)
                    .padding(12)
                    .background(.background, in: RoundedRectangle(cornerRadius: 14, style: .continuous))
                    .overlay(
                        RoundedRectangle(cornerRadius: 14, style: .continuous)
                            .stroke(.quaternary, lineWidth: 1)
                    )
                    .focused($inputFocused)
                    .onSubmit { viewModel.sendCurrentMessage() }
            }
            Button {
                viewModel.sendCurrentMessage()
                inputFocused = true
            } label: {
                if viewModel.isRunning {
                    ProgressView()
                        .controlSize(.small)
                        .frame(width: 72)
                } else {
                    Label("发送", systemImage: "paperplane.fill")
                        .frame(width: 72)
                }
            }
            .buttonStyle(.borderedProminent)
            .disabled(viewModel.isRunning || viewModel.inputText.trimmed().isEmpty)
        }
    }

    private func scrollToBottom(_ proxy: ScrollViewProxy) {
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) {
            withAnimation(.easeOut(duration: 0.25)) {
                if viewModel.isRunning {
                    proxy.scrollTo("typing", anchor: .bottom)
                } else if let last = viewModel.messages.last {
                    proxy.scrollTo(last.id, anchor: .bottom)
                }
            }
        }
    }
}

private struct ChatMessageBubble: View {
    let message: ChatMessage
    let isRunning: Bool
    let onSelectDrink: (CoffeeDrink) -> Void
    let onPayOrder: (CoffeeOrderCard) -> Void

    var body: some View {
        HStack(alignment: .top) {
            if message.role == .user { Spacer(minLength: 80) }

            VStack(alignment: message.role == .user ? .trailing : .leading, spacing: 10) {
                HStack(spacing: 8) {
                    if message.role == .assistant {
                        Image(systemName: "sparkles")
                            .foregroundStyle(.blue)
                    }
                    Text(message.role == .user ? "你" : "MiniApp Agent")
                        .font(.caption.weight(.semibold))
                        .foregroundStyle(.secondary)
                    if message.role == .user {
                        Image(systemName: "person.crop.circle.fill")
                            .foregroundStyle(.secondary)
                    }
                }

                VStack(alignment: .leading, spacing: 10) {
                    Text(message.text)
                        .font(.body)
                        .textSelection(.enabled)
                    if let detail = message.detail, !detail.isEmpty {
                        Text(detail)
                            .font(.callout)
                            .foregroundStyle(.secondary)
                            .textSelection(.enabled)
                    }
                    if let snapshot = message.snapshot {
                        SnapshotAttachmentView(snapshot: snapshot, logLines: message.logLines)
                    }
                    if let coffeeCard = message.coffeeCard {
                        CoffeeInteractiveCardView(
                            card: coffeeCard,
                            isRunning: isRunning,
                            onSelectDrink: onSelectDrink,
                            onPayOrder: onPayOrder
                        )
                    }
                }
                .padding(14)
                .frame(maxWidth: message.role == .user ? 520 : 860, alignment: .leading)
                .background(background, in: RoundedRectangle(cornerRadius: 18, style: .continuous))
                .overlay(
                    RoundedRectangle(cornerRadius: 18, style: .continuous)
                        .stroke(borderColor, lineWidth: 1)
                )
            }

            if message.role == .assistant { Spacer(minLength: 60) }
        }
    }

    private var background: Color {
        message.role == .user ? Color.accentColor.opacity(0.14) : Color(nsColor: .controlBackgroundColor)
    }

    private var borderColor: Color {
        message.role == .user ? Color.accentColor.opacity(0.24) : Color.secondary.opacity(0.12)
    }
}

private struct CoffeeInteractiveCardView: View {
    let card: CoffeeInteractiveCard
    let isRunning: Bool
    let onSelectDrink: (CoffeeDrink) -> Void
    let onPayOrder: (CoffeeOrderCard) -> Void

    var body: some View {
        switch card {
        case let .drinkList(model):
            DrinkListInteractiveCard(model: model, isRunning: isRunning, onSelectDrink: onSelectDrink)
        case let .orderConfirm(model):
            OrderConfirmInteractiveCard(model: model, isRunning: isRunning, onPayOrder: onPayOrder)
        case let .paymentResult(model):
            PaymentResultInteractiveCard(model: model)
        }
    }
}

private struct DrinkListInteractiveCard: View {
    let model: CoffeeDrinkListCard
    let isRunning: Bool
    let onSelectDrink: (CoffeeDrink) -> Void

    var body: some View {
        CardContainer {
            VStack(alignment: .leading, spacing: 14) {
                CardHeader(
                    icon: "cup.and.saucer.fill",
                    tint: .brown,
                    title: "选择咖啡",
                    subtitle: "MiniApp component: \(model.componentPath)"
                )
                Text(model.contentText)
                    .font(.callout)
                    .foregroundStyle(.secondary)

                VStack(spacing: 10) {
                    ForEach(model.drinks) { drink in
                        HStack(spacing: 12) {
                            VStack(alignment: .leading, spacing: 3) {
                                Text(drink.name)
                                    .font(.headline)
                                Text("drinkId=\(drink.id)")
                                    .font(.caption)
                                    .foregroundStyle(.secondary)
                            }
                            Spacer()
                            Text("¥\(drink.price)")
                                .font(.headline)
                            Button {
                                onSelectDrink(drink)
                            } label: {
                                Label("选择", systemImage: "hand.tap.fill")
                            }
                            .buttonStyle(.borderedProminent)
                            .disabled(isRunning)
                        }
                        .padding(12)
                        .background(.secondary.opacity(0.06), in: RoundedRectangle(cornerRadius: 12, style: .continuous))
                    }
                }

                FlowHint(text: "点击饮品 = 触发组件 confirmDrink 事件 → api/call confirmOrder")
                AuthBoundaryLabel(text: model.authSummary)
            }
        }
    }
}

private struct OrderConfirmInteractiveCard: View {
    let model: CoffeeOrderCard
    let isRunning: Bool
    let onPayOrder: (CoffeeOrderCard) -> Void

    var body: some View {
        CardContainer {
            VStack(alignment: .leading, spacing: 14) {
                CardHeader(
                    icon: "checklist.checked",
                    tint: .orange,
                    title: "确认订单",
                    subtitle: "MiniApp component: \(model.componentPath)"
                )
                Text(model.contentText)
                    .font(.callout)
                    .foregroundStyle(.secondary)

                VStack(alignment: .leading, spacing: 8) {
                    EvidenceRow(label: "订单", value: model.orderId)
                    EvidenceRow(label: "饮品", value: model.drinkId)
                    EvidenceRow(label: "应付", value: "¥\(model.payable)")
                }

                Button {
                    onPayOrder(model)
                } label: {
                    Label("支付 ¥\(model.payable)", systemImage: "creditcard.fill")
                        .frame(maxWidth: .infinity)
                }
                .buttonStyle(.borderedProminent)
                .controlSize(.large)
                .disabled(isRunning)

                FlowHint(text: "点击支付 = 触发组件 payOrder 事件 → api/call payOrder")
                AuthBoundaryLabel(text: model.authSummary)
            }
        }
    }
}

private struct PaymentResultInteractiveCard: View {
    let model: CoffeePaymentCard

    var body: some View {
        CardContainer {
            VStack(alignment: .leading, spacing: 14) {
                CardHeader(
                    icon: "checkmark.circle.fill",
                    tint: .green,
                    title: "支付结果",
                    subtitle: "MiniApp component: \(model.componentPath)"
                )
                Text(model.contentText)
                    .font(.callout)
                    .foregroundStyle(.secondary)
                VStack(alignment: .leading, spacing: 8) {
                    EvidenceRow(label: "订单", value: model.orderId)
                    EvidenceRow(label: "状态", value: model.status)
                }
                FlowHint(text: "支付完成后，容器会触发 expirePreviousCards，使上一张确认订单卡片过期。")
                AuthBoundaryLabel(text: model.authSummary)
            }
        }
    }
}

private struct CardHeader: View {
    let icon: String
    let tint: Color
    let title: String
    let subtitle: String

    var body: some View {
        HStack(spacing: 12) {
            ZStack {
                Circle()
                    .fill(tint.opacity(0.16))
                    .frame(width: 48, height: 48)
                Image(systemName: icon)
                    .foregroundStyle(tint)
                    .font(.title3)
            }
            VStack(alignment: .leading, spacing: 3) {
                Text(title)
                    .font(.title3.bold())
                Text(subtitle)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
                    .truncationMode(.middle)
            }
            Spacer()
        }
    }
}

private struct FlowHint: View {
    let text: String

    var body: some View {
        Label(text, systemImage: "arrow.triangle.branch")
            .font(.caption)
            .foregroundStyle(.secondary)
    }
}

private struct AuthBoundaryLabel: View {
    let text: String

    var body: some View {
        Label(text, systemImage: "lock.shield")
            .font(.caption)
            .foregroundStyle(.secondary)
            .lineLimit(2)
    }
}

private struct TypingBubble: View {
    var body: some View {
        HStack {
            VStack(alignment: .leading, spacing: 8) {
                Text("MiniApp Agent")
                    .font(.caption.weight(.semibold))
                    .foregroundStyle(.secondary)
                HStack(spacing: 10) {
                    ProgressView()
                        .controlSize(.small)
                    Text("正在识别意图并调用小程序容器…")
                        .font(.callout)
                        .foregroundStyle(.secondary)
                }
                .padding(14)
                .background(Color(nsColor: .controlBackgroundColor), in: RoundedRectangle(cornerRadius: 18, style: .continuous))
            }
            Spacer()
        }
    }
}

private struct SnapshotAttachmentView: View {
    let snapshot: PipelineSnapshot
    let logLines: [String]
    @State private var showRawJSON = false
    @State private var showLogs = false

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            PipelineOverviewCard(snapshot: snapshot)
            DIDAuthEvidenceCard(evidence: snapshot.authEvidence)
            FlowCardsView(steps: snapshot.steps)
            IntegrationEvidenceCard(snapshot: snapshot)
            DisclosureGroup("运行日志", isExpanded: $showLogs) {
                LogCard(logLines: logLines)
                    .padding(.top, 8)
            }
            DisclosureGroup("容器 JSON", isExpanded: $showRawJSON) {
                ScrollView(.horizontal) {
                    Text(snapshot.rawJSON)
                        .font(.system(.caption, design: .monospaced))
                        .textSelection(.enabled)
                        .padding(.top, 8)
                }
            }
            .font(.callout)
        }
        .padding(.top, 4)
    }
}

private struct StatusBadge: View {
    let text: String
    let isRunning: Bool
    let hasError: Bool

    var body: some View {
        HStack(spacing: 8) {
            Circle()
                .fill(color)
                .frame(width: 10, height: 10)
            Text(text)
                .font(.callout.weight(.semibold))
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 8)
        .background(color.opacity(0.12), in: Capsule())
    }

    private var color: Color {
        if hasError { return .red }
        if isRunning { return .orange }
        return .green
    }
}

private struct PipelineOverviewCard: View {
    let snapshot: PipelineSnapshot

    var body: some View {
        CardContainer {
            VStack(alignment: .leading, spacing: 14) {
                HStack {
                    Label("容器加载成功", systemImage: "shippingbox.fill")
                        .font(.headline)
                    Spacer()
                    Text(snapshot.status.uppercased())
                        .font(.caption.bold())
                        .foregroundStyle(.green)
                        .padding(.horizontal, 8)
                        .padding(.vertical, 4)
                        .background(.green.opacity(0.12), in: Capsule())
                }

                LazyVGrid(columns: [GridItem(.adaptive(minimum: 190), spacing: 12)], alignment: .leading, spacing: 12) {
                    MetricTile(title: "Skill APIs", value: snapshot.loadedApis.joined(separator: ", "))
                    MetricTile(title: "Components", value: "\(snapshot.components.count) loaded")
                    MetricTile(title: "Coffee Service", value: snapshot.serverBaseURL)
                    MetricTile(title: "DID/Auth", value: snapshot.authEvidence.overviewStatus)
                }
            }
        }
    }
}

private struct FlowCardsView: View {
    let steps: [PipelineStep]

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("Skill 返回组件")
                .font(.title2.bold())
            LazyVGrid(columns: [GridItem(.adaptive(minimum: 260), spacing: 14)], alignment: .leading, spacing: 14) {
                ForEach(steps) { step in
                    MiniAppStepCard(step: step)
                }
            }
        }
    }
}

private struct MiniAppStepCard: View {
    let step: PipelineStep

    var body: some View {
        CardContainer {
            VStack(alignment: .leading, spacing: 12) {
                HStack(alignment: .center, spacing: 10) {
                    ZStack {
                        Circle()
                            .fill(step.tint.opacity(0.16))
                            .frame(width: 42, height: 42)
                        Image(systemName: step.iconName)
                            .foregroundStyle(step.tint)
                    }
                    VStack(alignment: .leading, spacing: 3) {
                        Text(step.title)
                            .font(.headline)
                        Text(step.subtitle)
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                    Spacer()
                }

                if !step.contentTexts.isEmpty {
                    ForEach(step.contentTexts, id: \.self) { text in
                        Text(text)
                            .font(.callout)
                            .foregroundStyle(.secondary)
                    }
                }

                VStack(alignment: .leading, spacing: 8) {
                    ForEach(step.details.keys.sorted(), id: \.self) { key in
                        HStack(alignment: .top) {
                            Text(key)
                                .font(.caption.weight(.semibold))
                                .foregroundStyle(.secondary)
                                .frame(width: 88, alignment: .leading)
                            Text(step.details[key] ?? "")
                                .font(.callout)
                            Spacer(minLength: 0)
                        }
                    }
                }
            }
        }
    }
}

private struct DIDAuthEvidenceCard: View {
    let evidence: PipelineAuthEvidence

    var body: some View {
        CardContainer {
            VStack(alignment: .leading, spacing: 12) {
                HStack {
                    Label("DID/Auth Evidence", systemImage: "person.badge.shield.checkmark.fill")
                        .font(.headline)
                    Spacer()
                    Text(evidence.tokenReceived ? "TOKEN RECEIVED" : "NO TOKEN")
                        .font(.caption.bold())
                        .foregroundStyle(evidence.tokenReceived ? .green : .orange)
                        .padding(.horizontal, 8)
                        .padding(.vertical, 4)
                        .background((evidence.tokenReceived ? Color.green : Color.orange).opacity(0.12), in: Capsule())
                }

                LazyVGrid(columns: [GridItem(.adaptive(minimum: 310), spacing: 12)], alignment: .leading, spacing: 10) {
                    EvidenceRow(label: "Provider", value: evidence.authProvider)
                    EvidenceRow(label: "User DID", value: evidence.userDid)
                    EvidenceRow(label: "Agent DID", value: evidence.agentDid)
                    EvidenceRow(label: "Merchant DID", value: evidence.merchantDid)
                    EvidenceRow(label: "Challenge", value: evidence.challengeVerifiedDisplay)
                    EvidenceRow(label: "wx.login", value: evidence.wxLoginStatus)
                    EvidenceRow(label: "Request Auth", value: evidence.requestAuthMode)
                    EvidenceRow(label: "Token", value: evidence.tokenDisplay)
                    EvidenceRow(label: "Scopes", value: evidence.scopesDisplay)
                }
            }
        }
    }
}

private struct IntegrationEvidenceCard: View {
    let snapshot: PipelineSnapshot

    var body: some View {
        CardContainer {
            VStack(alignment: .leading, spacing: 12) {
                Label("调用证据", systemImage: "checkmark.seal.fill")
                    .font(.headline)
                EvidenceRow(label: "意图结果", value: "coffee_order → searchDrinks")
                EvidenceRow(label: "本地 Skill", value: "examples/coffee-skill")
                EvidenceRow(label: "容器流水线", value: "searchDrinks → confirmOrder → payOrder → expire")
                EvidenceRow(label: "业务结果", value: "drink=\(snapshot.firstDrinkID), order=\(snapshot.orderID), status=\(snapshot.paymentStatus)")
                EvidenceRow(label: "审计记录", value: "\(snapshot.auditCount) runtime audit events")
                if !snapshot.componentActions.isEmpty {
                    EvidenceRow(label: "组件动作", value: snapshot.componentActions.map { "\($0.key)=\($0.value)" }.sorted().joined(separator: ", "))
                }
            }
        }
    }
}

private struct LogCard: View {
    let logLines: [String]

    var body: some View {
        CardContainer {
            VStack(alignment: .leading, spacing: 10) {
                ForEach(logLines, id: \.self) { line in
                    Text(line)
                        .font(.system(.caption, design: .monospaced))
                        .foregroundStyle(.secondary)
                }
            }
        }
    }
}

private struct MetricTile: View {
    let title: String
    let value: String

    var body: some View {
        VStack(alignment: .leading, spacing: 5) {
            Text(title)
                .font(.caption.weight(.semibold))
                .foregroundStyle(.secondary)
            Text(value)
                .font(.callout)
                .lineLimit(2)
                .truncationMode(.middle)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(12)
        .background(.secondary.opacity(0.08), in: RoundedRectangle(cornerRadius: 12, style: .continuous))
    }
}

private struct EvidenceRow: View {
    let label: String
    let value: String

    var body: some View {
        HStack(alignment: .top, spacing: 12) {
            Text(label)
                .font(.callout.weight(.semibold))
                .frame(width: 92, alignment: .leading)
            Text(value)
                .font(.callout)
                .foregroundStyle(.secondary)
                .textSelection(.enabled)
            Spacer(minLength: 0)
        }
    }
}

private struct CardContainer<Content: View>: View {
    @ViewBuilder let content: Content

    var body: some View {
        content
            .padding(16)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(.background, in: RoundedRectangle(cornerRadius: 16, style: .continuous))
            .overlay(
                RoundedRectangle(cornerRadius: 16, style: .continuous)
                    .stroke(.quaternary, lineWidth: 1)
            )
    }
}

private extension PipelineStep {
    var title: String {
        switch name {
        case "searchDrinks": return "搜索饮品组件"
        case "confirmOrder": return "确认订单组件"
        case "payOrder": return "支付结果组件"
        case "expire": return "卡片过期事件"
        default: return name
        }
    }

    var subtitle: String {
        switch name {
        case "searchDrinks": return "MiniApp component: drink-list"
        case "confirmOrder": return "MiniApp component: order-confirm"
        case "payOrder": return "MiniApp component: payment-result"
        case "expire": return "expirePreviousCards lifecycle"
        default: return renderRootKind
        }
    }

    var iconName: String {
        switch name {
        case "searchDrinks": return "cup.and.saucer.fill"
        case "confirmOrder": return "checklist.checked"
        case "payOrder": return "creditcard.fill"
        case "expire": return "clock.badge.checkmark"
        default: return "square.stack.3d.up.fill"
        }
    }

    var tint: Color {
        switch name {
        case "searchDrinks": return .brown
        case "confirmOrder": return .orange
        case "payOrder": return .green
        case "expire": return .blue
        default: return .accentColor
        }
    }
}

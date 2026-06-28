//
//  CommandRegistryView.swift
//  robdex
//
//  Design surface placeholder generated from the Flutter Agent Runtime UI inventory.
//

import SwiftUI

struct CommandRegistryView: View {
    let requests: [CommandRegistryRequest]
    let entries: [RegisteredCommandEntry]

    @State private var mode: CommandRegistryMode = .requests
    @State private var requestFilter: RegistryRequestFilter = .actionable

    private var visibleRequests: [CommandRegistryRequest] {
        requests.filter { requestFilter.includes($0) }
    }

    var body: some View {
        VStack(spacing: 0) {
            CommandRegistryHeader(mode: $mode, requestFilter: $requestFilter)

            ScrollView {
                Group {
                    switch mode {
                    case .requests:
                        RegistryRequestList(requests: visibleRequests)
                    case .entries:
                        RegisteredEntryList(entries: entries)
                    }
                }
                .padding(18)
                .frame(maxWidth: 1040)
                .frame(maxWidth: .infinity)
            }
            .background(CommandRegistryBackground())
        }
    }
}

private struct CommandRegistryHeader: View {
    @Binding var mode: CommandRegistryMode
    @Binding var requestFilter: RegistryRequestFilter

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            HStack(alignment: .firstTextBaseline, spacing: 14) {
                VStack(alignment: .leading, spacing: 3) {
                    Text("Command Registry")
                        .font(.system(size: 20, weight: .semibold, design: .rounded))
                        .foregroundStyle(.primary)

                    Text("Review structured command definitions before they can be used by agents.")
                        .font(.system(size: 13))
                        .foregroundStyle(.secondary)
                }

                Spacer()

                if mode == .requests {
                    Menu {
                        Picker("Filter", selection: $requestFilter) {
                            ForEach(RegistryRequestFilter.allCases) { option in
                                Label(option.title, systemImage: option.icon)
                                    .tag(option)
                            }
                        }
                    } label: {
                        Image(systemName: "line.3.horizontal.decrease.circle")
                    }
                    .menuStyle(.button)
                    .buttonStyle(.borderless)
                    .help("Filter command requests")
                }
            }

            Picker("Command registry section", selection: $mode) {
                ForEach(CommandRegistryMode.allCases) { option in
                    Text(option.title).tag(option)
                }
            }
            .pickerStyle(.segmented)
        }
        .padding(.horizontal, 18)
        .padding(.vertical, 16)
        .background(.regularMaterial)
        .overlay(alignment: .bottom) {
            Divider().opacity(0.6)
        }
    }
}

private struct RegistryRequestList: View {
    let requests: [CommandRegistryRequest]

    var body: some View {
        if requests.isEmpty {
            CommandRegistryEmptyState(
                title: "No command registry requests",
                message: "Open command definition requests will appear here."
            )
        } else {
            LazyVStack(alignment: .leading, spacing: 14) {
                ForEach(requests) { request in
                    RegistryRequestCard(request: request)
                }
            }
        }
    }
}

private struct RegistryRequestCard: View {
    let request: CommandRegistryRequest

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            RegistryRequestSummaryHeader(request: request)

            ViewThatFits(in: .horizontal) {
                HStack(alignment: .top, spacing: 14) {
                    RegistryRequestOverview(request: request)
                        .frame(maxWidth: 310, alignment: .topLeading)

                    RegistryRequestDetail(request: request)
                }

                VStack(alignment: .leading, spacing: 14) {
                    RegistryRequestOverview(request: request)
                    RegistryRequestDetail(request: request)
                }
            }

            RegistryDecisionActions(request: request)
        }
        .padding(15)
        .background(Color(red: 0.058, green: 0.078, blue: 0.105))
        .overlay {
            RoundedRectangle(cornerRadius: 16, style: .continuous)
                .stroke(request.approvalState.stroke, lineWidth: 1)
        }
        .clipShape(RoundedRectangle(cornerRadius: 16, style: .continuous))
    }
}

private struct RegistryRequestSummaryHeader: View {
    let request: CommandRegistryRequest

    var body: some View {
        HStack(alignment: .top, spacing: 12) {
            OperationGlyph(operation: request.operation)
                .padding(.top, 1)

            VStack(alignment: .leading, spacing: 5) {
                HStack(alignment: .firstTextBaseline, spacing: 8) {
                    Text(request.humanActionLabel)
                        .font(.system(size: 16, weight: .semibold))
                        .foregroundStyle(.primary)
                        .lineLimit(2)

                    Spacer(minLength: 10)

                    Text(request.approvalState.label)
                        .font(.system(size: 12, weight: .semibold))
                        .foregroundStyle(request.approvalState.color)
                        .lineLimit(1)
                }

                Text("\(request.operation.label) · \(request.actionId)")
                    .font(.system(size: 13, weight: .medium))
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
            }
        }
    }
}

private struct RegistryRequestOverview: View {
    let request: CommandRegistryRequest

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            VStack(spacing: 7) {
                FactRow(label: "Request", value: request.id)
                FactRow(label: "Requester", value: request.requestedBy)
                FactRow(label: "Session", value: request.sourceSession ?? "Not attached")
                FactRow(label: "Apply", value: request.applicationStatus.label)
                FactRow(label: "Scope", value: request.finalScope?.summary ?? "Not selected")
                FactRow(label: "Policy", value: request.finalExecutionPolicy?.summary ?? "Not selected")
            }

            ReadinessBlock(readiness: request.readiness)

            RiskSummaryBlock(title: "Risk", text: request.riskSummary)
        }
    }
}

private struct RegistryRequestDetail: View {
    let request: CommandRegistryRequest

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            CommandSeedSection(title: "Proposed command seed", seed: request.proposedCommand)

            if let finalCommand = request.finalCommand {
                CommandSeedSection(title: "Final command seed", seed: finalCommand)
            } else {
                MissingFinalCommandBlock()
            }

            if let current = request.currentRegistryState {
                CommandSeedSection(title: "Current registry state", seed: current)
            }

            SemanticDiffBlock(diffs: request.semanticDiffs)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

private struct CommandSeedSection: View {
    let title: String
    let seed: CommandSeed

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(title)
                .font(.system(size: 11, weight: .semibold))
                .foregroundStyle(.tertiary)
                .textCase(.uppercase)
                .tracking(1.0)

            VStack(spacing: 8) {
                FactRow(label: "Action", value: seed.actionId)
                FactRow(label: "Binary", value: seed.binaryName)
                FactRow(label: "Agent call", value: "cmd[\"\(seed.starlarkObject)\"].\(seed.starlarkMethod)(...)")
                FactRow(label: "Arguments", value: seed.argvTemplate)
                FactRow(label: "Directory", value: seed.defaultCwd)
                FactRow(label: "Runtime", value: seed.runtimeSummary)
                FactRow(label: "Lifecycle", value: seed.lifecycleSummary)
                FactRow(label: "Mutation", value: seed.mutationClass)
                FactRow(label: "Description", value: seed.modelDescription)
            }
            .padding(11)
            .background(Color.black.opacity(0.20))
            .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
        }
    }
}

private struct ReadinessBlock: View {
    let readiness: RegistryRequestReadiness

    var body: some View {
        VStack(alignment: .leading, spacing: 7) {
            Text("Readiness")
                .font(.system(size: 11, weight: .semibold))
                .foregroundStyle(.tertiary)
                .textCase(.uppercase)
                .tracking(1.0)

            ReadinessLine(label: "Final scope", ready: readiness.hasFinalScope)
            ReadinessLine(label: "Execution policy", ready: readiness.hasFinalExecutionPolicy)
            ReadinessLine(label: "Final command", ready: readiness.hasFinalCommand)
        }
        .padding(11)
        .background(Color.primary.opacity(0.035))
        .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
    }
}

private struct ReadinessLine: View {
    let label: String
    let ready: Bool

    var body: some View {
        HStack(spacing: 8) {
            Image(systemName: ready ? "checkmark.circle.fill" : "circle")
                .font(.system(size: 12, weight: .semibold))
                .foregroundStyle(ready ? Color(red: 0.47, green: 0.86, blue: 0.62) : .secondary)

            Text(label)
                .font(.system(size: 13))
                .foregroundStyle(.secondary)
        }
    }
}

private struct RiskSummaryBlock: View {
    let title: String
    let text: String

    var body: some View {
        VStack(alignment: .leading, spacing: 5) {
            Text(title)
                .font(.system(size: 11, weight: .semibold))
                .foregroundStyle(.tertiary)
                .textCase(.uppercase)
                .tracking(1.0)

            Text(text)
                .font(.system(size: 13))
                .lineSpacing(2)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
        }
    }
}

private struct MissingFinalCommandBlock: View {
    var body: some View {
        Text("Final command fields have not been selected yet.")
            .font(.system(size: 13, weight: .medium))
            .foregroundStyle(.secondary)
            .padding(11)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(Color.primary.opacity(0.035))
            .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
    }
}

private struct SemanticDiffBlock: View {
    let diffs: [SemanticDiff]

    var body: some View {
        if !diffs.isEmpty {
            VStack(alignment: .leading, spacing: 8) {
                Text("Semantic diff")
                    .font(.system(size: 11, weight: .semibold))
                    .foregroundStyle(.tertiary)
                    .textCase(.uppercase)
                    .tracking(1.0)

                ForEach(diffs) { diff in
                    HStack(alignment: .top, spacing: 8) {
                        Image(systemName: diff.kind.icon)
                            .font(.system(size: 12, weight: .semibold))
                            .foregroundStyle(diff.kind.color)
                            .frame(width: 14)

                        VStack(alignment: .leading, spacing: 2) {
                            Text(diff.title)
                                .font(.system(size: 13, weight: .semibold))
                                .foregroundStyle(.primary)

                            Text(diff.detail)
                                .font(.system(size: 12))
                                .foregroundStyle(.secondary)
                                .fixedSize(horizontal: false, vertical: true)
                        }
                    }
                }
            }
        }
    }
}

private struct RegistryDecisionActions: View {
    let request: CommandRegistryRequest

    var body: some View {
        switch request.approvalState {
        case .needsDecision, .previewReady:
            HStack(spacing: 8) {
                Button("Preview decision") {}
                    .buttonStyle(.bordered)
                    .disabled(!request.canPreview)

                Button("Approve") {}
                    .buttonStyle(.borderedProminent)
                    .disabled(!request.canDecide || !request.readiness.readyForApproval)

                Button("Deny") {}
                    .buttonStyle(.bordered)
                    .disabled(!request.canDecide)
            }
            .controlSize(.small)

        case .approved:
            HStack(spacing: 8) {
                Button("Apply") {}
                    .buttonStyle(.borderedProminent)
                    .disabled(!request.canApply)

                Text(request.applicationStatus.explanation)
                    .font(.system(size: 12, weight: .medium))
                    .foregroundStyle(.secondary)
            }
            .controlSize(.small)

        case .waiting:
            MutedMessage(text: "Waiting for another reviewer or runtime check.")

        case .denied:
            MutedMessage(text: "Denied requests are hidden unless the filter includes them.")

        case .applied:
            MutedMessage(text: "This command registry request has already been applied.")

        case .stale:
            MutedMessage(text: "This request no longer matches the current registry state.")
        }
    }
}

private struct RegisteredEntryList: View {
    let entries: [RegisteredCommandEntry]

    var body: some View {
        if entries.isEmpty {
            CommandRegistryEmptyState(
                title: "No registered commands",
                message: "Applied command definitions will appear here."
            )
        } else {
            LazyVStack(alignment: .leading, spacing: 10) {
                ForEach(entries) { entry in
                    RegisteredEntryRow(entry: entry)
                }
            }
        }
    }
}

private struct RegisteredEntryRow: View {
    let entry: RegisteredCommandEntry

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(alignment: .firstTextBaseline, spacing: 10) {
                VStack(alignment: .leading, spacing: 4) {
                    Text(entry.actionLabel)
                        .font(.system(size: 15, weight: .semibold))
                        .foregroundStyle(.primary)

                    Text(entry.seed.modelDescription)
                        .font(.system(size: 13))
                        .foregroundStyle(.secondary)
                        .lineLimit(2)
                }

                Spacer(minLength: 12)

                Text(entry.enabled ? "Enabled" : "Disabled")
                    .font(.system(size: 12, weight: .semibold))
                    .foregroundStyle(entry.enabled ? Color(red: 0.47, green: 0.86, blue: 0.62) : .secondary)
            }

            VStack(spacing: 8) {
                FactRow(label: "Action", value: entry.seed.actionId)
                FactRow(label: "Scope", value: entry.scopeSummary)
                FactRow(label: "Version", value: entry.versionSummary)
                FactRow(label: "Binary", value: entry.seed.binaryName)
                FactRow(label: "Agent call", value: "cmd[\"\(entry.seed.starlarkObject)\"].\(entry.seed.starlarkMethod)(...)")
                FactRow(label: "Arguments", value: entry.seed.argvTemplate)
                FactRow(label: "Policies", value: entry.seed.policySummary)
                FactRow(label: "Lifecycle", value: entry.seed.lifecycleSummary)
                FactRow(label: "Mutation", value: entry.seed.mutationClass)
            }
            .padding(11)
            .background(Color.black.opacity(0.20))
            .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
        }
        .padding(14)
        .background(Color(red: 0.058, green: 0.078, blue: 0.105))
        .overlay {
            RoundedRectangle(cornerRadius: 14, style: .continuous)
                .stroke(Color.primary.opacity(0.10), lineWidth: 1)
        }
        .clipShape(RoundedRectangle(cornerRadius: 14, style: .continuous))
    }
}

private struct OperationGlyph: View {
    let operation: RegistryOperation

    var body: some View {
        Image(systemName: operation.icon)
            .font(.system(size: 15, weight: .bold))
            .foregroundStyle(operation.color)
            .frame(width: 28, height: 28)
            .background(operation.color.opacity(0.14))
            .clipShape(RoundedRectangle(cornerRadius: 8, style: .continuous))
    }
}

private struct FactRow: View {
    let label: String
    let value: String

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 12) {
            Text(label)
                .font(.system(size: 12, weight: .medium))
                .foregroundStyle(.tertiary)
                .frame(width: 82, alignment: .leading)

            Text(value)
                .font(.system(size: 13))
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)

            Spacer(minLength: 0)
        }
    }
}

private struct MutedMessage: View {
    let text: String

    var body: some View {
        Text(text)
            .font(.system(size: 12, weight: .medium))
            .foregroundStyle(.secondary)
            .padding(.horizontal, 10)
            .padding(.vertical, 8)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(Color.primary.opacity(0.035))
            .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
    }
}

private struct CommandRegistryEmptyState: View {
    let title: String
    let message: String

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(title)
                .font(.system(size: 15, weight: .semibold))
                .foregroundStyle(.primary)

            Text(message)
                .font(.system(size: 13))
                .foregroundStyle(.secondary)
        }
        .padding(16)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Color(red: 0.058, green: 0.078, blue: 0.105))
        .clipShape(RoundedRectangle(cornerRadius: 14, style: .continuous))
    }
}

private struct CommandRegistryBackground: View {
    var body: some View {
        ZStack {
            Color(red: 0.030, green: 0.040, blue: 0.056)

            LinearGradient(
                colors: [
                    Color.white.opacity(0.026),
                    Color.clear,
                    Color.black.opacity(0.10)
                ],
                startPoint: .top,
                endPoint: .bottom
            )
        }
    }
}

private enum CommandRegistryMode: String, CaseIterable, Identifiable {
    case requests
    case entries

    var id: String { rawValue }

    var title: String {
        switch self {
        case .requests:
            return "Requests"
        case .entries:
            return "Registered"
        }
    }
}

private enum RegistryRequestFilter: String, CaseIterable, Identifiable {
    case actionable
    case all

    var id: String { rawValue }

    var title: String {
        switch self {
        case .actionable:
            return "Actionable"
        case .all:
            return "Show hidden"
        }
    }

    var icon: String {
        switch self {
        case .actionable:
            return "hand.tap"
        case .all:
            return "tray.full"
        }
    }

    func includes(_ request: CommandRegistryRequest) -> Bool {
        switch self {
        case .actionable:
            return request.approvalState.isActionable || request.canApply
        case .all:
            return true
        }
    }
}

struct CommandRegistryRequest: Identifiable {
    let id: String
    let operation: RegistryOperation
    let actionId: String
    let humanActionLabel: String
    let approvalState: RegistryApprovalState
    let applicationStatus: RegistryApplicationStatus
    let finalScope: RegistryScope?
    let finalExecutionPolicy: RegistryExecutionPolicy?
    let canPreview: Bool
    let canDecide: Bool
    let canApply: Bool
    let requestedBy: String
    let sourceSession: String?
    let proposedCommand: CommandSeed
    let finalCommand: CommandSeed?
    let currentRegistryState: CommandSeed?
    let readiness: RegistryRequestReadiness
    let semanticDiffs: [SemanticDiff]
    let riskSummary: String
}

struct CommandSeed {
    let actionId: String
    let binaryName: String
    let candidatePaths: String
    let starlarkObject: String
    let starlarkMethod: String
    let argvPrefix: String
    let argvTemplate: String
    let defaultCwd: String
    let cwdPolicy: String
    let envPolicy: String
    let stdinPolicy: String
    let syncAllowed: Bool
    let asyncAllowed: Bool
    let maxRuntimeMs: Int
    let endOfTurnBehavior: String
    let endOfSessionBehavior: String
    let mutationClass: String
    let modelDescription: String
    let allowCwdArg: Bool
    let allowArgsArg: Bool
    let forbiddenArgs: String
    let executionPolicy: String

    var runtimeSummary: String {
        let sync = syncAllowed ? "sync" : "no sync"
        let async = asyncAllowed ? "async" : "no async"
        return "\(sync), \(async), \(maxRuntimeMs) ms"
    }

    var lifecycleSummary: String {
        "Turn: \(endOfTurnBehavior); session: \(endOfSessionBehavior)"
    }

    var policySummary: String {
        "cwd: \(cwdPolicy); env: \(envPolicy); stdin: \(stdinPolicy); exec: \(executionPolicy)"
    }
}

struct RegistryScope {
    let summary: String
}

struct RegistryExecutionPolicy {
    let summary: String
}

struct RegistryRequestReadiness {
    let hasFinalScope: Bool
    let hasFinalExecutionPolicy: Bool
    let hasFinalCommand: Bool

    var readyForApproval: Bool {
        hasFinalScope && hasFinalExecutionPolicy && hasFinalCommand
    }
}

struct SemanticDiff: Identifiable {
    let id: String
    let title: String
    let detail: String
    let kind: SemanticDiffKind
}

enum SemanticDiffKind {
    case added
    case changed
    case removed
    case risk

    var icon: String {
        switch self {
        case .added:
            return "plus.circle.fill"
        case .changed:
            return "arrow.triangle.2.circlepath"
        case .removed:
            return "minus.circle.fill"
        case .risk:
            return "exclamationmark.triangle.fill"
        }
    }

    var color: Color {
        switch self {
        case .added:
            return Color(red: 0.47, green: 0.86, blue: 0.62)
        case .changed:
            return Color(red: 0.47, green: 0.70, blue: 0.92)
        case .removed, .risk:
            return Color(red: 0.98, green: 0.67, blue: 0.25)
        }
    }
}

struct RegisteredCommandEntry: Identifiable {
    let id: String
    let actionLabel: String
    let scopeSummary: String
    let enabled: Bool
    let versionSummary: String
    let seed: CommandSeed
}

enum RegistryOperation {
    case add
    case update
    case disable
    case enable

    var label: String {
        switch self {
        case .add:
            return "Add"
        case .update:
            return "Update"
        case .disable:
            return "Disable"
        case .enable:
            return "Enable"
        }
    }

    var icon: String {
        switch self {
        case .add:
            return "plus"
        case .update:
            return "pencil"
        case .disable:
            return "minus"
        case .enable:
            return "checkmark"
        }
    }

    var color: Color {
        switch self {
        case .add, .enable:
            return Color(red: 0.47, green: 0.86, blue: 0.62)
        case .update:
            return Color(red: 0.47, green: 0.70, blue: 0.92)
        case .disable:
            return Color(red: 0.98, green: 0.67, blue: 0.25)
        }
    }
}

enum RegistryApprovalState {
    case needsDecision
    case previewReady
    case approved
    case waiting
    case denied
    case applied
    case stale

    var isActionable: Bool {
        switch self {
        case .needsDecision, .previewReady:
            return true
        case .approved, .waiting, .denied, .applied, .stale:
            return false
        }
    }

    var label: String {
        switch self {
        case .needsDecision:
            return "Decision needed"
        case .previewReady:
            return "Preview ready"
        case .approved:
            return "Approved"
        case .waiting:
            return "Waiting"
        case .denied:
            return "Denied"
        case .applied:
            return "Applied"
        case .stale:
            return "Stale"
        }
    }

    var color: Color {
        switch self {
        case .needsDecision, .previewReady, .approved:
            return Color(red: 0.47, green: 0.70, blue: 0.92)
        case .waiting:
            return Color(red: 0.62, green: 0.70, blue: 0.80)
        case .applied:
            return Color(red: 0.47, green: 0.86, blue: 0.62)
        case .denied, .stale:
            return Color(red: 1.00, green: 0.42, blue: 0.38)
        }
    }

    var stroke: Color {
        switch self {
        case .needsDecision, .previewReady, .approved:
            return Color(red: 0.47, green: 0.70, blue: 0.92).opacity(0.30)
        case .denied, .stale:
            return Color(red: 1.00, green: 0.42, blue: 0.38).opacity(0.30)
        default:
            return Color.primary.opacity(0.10)
        }
    }
}

enum RegistryApplicationStatus {
    case notReady
    case pending
    case applied
    case failed

    var label: String {
        switch self {
        case .notReady:
            return "Not ready"
        case .pending:
            return "Pending apply"
        case .applied:
            return "Applied"
        case .failed:
            return "Apply failed"
        }
    }

    var explanation: String {
        switch self {
        case .notReady:
            return "Approve the decision before applying."
        case .pending:
            return "Approved and waiting to write."
        case .applied:
            return "Already written to the registry."
        case .failed:
            return "The registry write failed."
        }
    }
}

#Preview(traits: .landscapeLeft) {
    let statusSeed = CommandSeed(
        actionId: "cmd.git.status",
        binaryName: "git",
        candidatePaths: "/usr/bin/git, /opt/homebrew/bin/git",
        starlarkObject: "git",
        starlarkMethod: "status",
        argvPrefix: "status --short",
        argvTemplate: "git status --short",
        defaultCwd: "Project workspace",
        cwdPolicy: "project only",
        envPolicy: "inherit safe environment",
        stdinPolicy: "closed",
        syncAllowed: true,
        asyncAllowed: false,
        maxRuntimeMs: 10000,
        endOfTurnBehavior: "complete",
        endOfSessionBehavior: "stop",
        mutationClass: "read-only",
        modelDescription: "Shows changed files in the selected project.",
        allowCwdArg: false,
        allowArgsArg: false,
        forbiddenArgs: "reset, clean, checkout",
        executionPolicy: "owner approved"
    )

    let searchSeed = CommandSeed(
        actionId: "cmd.search.files",
        binaryName: "rg",
        candidatePaths: "/opt/homebrew/bin/rg, /usr/bin/rg",
        starlarkObject: "search",
        starlarkMethod: "files",
        argvPrefix: "--files",
        argvTemplate: "rg --files <project>",
        defaultCwd: "Project workspace",
        cwdPolicy: "project only",
        envPolicy: "inherit safe environment",
        stdinPolicy: "closed",
        syncAllowed: true,
        asyncAllowed: false,
        maxRuntimeMs: 15000,
        endOfTurnBehavior: "complete",
        endOfSessionBehavior: "stop",
        mutationClass: "read-only",
        modelDescription: "Lists files under the selected project root.",
        allowCwdArg: false,
        allowArgsArg: true,
        forbiddenArgs: "--hidden outside project",
        executionPolicy: "owner approved"
    )

    let xcodeSeed = CommandSeed(
        actionId: "cmd.xcode.list",
        binaryName: "xcodebuild",
        candidatePaths: "/usr/bin/xcodebuild",
        starlarkObject: "xcode",
        starlarkMethod: "list",
        argvPrefix: "-list",
        argvTemplate: "xcodebuild -list",
        defaultCwd: "Selected project",
        cwdPolicy: "project only",
        envPolicy: "inherit developer environment",
        stdinPolicy: "closed",
        syncAllowed: true,
        asyncAllowed: true,
        maxRuntimeMs: 30000,
        endOfTurnBehavior: "complete",
        endOfSessionBehavior: "terminate",
        mutationClass: "read-only",
        modelDescription: "Shows Xcode schemes and project metadata.",
        allowCwdArg: true,
        allowArgsArg: false,
        forbiddenArgs: "build, test, archive",
        executionPolicy: "owner approved"
    )

    let requests = [
        CommandRegistryRequest(
            id: "req-001",
            operation: .add,
            actionId: "cmd.git.status",
            humanActionLabel: "Add project status command",
            approvalState: .needsDecision,
            applicationStatus: .notReady,
            finalScope: RegistryScope(summary: "Current project"),
            finalExecutionPolicy: RegistryExecutionPolicy(summary: "Owner approval required"),
            canPreview: true,
            canDecide: true,
            canApply: false,
            requestedBy: "Runtime allow",
            sourceSession: "Release notes cleanup",
            proposedCommand: statusSeed,
            finalCommand: nil,
            currentRegistryState: nil,
            readiness: RegistryRequestReadiness(hasFinalScope: true, hasFinalExecutionPolicy: true, hasFinalCommand: false),
            semanticDiffs: [
                SemanticDiff(id: "new-entry", title: "New registry entry", detail: "Adds a read-only Git status action.", kind: .added),
                SemanticDiff(id: "missing-final", title: "Final command missing", detail: "Reviewer must confirm or edit command fields before approval.", kind: .risk)
            ],
            riskSummary: "Read-only command. Risk is limited to project path disclosure."
        ),
        CommandRegistryRequest(
            id: "req-002",
            operation: .update,
            actionId: "cmd.search.files",
            humanActionLabel: "Update project file search",
            approvalState: .previewReady,
            applicationStatus: .notReady,
            finalScope: RegistryScope(summary: "Current project"),
            finalExecutionPolicy: RegistryExecutionPolicy(summary: "Allowed after approval"),
            canPreview: true,
            canDecide: true,
            canApply: false,
            requestedBy: "Runtime approval",
            sourceSession: "CLI install check",
            proposedCommand: searchSeed,
            finalCommand: searchSeed,
            currentRegistryState: statusSeed,
            readiness: RegistryRequestReadiness(hasFinalScope: true, hasFinalExecutionPolicy: true, hasFinalCommand: true),
            semanticDiffs: [
                SemanticDiff(id: "binary", title: "Binary changed", detail: "Final command uses rg instead of git.", kind: .changed),
                SemanticDiff(id: "args", title: "Argument template narrowed", detail: "Search remains scoped to the selected project.", kind: .changed)
            ],
            riskSummary: "Read-only search. Confirm hidden-file behavior before approving."
        ),
        CommandRegistryRequest(
            id: "req-003",
            operation: .add,
            actionId: "cmd.xcode.list",
            humanActionLabel: "Apply Xcode project inspection",
            approvalState: .approved,
            applicationStatus: .pending,
            finalScope: RegistryScope(summary: "Selected project"),
            finalExecutionPolicy: RegistryExecutionPolicy(summary: "Owner approval required"),
            canPreview: false,
            canDecide: false,
            canApply: true,
            requestedBy: "Runtime allow",
            sourceSession: "Starter kit evidence",
            proposedCommand: xcodeSeed,
            finalCommand: xcodeSeed,
            currentRegistryState: nil,
            readiness: RegistryRequestReadiness(hasFinalScope: true, hasFinalExecutionPolicy: true, hasFinalCommand: true),
            semanticDiffs: [
                SemanticDiff(id: "add-xcode", title: "New registry entry", detail: "Adds read-only Xcode project inspection.", kind: .added)
            ],
            riskSummary: "Read-only developer tooling command. May be slower on large workspaces."
        ),
        CommandRegistryRequest(
            id: "req-004",
            operation: .disable,
            actionId: "cmd.legacy.build",
            humanActionLabel: "Disable legacy build command",
            approvalState: .waiting,
            applicationStatus: .notReady,
            finalScope: nil,
            finalExecutionPolicy: nil,
            canPreview: false,
            canDecide: false,
            canApply: false,
            requestedBy: "Role admin",
            sourceSession: nil,
            proposedCommand: xcodeSeed,
            finalCommand: nil,
            currentRegistryState: xcodeSeed,
            readiness: RegistryRequestReadiness(hasFinalScope: false, hasFinalExecutionPolicy: false, hasFinalCommand: false),
            semanticDiffs: [
                SemanticDiff(id: "disable", title: "Disable requested", detail: "Existing command would be hidden from agents.", kind: .removed)
            ],
            riskSummary: "Disabling may break sessions that rely on this action."
        )
    ]

    let entries = [
        RegisteredCommandEntry(
            id: "cmd.git.status",
            actionLabel: "Project status",
            scopeSummary: "Global default · enabled for current project",
            enabled: true,
            versionSummary: "Version 3",
            seed: statusSeed
        ),
        RegisteredCommandEntry(
            id: "cmd.search.files",
            actionLabel: "Search project files",
            scopeSummary: "Current project",
            enabled: true,
            versionSummary: "Version 1",
            seed: searchSeed
        ),
        RegisteredCommandEntry(
            id: "cmd.xcode.list",
            actionLabel: "Inspect Xcode project",
            scopeSummary: "Selected project",
            enabled: false,
            versionSummary: "Pending version",
            seed: xcodeSeed
        )
    ]

    CommandRegistryView(requests: requests, entries: entries)
        .frame(width: 980, height: 760)
        .preferredColorScheme(.dark)
}

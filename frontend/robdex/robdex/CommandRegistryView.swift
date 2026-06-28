//
//  CommandRegistryView.swift
//  robdex
//
//  Design surface placeholder generated from the Flutter Agent Runtime UI inventory.
//

import SwiftUI

struct CommandRegistryView: View {
    let requests: [CommandRegistryRequest]
    let commands: [RegisteredCommand]

    @State private var mode: CommandRegistryMode = .requests
    @State private var requestFilter: CommandRequestFilter = .actionable

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
                        CommandRequestList(requests: visibleRequests)
                    case .commands:
                        RegisteredCommandList(commands: commands)
                    }
                }
                .padding(18)
                .frame(maxWidth: 880)
                .frame(maxWidth: .infinity)
            }
            .background(CommandRegistryBackground())
        }
    }
}

private struct CommandRegistryHeader: View {
    @Binding var mode: CommandRegistryMode
    @Binding var requestFilter: CommandRequestFilter

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            HStack(alignment: .firstTextBaseline, spacing: 14) {
                VStack(alignment: .leading, spacing: 3) {
                    Text("Command Registry")
                        .font(.system(size: 20, weight: .semibold, design: .rounded))
                        .foregroundStyle(.primary)

                    Text("Review requested commands and inspect available actions.")
                        .font(.system(size: 13))
                        .foregroundStyle(.secondary)
                }

                Spacer()

                if mode == .requests {
                    Menu {
                        Picker("Filter", selection: $requestFilter) {
                            ForEach(CommandRequestFilter.allCases) { option in
                                Label(option.title, systemImage: option.icon)
                                    .tag(option)
                            }
                        }
                    } label: {
                        Image(systemName: "line.3.horizontal.decrease.circle")
                    }
                    .menuStyle(.button)
                    .buttonStyle(.borderless)
                    .help("Filter requests")
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

private struct CommandRequestList: View {
    let requests: [CommandRegistryRequest]

    var body: some View {
        if requests.isEmpty {
            CommandRegistryEmptyState(
                title: "No command requests",
                body: "Requests that need your decision will appear here."
            )
        } else {
            LazyVStack(alignment: .leading, spacing: 12) {
                ForEach(requests) { request in
                    CommandRequestCard(request: request)
                }
            }
        }
    }
}

private struct CommandRequestCard: View {
    let request: CommandRegistryRequest

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(alignment: .top, spacing: 12) {
                Image(systemName: request.state.icon)
                    .font(.system(size: 17, weight: .semibold))
                    .foregroundStyle(request.state.color)
                    .frame(width: 22, height: 22)
                    .padding(.top, 2)

                VStack(alignment: .leading, spacing: 5) {
                    HStack(alignment: .firstTextBaseline, spacing: 8) {
                        Text(request.title)
                            .font(.system(size: 15, weight: .semibold))
                            .foregroundStyle(.primary)
                            .lineLimit(2)

                        Spacer(minLength: 10)

                        Text(request.state.label)
                            .font(.system(size: 12, weight: .semibold))
                            .foregroundStyle(request.state.color)
                            .lineLimit(1)
                    }

                    Text(request.summary)
                        .font(.system(size: 13))
                        .lineSpacing(2)
                        .foregroundStyle(.secondary)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }

            CommandPreviewBlock(command: request.command)

            VStack(spacing: 7) {
                CommandFactRow(label: "Scope", value: request.scope)
                CommandFactRow(label: "Policy", value: request.policy)
                CommandFactRow(label: "Requested by", value: request.requestedBy)
            }

            CommandRequestActions(state: request.state)
        }
        .padding(14)
        .background(request.state.surface)
        .overlay {
            RoundedRectangle(cornerRadius: 15, style: .continuous)
                .stroke(request.state.stroke, lineWidth: 1)
        }
        .clipShape(RoundedRectangle(cornerRadius: 15, style: .continuous))
    }
}

private struct CommandRequestActions: View {
    let state: CommandRequestState

    var body: some View {
        switch state {
        case .needsReview:
            HStack(spacing: 8) {
                Button("Preview") {}
                    .buttonStyle(.bordered)

                Button("Allow") {}
                    .buttonStyle(.borderedProminent)

                Button("Deny") {}
                    .buttonStyle(.bordered)
            }
            .controlSize(.small)

        case .previewReady:
            HStack(spacing: 8) {
                Button("Allow") {}
                    .buttonStyle(.borderedProminent)

                Button("Deny") {}
                    .buttonStyle(.bordered)

                Text("Preview is ready.")
                    .font(.system(size: 12, weight: .medium))
                    .foregroundStyle(.secondary)
            }
            .controlSize(.small)

        case .readyToApply:
            HStack(spacing: 8) {
                Button("Apply") {}
                    .buttonStyle(.borderedProminent)

                Text("Approved but not applied to this session.")
                    .font(.system(size: 12, weight: .medium))
                    .foregroundStyle(.secondary)
            }
            .controlSize(.small)

        case .waiting:
            CommandMutedMessage(text: "Waiting for another runtime decision.")

        case .denied:
            CommandMutedMessage(text: "Denied requests are read-only.")

        case .applied:
            CommandMutedMessage(text: "This command has already been applied.")

        case .unavailable:
            CommandMutedMessage(text: "This request no longer matches the current session.")
        }
    }
}

private struct RegisteredCommandList: View {
    let commands: [RegisteredCommand]

    var body: some View {
        if commands.isEmpty {
            CommandRegistryEmptyState(
                title: "No commands available",
                body: "Commands exposed by the runtime will appear here."
            )
        } else {
            LazyVStack(alignment: .leading, spacing: 10) {
                ForEach(commands) { command in
                    RegisteredCommandRow(command: command)
                }
            }
        }
    }
}

private struct RegisteredCommandRow: View {
    let command: RegisteredCommand

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(alignment: .firstTextBaseline, spacing: 10) {
                VStack(alignment: .leading, spacing: 4) {
                    Text(command.title)
                        .font(.system(size: 15, weight: .semibold))
                        .foregroundStyle(.primary)

                    Text(command.summary)
                        .font(.system(size: 13))
                        .foregroundStyle(.secondary)
                        .lineLimit(2)
                }

                Spacer(minLength: 12)

                Text(command.availability)
                    .font(.system(size: 12, weight: .semibold))
                    .foregroundStyle(command.available ? Color(red: 0.47, green: 0.86, blue: 0.62) : .secondary)
            }

            CommandPreviewBlock(command: command.command)

            HStack(spacing: 10) {
                Text(command.scope)
                    .font(.system(size: 12, weight: .medium))
                    .foregroundStyle(.secondary)

                Spacer()

                Button("Show details") {}
                    .buttonStyle(.bordered)
                    .controlSize(.small)
            }
        }
        .padding(14)
        .background(Color(red: 0.060, green: 0.080, blue: 0.105))
        .overlay {
            RoundedRectangle(cornerRadius: 14, style: .continuous)
                .stroke(Color.primary.opacity(0.10), lineWidth: 1)
        }
        .clipShape(RoundedRectangle(cornerRadius: 14, style: .continuous))
    }
}

private struct CommandPreviewBlock: View {
    let command: String

    var body: some View {
        HStack(alignment: .top, spacing: 8) {
            Image(systemName: "terminal")
                .font(.system(size: 12, weight: .semibold))
                .foregroundStyle(.secondary)
                .padding(.top, 1)

            Text(command)
                .font(.system(size: 12, weight: .medium, design: .monospaced))
                .foregroundStyle(Color(red: 0.78, green: 0.86, blue: 0.92))
                .lineLimit(3)
                .fixedSize(horizontal: false, vertical: true)

            Spacer(minLength: 0)
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 8)
        .background(Color.black.opacity(0.22))
        .clipShape(RoundedRectangle(cornerRadius: 9, style: .continuous))
    }
}

private struct CommandFactRow: View {
    let label: String
    let value: String

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 12) {
            Text(label)
                .font(.system(size: 12, weight: .medium))
                .foregroundStyle(.tertiary)
                .frame(width: 88, alignment: .leading)

            Text(value)
                .font(.system(size: 13))
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)

            Spacer(minLength: 0)
        }
    }
}

private struct CommandMutedMessage: View {
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
    let body: String

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(title)
                .font(.system(size: 15, weight: .semibold))
                .foregroundStyle(.primary)

            Text(body)
                .font(.system(size: 13))
                .foregroundStyle(.secondary)
        }
        .padding(16)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Color(red: 0.060, green: 0.080, blue: 0.105))
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
    case commands

    var id: String { rawValue }

    var title: String {
        switch self {
        case .requests:
            return "Requests"
        case .commands:
            return "Commands"
        }
    }
}

private enum CommandRequestFilter: String, CaseIterable, Identifiable {
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
            return request.state.isUserActionable
        case .all:
            return true
        }
    }
}

struct CommandRegistryRequest: Identifiable {
    let id: String
    let title: String
    let summary: String
    let command: String
    let scope: String
    let policy: String
    let requestedBy: String
    let state: CommandRequestState
}

struct RegisteredCommand: Identifiable {
    let id: String
    let title: String
    let summary: String
    let command: String
    let scope: String
    let available: Bool
    let availability: String
}

enum CommandRequestState {
    case needsReview
    case previewReady
    case readyToApply
    case waiting
    case denied
    case applied
    case unavailable

    var isUserActionable: Bool {
        switch self {
        case .needsReview, .previewReady, .readyToApply:
            return true
        case .waiting, .denied, .applied, .unavailable:
            return false
        }
    }

    var label: String {
        switch self {
        case .needsReview:
            return "Review needed"
        case .previewReady:
            return "Preview ready"
        case .readyToApply:
            return "Ready to apply"
        case .waiting:
            return "Waiting"
        case .denied:
            return "Denied"
        case .applied:
            return "Applied"
        case .unavailable:
            return "Unavailable"
        }
    }

    var icon: String {
        switch self {
        case .needsReview:
            return "command.circle.fill"
        case .previewReady:
            return "doc.text.magnifyingglass"
        case .readyToApply:
            return "arrow.down.circle.fill"
        case .waiting:
            return "clock.fill"
        case .denied:
            return "xmark.circle.fill"
        case .applied:
            return "checkmark.circle.fill"
        case .unavailable:
            return "exclamationmark.triangle.fill"
        }
    }

    var color: Color {
        switch self {
        case .needsReview, .previewReady, .readyToApply:
            return Color(red: 0.98, green: 0.67, blue: 0.25)
        case .waiting:
            return Color(red: 0.62, green: 0.70, blue: 0.80)
        case .applied:
            return Color(red: 0.47, green: 0.86, blue: 0.62)
        case .denied, .unavailable:
            return Color(red: 1.00, green: 0.42, blue: 0.38)
        }
    }

    var surface: Color {
        switch self {
        case .needsReview, .previewReady, .readyToApply:
            return Color(red: 0.120, green: 0.090, blue: 0.052)
        case .denied, .unavailable:
            return Color(red: 0.120, green: 0.050, blue: 0.052)
        default:
            return Color(red: 0.060, green: 0.080, blue: 0.105)
        }
    }

    var stroke: Color {
        switch self {
        case .needsReview, .previewReady, .readyToApply:
            return Color(red: 0.98, green: 0.67, blue: 0.25).opacity(0.30)
        case .denied, .unavailable:
            return Color(red: 1.00, green: 0.42, blue: 0.38).opacity(0.30)
        default:
            return Color.primary.opacity(0.10)
        }
    }
}

#Preview(traits: .landscapeLeft) {
    let requests = [
        CommandRegistryRequest(
            id: "needs-review",
            title: "Allow project status command",
            summary: "The runtime requested a command that needs owner review before it can be used.",
            command: "git status --short",
            scope: "Current project",
            policy: "Owner approval",
            requestedBy: "Runtime allow",
            state: .needsReview
        ),
        CommandRegistryRequest(
            id: "preview-ready",
            title: "Review generated search command",
            summary: "A preview is available. Decide whether this command should be allowed.",
            command: "rg --files frontend/robdex",
            scope: "Current project",
            policy: "Owner approval",
            requestedBy: "Runtime approval",
            state: .previewReady
        ),
        CommandRegistryRequest(
            id: "ready-to-apply",
            title: "Apply approved build check",
            summary: "This command was approved and can now be applied to the selected session.",
            command: "xcodebuild -list",
            scope: "Selected session",
            policy: "Allowed after approval",
            requestedBy: "Runtime allow",
            state: .readyToApply
        ),
        CommandRegistryRequest(
            id: "waiting",
            title: "Waiting for registry owner",
            summary: "Another runtime actor needs to finish reviewing this request.",
            command: "swift test",
            scope: "Project workspace",
            policy: "Review required",
            requestedBy: "Role admin",
            state: .waiting
        ),
        CommandRegistryRequest(
            id: "denied",
            title: "Denied destructive command",
            summary: "This command was denied and is hidden unless filtered in.",
            command: "git reset --hard",
            scope: "Project workspace",
            policy: "Denied",
            requestedBy: "Runtime approval",
            state: .denied
        ),
        CommandRegistryRequest(
            id: "applied",
            title: "Applied status command",
            summary: "This request has already been handled.",
            command: "git status --short",
            scope: "Current project",
            policy: "Allowed",
            requestedBy: "Runtime allow",
            state: .applied
        ),
        CommandRegistryRequest(
            id: "unavailable",
            title: "Stale command request",
            summary: "The request no longer matches the selected session state.",
            command: "npm run build",
            scope: "Previous session",
            policy: "Review required",
            requestedBy: "Runtime no-rg",
            state: .unavailable
        )
    ]

    let commands = [
        RegisteredCommand(
            id: "status",
            title: "Project status",
            summary: "Shows the current project changes.",
            command: "git status --short",
            scope: "Current project",
            available: true,
            availability: "Available"
        ),
        RegisteredCommand(
            id: "search-files",
            title: "Search project files",
            summary: "Lists files in the selected project scope.",
            command: "rg --files",
            scope: "Current project",
            available: true,
            availability: "Available"
        ),
        RegisteredCommand(
            id: "build-list",
            title: "Inspect Xcode project",
            summary: "Shows build schemes and project metadata.",
            command: "xcodebuild -list",
            scope: "Selected project",
            available: false,
            availability: "Needs review"
        )
    ]

    CommandRegistryView(requests: requests, commands: commands)
        .frame(width: 860, height: 760)
        .preferredColorScheme(.dark)
}

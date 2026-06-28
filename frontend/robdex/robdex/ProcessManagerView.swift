//
//  ProcessManagerView.swift
//  robdex
//
//  Design surface placeholder generated from the Flutter Agent Runtime UI inventory.
//

import HighlightSwift
import SwiftUI

struct ProcessManagerView: View {
    let currentSessionId: String
    let currentSessionTitle: String
    let processes: [RuntimeProcess]
    let onClose: () -> Void
    let onTerminate: (String) -> Void
    let onTerminateVisible: ([String]) -> Void

    @State private var scope: ProcessManagerScope = .currentSession

    private var activeProcesses: [RuntimeProcess] {
        processes
    }

    private var visibleProcesses: [RuntimeProcess] {
        switch scope {
        case .currentSession:
            return activeProcesses.filter { $0.sessionId == currentSessionId }
        case .allSessions:
            return activeProcesses
        }
    }

    private var visibleProcessIds: [String] {
        visibleProcesses.map(\.id)
    }

    init(
        currentSessionId: String,
        currentSessionTitle: String,
        processes: [RuntimeProcess],
        onClose: @escaping () -> Void = {},
        onTerminate: @escaping (String) -> Void = { _ in },
        onTerminateVisible: @escaping ([String]) -> Void = { _ in }
    ) {
        self.currentSessionId = currentSessionId
        self.currentSessionTitle = currentSessionTitle
        self.processes = processes
        self.onClose = onClose
        self.onTerminate = onTerminate
        self.onTerminateVisible = onTerminateVisible
    }

    var body: some View {
        VStack(spacing: 0) {
            ProcessManagerHeader(
                currentSessionTitle: currentSessionTitle,
                visibleProcesses: visibleProcesses,
                onClose: onClose,
                onTerminateVisible: {
                    onTerminateVisible(visibleProcessIds)
                }
            )

            Divider().opacity(0.6)

            ScrollView {
                VStack(alignment: .leading, spacing: 22) {
                    ProcessManagerScopeSection(
                        scope: $scope,
                        currentSessionTitle: currentSessionTitle
                    )

                    if visibleProcesses.isEmpty {
                        ProcessManagerEmptyState(scope: scope, currentSessionTitle: currentSessionTitle)
                    } else {
                        LazyVStack(alignment: .leading, spacing: 16) {
                            ForEach(visibleProcesses) { process in
                                RuntimeProcessCard(
                                    process: process,
                                    showsSession: scope == .allSessions,
                                    onTerminate: {
                                        onTerminate(process.id)
                                    }
                                )
                            }
                        }
                    }
                }
                .padding(24)
                .frame(maxWidth: 980, alignment: .topLeading)
                .frame(maxWidth: .infinity)
            }
            .background(ProcessManagerBackground())
        }
        .frame(minWidth: 720, minHeight: 680)
    }
}

private struct ProcessManagerHeader: View {
    let currentSessionTitle: String
    let visibleProcesses: [RuntimeProcess]
    let onClose: () -> Void
    let onTerminateVisible: () -> Void

    var body: some View {
        HStack(alignment: .center, spacing: 16) {
            VStack(alignment: .leading, spacing: 4) {
                Text("Processes")
                    .font(.system(size: 22, weight: .semibold, design: .rounded))
                    .foregroundStyle(.primary)

                Text("Async processes and services still running for \(currentSessionTitle).")
                    .font(.system(size: 13))
                    .foregroundStyle(.secondary)
                    .lineLimit(2)
            }

            Spacer()

            Button("Terminate visible") {
                onTerminateVisible()
            }
            .buttonStyle(.bordered)
            .disabled(visibleProcesses.isEmpty)
            .help(visibleProcesses.isEmpty ? "No running processes are visible." : "Terminate every process currently shown.")

            Button("Done") {
                onClose()
            }
            .buttonStyle(.borderedProminent)
        }
        .padding(.horizontal, 24)
        .padding(.vertical, 18)
        .background(.regularMaterial)
    }
}

private struct ProcessManagerScopeSection: View {
    @Binding var scope: ProcessManagerScope
    let currentSessionTitle: String

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Picker("Process scope", selection: $scope) {
                Text("Current session").tag(ProcessManagerScope.currentSession)
                Text("All sessions").tag(ProcessManagerScope.allSessions)
            }
            .pickerStyle(.segmented)
            .frame(maxWidth: 380)

            Text(scope == .currentSession ? "Showing running async work for \(currentSessionTitle)." : "Showing running async work across every session.")
                .font(.system(size: 12))
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
        }
    }
}

private struct RuntimeProcessCard: View {
    let process: RuntimeProcess
    let showsSession: Bool
    let onTerminate: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            HStack(alignment: .top, spacing: 14) {
                ProcessKindIcon(kind: process.kind)

                VStack(alignment: .leading, spacing: 6) {
                    HStack(alignment: .firstTextBaseline, spacing: 10) {
                        Text(process.title)
                            .font(.system(size: 17, weight: .semibold, design: .rounded))
                            .foregroundStyle(.primary)

                        Text(process.kind.title)
                            .font(.system(size: 12, weight: .medium))
                            .foregroundStyle(.secondary)
                    }

                    Text(process.summary)
                        .font(.system(size: 13))
                        .foregroundStyle(.secondary)
                        .fixedSize(horizontal: false, vertical: true)

                    HStack(alignment: .firstTextBaseline, spacing: 12) {
                        if showsSession {
                            ProcessMetadataPill(label: process.sessionTitle, systemImage: "text.bubble")
                        }
                        ProcessMetadataPill(label: process.startedText, systemImage: "clock")
                        ProcessMetadataPill(label: process.state.title, systemImage: process.state.systemImage)
                    }
                }

                Spacer(minLength: 12)

                Button(role: .destructive) {
                    onTerminate()
                } label: {
                    Label("Terminate", systemImage: "xmark.circle")
                }
                .buttonStyle(.bordered)
                .help("Terminate this running process.")
            }

            VStack(alignment: .leading, spacing: 8) {
                ProcessKeyValueRow(label: "Command", value: process.command, monospaced: true)
                ProcessKeyValueRow(label: "Working dir", value: process.workingDirectory, monospaced: true)
                if let serviceURL = process.serviceURL, !serviceURL.isEmpty {
                    ProcessKeyValueRow(label: "Service", value: serviceURL, monospaced: true)
                }
            }

            VStack(alignment: .leading, spacing: 8) {
                Text("Starlark responsible")
                    .font(.system(size: 11, weight: .semibold))
                    .foregroundStyle(.tertiary)
                    .textCase(.uppercase)
                    .tracking(1.0)

                ScrollView(.horizontal, showsIndicators: true) {
                    CodeText(process.starlarkCode)
                        .highlightLanguage(.python)
                        .codeTextColors(.theme(.github))
                        .codeTextStyle(.card(cornerRadius: 12, stroke: .clear, verticalPadding: 12))
                        .font(.system(size: 12))
                        .textSelection(.enabled)
                        .frame(minWidth: 520, alignment: .leading)
                }
            }
        }
        .padding(16)
        .background(Color.primary.opacity(0.035))
        .clipShape(RoundedRectangle(cornerRadius: 16, style: .continuous))
        .overlay {
            RoundedRectangle(cornerRadius: 16, style: .continuous)
                .stroke(Color.primary.opacity(0.09), lineWidth: 1)
        }
    }
}

private struct ProcessKindIcon: View {
    let kind: RuntimeProcessKind

    var body: some View {
        Image(systemName: kind.systemImage)
            .font(.system(size: 17, weight: .semibold))
            .foregroundStyle(kind.color)
            .frame(width: 32, height: 32)
            .background(kind.color.opacity(0.13))
            .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
    }
}

private struct ProcessMetadataPill: View {
    let label: String
    let systemImage: String

    var body: some View {
        Label(label, systemImage: systemImage)
            .font(.system(size: 12, weight: .medium))
            .foregroundStyle(.secondary)
            .lineLimit(1)
    }
}

private struct ProcessKeyValueRow: View {
    let label: String
    let value: String
    var monospaced: Bool = false

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 12) {
            Text(label)
                .font(.system(size: 11, weight: .semibold))
                .foregroundStyle(.tertiary)
                .textCase(.uppercase)
                .tracking(0.7)
                .frame(width: 86, alignment: .leading)

            Text(value)
                .font(.system(size: 12, weight: .medium, design: monospaced ? .monospaced : .default))
                .foregroundStyle(.secondary)
                .lineLimit(1)
                .truncationMode(monospaced ? .middle : .tail)
                .frame(maxWidth: .infinity, alignment: .leading)
        }
    }
}

private struct ProcessManagerEmptyState: View {
    let scope: ProcessManagerScope
    let currentSessionTitle: String

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("No running processes")
                .font(.system(size: 17, weight: .semibold, design: .rounded))
                .foregroundStyle(.primary)

            Text(scope == .currentSession ? "\(currentSessionTitle) has no async processes or services still running." : "No sessions have async processes or services still running.")
                .font(.system(size: 13))
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
        }
        .padding(16)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Color.primary.opacity(0.035))
        .clipShape(RoundedRectangle(cornerRadius: 16, style: .continuous))
        .overlay {
            RoundedRectangle(cornerRadius: 16, style: .continuous)
                .stroke(Color.primary.opacity(0.09), lineWidth: 1)
        }
    }
}

private struct ProcessManagerBackground: View {
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

enum ProcessManagerScope: Hashable {
    case currentSession
    case allSessions
}

struct RuntimeProcess: Identifiable {
    let id: String
    let sessionId: String
    let sessionTitle: String
    let title: String
    let summary: String
    let kind: RuntimeProcessKind
    let state: RuntimeProcessState
    let startedText: String
    let command: String
    let workingDirectory: String
    let serviceURL: String?
    let starlarkCode: String
}

enum RuntimeProcessKind {
    case asyncProcess
    case service

    var title: String {
        switch self {
        case .asyncProcess:
            return "Async process"
        case .service:
            return "Service"
        }
    }

    var systemImage: String {
        switch self {
        case .asyncProcess:
            return "terminal"
        case .service:
            return "server.rack"
        }
    }

    var color: Color {
        switch self {
        case .asyncProcess:
            return Color(red: 0.48, green: 0.68, blue: 1.00)
        case .service:
            return Color.green
        }
    }
}

enum RuntimeProcessState {
    case running
    case starting
    case stopping

    var title: String {
        switch self {
        case .running:
            return "Running"
        case .starting:
            return "Starting"
        case .stopping:
            return "Stopping"
        }
    }

    var systemImage: String {
        switch self {
        case .running:
            return "play.circle"
        case .starting:
            return "clock"
        case .stopping:
            return "pause.circle"
        }
    }
}

#Preview(traits: .landscapeLeft) {
    let processes = [
        RuntimeProcess(
            id: "proc-asset-watch",
            sessionId: "session-release-notes",
            sessionTitle: "Release notes cleanup",
            title: "Asset watcher",
            summary: "Watching generated screenshots while the agent updates the release notes.",
            kind: .asyncProcess,
            state: .running,
            startedText: "18 min ago",
            command: "fswatch frontend/robdex/Assets.xcassets",
            workingDirectory: "/Users/robertsale/.codex/frontend/robdex",
            serviceURL: nil,
            starlarkCode: """
            watcher = cmd["process"].spawn(
                argv=["fswatch", "frontend/robdex/Assets.xcassets"],
                cwd=ctx.workdir,
                async=True,
            )
            ctx.session.remember_process(watcher)
            """
        ),
        RuntimeProcess(
            id: "proc-preview-server",
            sessionId: "session-release-notes",
            sessionTitle: "Release notes cleanup",
            title: "Preview server",
            summary: "Serving a local preview for assets generated by this session.",
            kind: .service,
            state: .running,
            startedText: "11 min ago",
            command: "python3 -m http.server 8042",
            workingDirectory: "/Users/robertsale/.codex/frontend/robdex/PreviewAssets",
            serviceURL: "http://127.0.0.1:8042",
            starlarkCode: """
            preview = cmd["process"].service(
                argv=["python3", "-m", "http.server", "8042"],
                cwd=ctx.workdir + "/PreviewAssets",
                health_url="http://127.0.0.1:8042",
            )
            ctx.session.attach_service(preview)
            """
        ),
        RuntimeProcess(
            id: "proc-indexer",
            sessionId: "session-command-registry",
            sessionTitle: "Command registry review",
            title: "Registry indexer",
            summary: "Indexing command registry changes for another active session.",
            kind: .asyncProcess,
            state: .starting,
            startedText: "2 min ago",
            command: "python3 scripts/index_registry.py --watch",
            workingDirectory: "/Users/robertsale/.codex",
            serviceURL: nil,
            starlarkCode: """
            indexer = cmd["python"].run(
                module="scripts.index_registry",
                args=["--watch"],
                cwd=ctx.project.workdir,
                async=True,
            )
            """
        )
    ]

    ProcessManagerView(
        currentSessionId: "session-release-notes",
        currentSessionTitle: "Release notes cleanup",
        processes: processes
    )
    .frame(width: 960, height: 740)
    .preferredColorScheme(.dark)
}

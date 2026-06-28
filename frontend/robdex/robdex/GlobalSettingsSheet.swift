//
//  GlobalSettingsSheet.swift
//  robdex
//
//  Design surface placeholder generated from the Flutter Agent Runtime UI inventory.
//

import Foundation
import SwiftUI

struct GlobalSettingsSheet: View {
    let connection: GlobalRuntimeConnection
    let localDiscovery: GlobalDiscoveryTarget
    let iCloudProfile: GlobalDiscoveryTarget
    let importedProfile: GlobalDiscoveryTarget
    let projectFilters: [GlobalProjectFilterOption]
    let initialProjectFilterId: String
    let modelOptions: GlobalModelOptionsState
    let diagnostics: [GlobalRuntimeDiagnostic]
    let onDone: () -> Void
    let onDisconnect: () -> Void
    let onReconnect: () -> Void
    let onRehydrate: () -> Void
    let onConnectURL: (String) -> Void
    let onRefreshLocalDiscovery: () -> Void
    let onConnectLocalRuntime: () -> Void
    let onRefreshICloudProfile: () -> Void
    let onConnectICloudRuntime: () -> Void
    let onImportProfile: () -> Void
    let onRefreshImportedProfile: () -> Void
    let onConnectImportedRuntime: () -> Void
    let onSelectProjectFilter: (String) -> Void

    @State private var runtimeURL: String
    @State private var selectedProjectFilterId: String
    @State private var manualURLValidationMessage: String?

    init(
        connection: GlobalRuntimeConnection,
        localDiscovery: GlobalDiscoveryTarget,
        iCloudProfile: GlobalDiscoveryTarget,
        importedProfile: GlobalDiscoveryTarget,
        projectFilters: [GlobalProjectFilterOption],
        initialProjectFilterId: String,
        modelOptions: GlobalModelOptionsState,
        diagnostics: [GlobalRuntimeDiagnostic] = [],
        onDone: @escaping () -> Void = {},
        onDisconnect: @escaping () -> Void = {},
        onReconnect: @escaping () -> Void = {},
        onRehydrate: @escaping () -> Void = {},
        onConnectURL: @escaping (String) -> Void = { _ in },
        onRefreshLocalDiscovery: @escaping () -> Void = {},
        onConnectLocalRuntime: @escaping () -> Void = {},
        onRefreshICloudProfile: @escaping () -> Void = {},
        onConnectICloudRuntime: @escaping () -> Void = {},
        onImportProfile: @escaping () -> Void = {},
        onRefreshImportedProfile: @escaping () -> Void = {},
        onConnectImportedRuntime: @escaping () -> Void = {},
        onSelectProjectFilter: @escaping (String) -> Void = { _ in }
    ) {
        self.connection = connection
        self.localDiscovery = localDiscovery
        self.iCloudProfile = iCloudProfile
        self.importedProfile = importedProfile
        self.projectFilters = projectFilters
        self.initialProjectFilterId = initialProjectFilterId
        self.modelOptions = modelOptions
        self.diagnostics = diagnostics
        self.onDone = onDone
        self.onDisconnect = onDisconnect
        self.onReconnect = onReconnect
        self.onRehydrate = onRehydrate
        self.onConnectURL = onConnectURL
        self.onRefreshLocalDiscovery = onRefreshLocalDiscovery
        self.onConnectLocalRuntime = onConnectLocalRuntime
        self.onRefreshICloudProfile = onRefreshICloudProfile
        self.onConnectICloudRuntime = onConnectICloudRuntime
        self.onImportProfile = onImportProfile
        self.onRefreshImportedProfile = onRefreshImportedProfile
        self.onConnectImportedRuntime = onConnectImportedRuntime
        self.onSelectProjectFilter = onSelectProjectFilter
        _runtimeURL = State(initialValue: connection.baseURL.isEmpty ? "http://127.0.0.1:8765" : connection.baseURL)
        _selectedProjectFilterId = State(initialValue: initialProjectFilterId)
    }

    var body: some View {
        VStack(spacing: 0) {
            GlobalSettingsHeader(onDone: onDone)

            Divider().opacity(0.6)

            ScrollView {
                ViewThatFits(in: .horizontal) {
                    HStack(alignment: .top, spacing: 28) {
                        VStack(alignment: .leading, spacing: 28) {
                            GlobalConnectionStatusSection(
                                connection: connection,
                                onDisconnect: onDisconnect,
                                onReconnect: onReconnect,
                                onRehydrate: onRehydrate
                            )

                            GlobalManualConnectionSection(
                                runtimeURL: $runtimeURL,
                                validationMessage: manualURLValidationMessage,
                                onConnect: connectToManualURL
                            )

                            GlobalWorkspaceScopeSection(
                                selectedProjectFilterId: $selectedProjectFilterId,
                                projectFilters: projectFilters,
                                onSelect: onSelectProjectFilter
                            )
                        }
                        .frame(maxWidth: 560, alignment: .topLeading)

                        VStack(alignment: .leading, spacing: 28) {
                            GlobalDiscoverySection(
                                localDiscovery: localDiscovery,
                                iCloudProfile: iCloudProfile,
                                importedProfile: importedProfile,
                                onRefreshLocalDiscovery: onRefreshLocalDiscovery,
                                onConnectLocalRuntime: onConnectLocalRuntime,
                                onRefreshICloudProfile: onRefreshICloudProfile,
                                onConnectICloudRuntime: onConnectICloudRuntime,
                                onImportProfile: onImportProfile,
                                onRefreshImportedProfile: onRefreshImportedProfile,
                                onConnectImportedRuntime: onConnectImportedRuntime
                            )

                            GlobalModelOptionsSection(modelOptions: modelOptions)

                            GlobalDiagnosticsSection(diagnostics: diagnostics)
                        }
                        .frame(maxWidth: 420, alignment: .topLeading)
                    }

                    VStack(alignment: .leading, spacing: 28) {
                        GlobalConnectionStatusSection(
                            connection: connection,
                            onDisconnect: onDisconnect,
                            onReconnect: onReconnect,
                            onRehydrate: onRehydrate
                        )

                        GlobalManualConnectionSection(
                            runtimeURL: $runtimeURL,
                            validationMessage: manualURLValidationMessage,
                            onConnect: connectToManualURL
                        )

                        GlobalDiscoverySection(
                            localDiscovery: localDiscovery,
                            iCloudProfile: iCloudProfile,
                            importedProfile: importedProfile,
                            onRefreshLocalDiscovery: onRefreshLocalDiscovery,
                            onConnectLocalRuntime: onConnectLocalRuntime,
                            onRefreshICloudProfile: onRefreshICloudProfile,
                            onConnectICloudRuntime: onConnectICloudRuntime,
                            onImportProfile: onImportProfile,
                            onRefreshImportedProfile: onRefreshImportedProfile,
                            onConnectImportedRuntime: onConnectImportedRuntime
                        )

                        GlobalWorkspaceScopeSection(
                            selectedProjectFilterId: $selectedProjectFilterId,
                            projectFilters: projectFilters,
                            onSelect: onSelectProjectFilter
                        )

                        GlobalModelOptionsSection(modelOptions: modelOptions)
                        GlobalDiagnosticsSection(diagnostics: diagnostics)
                    }
                    .frame(maxWidth: 640, alignment: .topLeading)
                }
                .padding(24)
                .frame(maxWidth: 1060)
                .frame(maxWidth: .infinity)
            }
            .background(GlobalSettingsBackground())
        }
        .frame(minWidth: 720, minHeight: 700)
    }

    private func connectToManualURL() {
        let normalizedURL = normalizedRuntimeURL(from: runtimeURL)
        guard let validURL = normalizedURL else {
            manualURLValidationMessage = "Enter an HTTP or HTTPS runtime URL. Host and port shorthand is okay."
            return
        }

        manualURLValidationMessage = nil
        runtimeURL = validURL
        onConnectURL(validURL)
    }

    private func normalizedRuntimeURL(from value: String) -> String? {
        let trimmedValue = value.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedValue.isEmpty else {
            return nil
        }

        let candidate = trimmedValue.contains("://") ? trimmedValue : "http://\(trimmedValue)"
        guard let components = URLComponents(string: candidate),
              let scheme = components.scheme?.lowercased(),
              scheme == "http" || scheme == "https",
              let host = components.host,
              !host.isEmpty else {
            return nil
        }

        return candidate
    }
}

private struct GlobalSettingsHeader: View {
    let onDone: () -> Void

    var body: some View {
        HStack(alignment: .center, spacing: 16) {
            VStack(alignment: .leading, spacing: 4) {
                Text("Global Settings")
                    .font(.system(size: 22, weight: .semibold, design: .rounded))
                    .foregroundStyle(.primary)

                Text("Runtime connection, discovery, and workspace scope.")
                    .font(.system(size: 13))
                    .foregroundStyle(.secondary)
            }

            Spacer()

            Button("Done") {
                onDone()
            }
            .buttonStyle(.borderedProminent)
        }
        .padding(.horizontal, 24)
        .padding(.vertical, 18)
        .background(.regularMaterial)
    }
}

private struct GlobalConnectionStatusSection: View {
    let connection: GlobalRuntimeConnection
    let onDisconnect: () -> Void
    let onReconnect: () -> Void
    let onRehydrate: () -> Void

    private var canDisconnect: Bool {
        connection.state.allowsDisconnect
    }

    private var canReconnect: Bool {
        !connection.baseURL.isEmpty
    }

    private var reconnectReason: String? {
        canReconnect ? nil : "A runtime URL is required before reconnecting."
    }

    var body: some View {
        GlobalSettingsSection(title: "Connection status") {
            VStack(alignment: .leading, spacing: 14) {
                HStack(alignment: .firstTextBaseline, spacing: 10) {
                    Circle()
                        .fill(connection.state.color)
                        .frame(width: 9, height: 9)

                    Text(connection.state.title)
                        .font(.system(size: 18, weight: .semibold, design: .rounded))
                        .foregroundStyle(.primary)

                    Spacer(minLength: 12)

                    Text(connection.state.supportingText)
                        .font(.system(size: 12, weight: .medium))
                        .foregroundStyle(.secondary)
                }

                VStack(alignment: .leading, spacing: 10) {
                    GlobalKeyValueRow(label: "State", value: connection.state.title, placeholder: "Disconnected")
                    GlobalKeyValueRow(label: "Base URL", value: connection.baseURL, placeholder: "No runtime targeted", monospaced: true)
                    GlobalKeyValueRow(label: "Runtime", value: connection.runtimeIdentity ?? "", placeholder: "Identity not loaded")
                    GlobalKeyValueRow(label: "Project filter", value: connection.selectedProjectFilter ?? "", placeholder: "All projects")
                }

                if let lastError = connection.lastError, !lastError.isEmpty {
                    GlobalInlineNotice(title: "Connection error", message: lastError, tone: .critical)
                }

                HStack(spacing: 10) {
                    GlobalActionButton(
                        title: "Disconnect",
                        systemImage: "power",
                        enabled: canDisconnect,
                        disabledReason: "Runtime is not connected.",
                        action: onDisconnect
                    )

                    GlobalActionButton(
                        title: connection.state == .hydrating ? "Hydrating" : "Rehydrate",
                        systemImage: "arrow.triangle.2.circlepath",
                        enabled: canReconnect && connection.state != .hydrating,
                        disabledReason: connection.state == .hydrating ? "Runtime hydration is already in progress." : reconnectReason,
                        action: onRehydrate
                    )

                    GlobalActionButton(
                        title: "Reconnect",
                        systemImage: "antenna.radiowaves.left.and.right",
                        enabled: canReconnect && connection.state != .connecting,
                        disabledReason: connection.state == .connecting ? "Connection is already in progress." : reconnectReason,
                        action: onReconnect
                    )
                }

                if let actionMessage = disabledConnectionActionMessage {
                    Text(actionMessage)
                        .font(.system(size: 12))
                        .foregroundStyle(.tertiary)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
        }
    }

    private var disabledConnectionActionMessage: String? {
        if !canDisconnect && !canReconnect {
            return "Connect to a runtime URL before disconnect, reconnect, or hydrate actions are available."
        }
        if !canDisconnect {
            return "Disconnect is unavailable because no runtime connection is active."
        }
        if connection.state == .hydrating {
            return "Rehydrate is unavailable while runtime hydration is already in progress."
        }
        if connection.state == .connecting {
            return "Reconnect is unavailable while a connection attempt is already in progress."
        }
        return nil
    }
}

private struct GlobalManualConnectionSection: View {
    @Binding var runtimeURL: String
    let validationMessage: String?
    let onConnect: () -> Void

    var body: some View {
        GlobalSettingsSection(title: "Manual connection") {
            VStack(alignment: .leading, spacing: 12) {
                VStack(alignment: .leading, spacing: 7) {
                    Text("Runtime URL")
                        .font(.system(size: 13, weight: .semibold))
                        .foregroundStyle(.primary)

                    HStack(spacing: 8) {
                        TextField("http://127.0.0.1:8765", text: $runtimeURL)
                            .textFieldStyle(.plain)
                            .font(.system(size: 14, design: .monospaced))
                            .padding(.horizontal, 11)
                            .padding(.vertical, 10)
                            .background(Color.primary.opacity(0.045))
                            .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
                            .overlay {
                                RoundedRectangle(cornerRadius: 10, style: .continuous)
                                    .stroke(Color.primary.opacity(0.10), lineWidth: 1)
                            }

                        Button("Connect to URL") {
                            onConnect()
                        }
                        .buttonStyle(.borderedProminent)
                    }

                    Text("Use a full HTTP URL or host:port shorthand for a reachable Agent Runtime.")
                        .font(.system(size: 12))
                        .foregroundStyle(.secondary)
                        .fixedSize(horizontal: false, vertical: true)
                }

                if let validationMessage {
                    GlobalInlineNotice(title: "Cannot connect yet", message: validationMessage, tone: .warning)
                }
            }
        }
    }
}

private struct GlobalDiscoverySection: View {
    let localDiscovery: GlobalDiscoveryTarget
    let iCloudProfile: GlobalDiscoveryTarget
    let importedProfile: GlobalDiscoveryTarget
    let onRefreshLocalDiscovery: () -> Void
    let onConnectLocalRuntime: () -> Void
    let onRefreshICloudProfile: () -> Void
    let onConnectICloudRuntime: () -> Void
    let onImportProfile: () -> Void
    let onRefreshImportedProfile: () -> Void
    let onConnectImportedRuntime: () -> Void

    var body: some View {
        GlobalSettingsSection(title: "Discovery") {
            VStack(alignment: .leading, spacing: 14) {
                GlobalDiscoveryTargetRow(
                    target: localDiscovery,
                    refreshTitle: "Refresh local discovery",
                    connectTitle: "Connect local runtime",
                    importTitle: nil,
                    onImport: nil,
                    onRefresh: onRefreshLocalDiscovery,
                    onConnect: onConnectLocalRuntime
                )

                GlobalDiscoveryTargetRow(
                    target: iCloudProfile,
                    refreshTitle: "Refresh iCloud profile",
                    connectTitle: "Connect iCloud runtime",
                    importTitle: nil,
                    onImport: nil,
                    onRefresh: onRefreshICloudProfile,
                    onConnect: onConnectICloudRuntime
                )

                GlobalDiscoveryTargetRow(
                    target: importedProfile,
                    refreshTitle: "Refresh imported profile",
                    connectTitle: "Connect imported runtime",
                    importTitle: "Import profile…",
                    onImport: onImportProfile,
                    onRefresh: onRefreshImportedProfile,
                    onConnect: onConnectImportedRuntime
                )
            }
        }
    }
}

private struct GlobalDiscoveryTargetRow: View {
    let target: GlobalDiscoveryTarget
    let refreshTitle: String
    let connectTitle: String
    let importTitle: String?
    let onImport: (() -> Void)?
    let onRefresh: () -> Void
    let onConnect: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            VStack(alignment: .leading, spacing: 4) {
                HStack(alignment: .firstTextBaseline, spacing: 8) {
                    Text(target.title)
                        .font(.system(size: 15, weight: .semibold, design: .rounded))
                        .foregroundStyle(.primary)

                    Spacer(minLength: 12)

                    Text(target.stateText)
                        .font(.system(size: 12, weight: .medium))
                        .foregroundStyle(target.isConnectable ? Color.green : .secondary)
                }

                Text(target.sourceDescription)
                    .font(.system(size: 12))
                    .foregroundStyle(.secondary)

                if let path = target.profilePath, !path.isEmpty {
                    Text(path)
                        .font(.system(size: 11, design: .monospaced))
                        .foregroundStyle(.tertiary)
                        .lineLimit(1)
                        .truncationMode(.middle)
                }
            }

            if !target.message.isEmpty {
                Text(target.message)
                    .font(.system(size: 13))
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }

            VStack(alignment: .leading, spacing: 8) {
                GlobalKeyValueRow(label: "Base", value: target.baseURL ?? "", placeholder: "Not discovered", monospaced: true)
                GlobalKeyValueRow(label: "Health", value: target.healthURL ?? "", placeholder: "Not discovered", monospaced: true)
                GlobalKeyValueRow(label: "Stream", value: target.webSocketURL ?? "", placeholder: "Not discovered", monospaced: true)
                GlobalKeyValueRow(label: "Runtime", value: target.runtimeIdentity ?? "", placeholder: "Identity not loaded")
                if let timestamp = target.lastUpdatedText, !timestamp.isEmpty {
                    GlobalKeyValueRow(label: "Updated", value: timestamp, placeholder: "Not refreshed")
                }
            }

            if !target.diagnostics.isEmpty {
                VStack(alignment: .leading, spacing: 6) {
                    ForEach(target.diagnostics, id: \.self) { diagnostic in
                        HStack(alignment: .firstTextBaseline, spacing: 7) {
                            Image(systemName: "exclamationmark.triangle")
                                .font(.system(size: 11, weight: .semibold))
                                .foregroundStyle(Color(red: 0.98, green: 0.67, blue: 0.25))

                            Text(diagnostic)
                                .font(.system(size: 12))
                                .foregroundStyle(.secondary)
                                .fixedSize(horizontal: false, vertical: true)
                        }
                    }
                }
            }

            HStack(spacing: 10) {
                if let importTitle, let onImport {
                    Button(importTitle) {
                        onImport()
                    }
                    .buttonStyle(.bordered)
                }

                Button(refreshTitle) {
                    onRefresh()
                }
                .buttonStyle(.bordered)

                GlobalActionButton(
                    title: connectTitle,
                    systemImage: "bolt.horizontal.circle",
                    enabled: target.isConnectable,
                    disabledReason: target.disabledConnectReason ?? "A discovered base URL is required before connecting.",
                    action: onConnect
                )
            }

            if !target.isConnectable {
                Text(target.disabledConnectReason ?? "A discovered base URL is required before connecting.")
                    .font(.system(size: 12))
                    .foregroundStyle(.tertiary)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
        .padding(14)
        .background(Color.primary.opacity(0.035))
        .clipShape(RoundedRectangle(cornerRadius: 14, style: .continuous))
        .overlay {
            RoundedRectangle(cornerRadius: 14, style: .continuous)
                .stroke(Color.primary.opacity(0.09), lineWidth: 1)
        }
    }
}

private struct GlobalWorkspaceScopeSection: View {
    @Binding var selectedProjectFilterId: String
    let projectFilters: [GlobalProjectFilterOption]
    let onSelect: (String) -> Void

    var body: some View {
        GlobalSettingsSection(title: "Workspace scope") {
            VStack(alignment: .leading, spacing: 12) {
                VStack(alignment: .leading, spacing: 7) {
                    Text("Project filter")
                        .font(.system(size: 13, weight: .semibold))
                        .foregroundStyle(.primary)

                    Picker("Project filter", selection: $selectedProjectFilterId) {
                        ForEach(projectFilters) { option in
                            Text(option.label).tag(option.id)
                        }
                    }
                    .labelsHidden()
                    .pickerStyle(.menu)
                    .onChange(of: selectedProjectFilterId) { _, newValue in
                        onSelect(newValue)
                    }

                    Text("Filters visible sessions and project-scoped runtime surfaces in this shell.")
                        .font(.system(size: 12))
                        .foregroundStyle(.secondary)
                        .fixedSize(horizontal: false, vertical: true)
                }

                if let selected = projectFilters.first(where: { $0.id == selectedProjectFilterId }) {
                    VStack(alignment: .leading, spacing: 6) {
                        GlobalKeyValueRow(label: "Scope", value: selected.summary, placeholder: "All projects")
                        if let projectKey = selected.projectKey, !projectKey.isEmpty {
                            GlobalKeyValueRow(label: "Project key", value: projectKey, placeholder: "No project key", monospaced: true)
                        }
                    }
                }
            }
        }
    }
}

private struct GlobalModelOptionsSection: View {
    let modelOptions: GlobalModelOptionsState

    var body: some View {
        GlobalSettingsSection(title: "Model options") {
            VStack(alignment: .leading, spacing: 12) {
                switch modelOptions.availability {
                case .loaded:
                    if modelOptions.models.isEmpty {
                        GlobalInlineNotice(title: "No models reported", message: "The runtime is reachable but did not return model options.", tone: .warning)
                    } else {
                        VStack(alignment: .leading, spacing: 8) {
                            ForEach(modelOptions.models) { model in
                                HStack(alignment: .firstTextBaseline, spacing: 10) {
                                    Text(model.label)
                                        .font(.system(size: 13, weight: .medium))
                                        .foregroundStyle(.primary)

                                    if model.isDefault {
                                        Text("default")
                                            .font(.system(size: 11, weight: .semibold))
                                            .foregroundStyle(.secondary)
                                    }

                                    Spacer(minLength: 12)

                                    Text(model.provider)
                                        .font(.system(size: 12))
                                        .foregroundStyle(.secondary)
                                        .lineLimit(1)
                                }
                                .padding(.vertical, 2)
                            }
                        }
                    }
                case .loading:
                    GlobalInlineNotice(title: "Loading models", message: "Runtime model options are being hydrated.", tone: .neutral)
                case .unavailable(let reason):
                    GlobalInlineNotice(title: "Models unavailable", message: reason, tone: .warning)
                }
            }
        }
    }
}

private struct GlobalDiagnosticsSection: View {
    let diagnostics: [GlobalRuntimeDiagnostic]

    var body: some View {
        GlobalSettingsSection(title: "Diagnostics") {
            if diagnostics.isEmpty {
                Text("No connection or discovery issues reported.")
                    .font(.system(size: 13))
                    .foregroundStyle(.secondary)
            } else {
                VStack(alignment: .leading, spacing: 10) {
                    ForEach(diagnostics) { diagnostic in
                        HStack(alignment: .top, spacing: 9) {
                            Image(systemName: diagnostic.tone.systemImage)
                                .font(.system(size: 13, weight: .semibold))
                                .foregroundStyle(diagnostic.tone.color)
                                .frame(width: 16)

                            VStack(alignment: .leading, spacing: 3) {
                                Text(diagnostic.title)
                                    .font(.system(size: 13, weight: .semibold))
                                    .foregroundStyle(.primary)

                                Text(diagnostic.message)
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
}

private struct GlobalSettingsSection<Content: View>: View {
    let title: String
    let content: Content

    init(title: String, @ViewBuilder content: () -> Content) {
        self.title = title
        self.content = content()
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text(title)
                .font(.system(size: 11, weight: .semibold))
                .foregroundStyle(.tertiary)
                .textCase(.uppercase)
                .tracking(1.0)

            VStack(alignment: .leading, spacing: 14) {
                content
            }
        }
    }
}

private struct GlobalKeyValueRow: View {
    let label: String
    let value: String
    let placeholder: String
    var monospaced: Bool = false

    private var displayValue: String {
        value.isEmpty ? placeholder : value
    }

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 12) {
            Text(label)
                .font(.system(size: 11, weight: .semibold))
                .foregroundStyle(.tertiary)
                .textCase(.uppercase)
                .tracking(0.7)
                .frame(width: 78, alignment: .leading)

            Text(displayValue)
                .font(.system(size: 12, weight: value.isEmpty ? .regular : .medium, design: monospaced ? .monospaced : .default))
                .foregroundStyle(value.isEmpty ? .tertiary : .secondary)
                .lineLimit(1)
                .truncationMode(monospaced ? .middle : .tail)
                .frame(maxWidth: .infinity, alignment: .leading)
        }
    }
}

private struct GlobalInlineNotice: View {
    let title: String
    let message: String
    let tone: GlobalNoticeTone

    var body: some View {
        HStack(alignment: .top, spacing: 9) {
            Image(systemName: tone.systemImage)
                .font(.system(size: 13, weight: .semibold))
                .foregroundStyle(tone.color)
                .frame(width: 16)

            VStack(alignment: .leading, spacing: 3) {
                Text(title)
                    .font(.system(size: 13, weight: .semibold))
                    .foregroundStyle(.primary)

                Text(message)
                    .font(.system(size: 12))
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
        .padding(12)
        .background(tone.backgroundColor)
        .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
    }
}

private struct GlobalActionButton: View {
    let title: String
    let systemImage: String
    let enabled: Bool
    let disabledReason: String?
    let action: () -> Void

    var body: some View {
        Button {
            action()
        } label: {
            Label(title, systemImage: systemImage)
        }
        .buttonStyle(.bordered)
        .disabled(!enabled)
        .help(enabled ? title : (disabledReason ?? "Unavailable"))
    }
}

private struct GlobalSettingsBackground: View {
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

struct GlobalRuntimeConnection {
    let state: GlobalRuntimeConnectionState
    let baseURL: String
    let runtimeIdentity: String?
    let selectedProjectFilter: String?
    let lastError: String?
}

enum GlobalRuntimeConnectionState: Equatable {
    case disconnected
    case connecting
    case hydrating
    case streaming
    case reconnecting
    case error

    var title: String {
        switch self {
        case .disconnected:
            return "Disconnected"
        case .connecting:
            return "Connecting"
        case .hydrating:
            return "Hydrating"
        case .streaming:
            return "Streaming"
        case .reconnecting:
            return "Reconnecting"
        case .error:
            return "Connection error"
        }
    }

    var supportingText: String {
        switch self {
        case .disconnected:
            return "No active runtime"
        case .connecting:
            return "Opening connection"
        case .hydrating:
            return "Loading runtime state"
        case .streaming:
            return "Streaming ready"
        case .reconnecting:
            return "Recovering connection"
        case .error:
            return "Needs attention"
        }
    }

    var color: Color {
        switch self {
        case .disconnected:
            return .secondary
        case .connecting, .hydrating, .reconnecting:
            return Color(red: 0.98, green: 0.67, blue: 0.25)
        case .streaming:
            return Color.green
        case .error:
            return Color(red: 1.00, green: 0.42, blue: 0.38)
        }
    }

    var allowsDisconnect: Bool {
        switch self {
        case .connecting, .hydrating, .streaming, .reconnecting, .error:
            return true
        case .disconnected:
            return false
        }
    }
}

struct GlobalDiscoveryTarget {
    let title: String
    let sourceDescription: String
    let profilePath: String?
    let stateText: String
    let message: String
    let baseURL: String?
    let healthURL: String?
    let webSocketURL: String?
    let runtimeIdentity: String?
    let lastUpdatedText: String?
    let diagnostics: [String]
    let isConnectable: Bool
    let disabledConnectReason: String?
}

struct GlobalProjectFilterOption: Identifiable {
    let id: String
    let label: String
    let projectKey: String?
    let summary: String
}

struct GlobalModelOptionsState {
    let availability: GlobalModelAvailability
    let models: [GlobalRuntimeModelOption]
}

enum GlobalModelAvailability {
    case loaded
    case loading
    case unavailable(String)
}

struct GlobalRuntimeModelOption: Identifiable {
    let id: String
    let label: String
    let provider: String
    let isDefault: Bool
}

struct GlobalRuntimeDiagnostic: Identifiable {
    let id: String
    let title: String
    let message: String
    let tone: GlobalNoticeTone
}

enum GlobalNoticeTone {
    case neutral
    case warning
    case critical

    var systemImage: String {
        switch self {
        case .neutral:
            return "info.circle"
        case .warning:
            return "exclamationmark.triangle"
        case .critical:
            return "xmark.octagon"
        }
    }

    var color: Color {
        switch self {
        case .neutral:
            return Color(red: 0.48, green: 0.68, blue: 1.00)
        case .warning:
            return Color(red: 0.98, green: 0.67, blue: 0.25)
        case .critical:
            return Color(red: 1.00, green: 0.42, blue: 0.38)
        }
    }

    var backgroundColor: Color {
        switch self {
        case .neutral:
            return Color(red: 0.055, green: 0.080, blue: 0.130)
        case .warning:
            return Color(red: 0.130, green: 0.095, blue: 0.050)
        case .critical:
            return Color(red: 0.130, green: 0.055, blue: 0.055)
        }
    }
}

#Preview(traits: .landscapeLeft) {
    let connection = GlobalRuntimeConnection(
        state: .streaming,
        baseURL: "http://127.0.0.1:8765",
        runtimeIdentity: "robdex-agent-runtime/0.1.0",
        selectedProjectFilter: "Codex Config",
        lastError: nil
    )

    let localDiscovery = GlobalDiscoveryTarget(
        title: "Local discovery",
        sourceDescription: "Source: local service discovery file",
        profilePath: "/Users/robertsale/.codex/agent-runtime.discovery.json",
        stateText: "Ready",
        message: "A local runtime profile is available on this Mac.",
        baseURL: "http://127.0.0.1:8765",
        healthURL: "http://127.0.0.1:8765/health",
        webSocketURL: "ws://127.0.0.1:8765/stream",
        runtimeIdentity: "robdex-agent-runtime/0.1.0",
        lastUpdatedText: "Today, 10:42 AM",
        diagnostics: [],
        isConnectable: true,
        disabledConnectReason: nil
    )

    let iCloudProfile = GlobalDiscoveryTarget(
        title: "iCloud remote profile",
        sourceDescription: "Source: iCloud synced remote profile",
        profilePath: "/Users/robertsale/Library/Mobile Documents/com~apple~CloudDocs/Robdex/runtime.profile.json",
        stateText: "Stale",
        message: "The synced profile exists, but it has not been refreshed today.",
        baseURL: "https://agent-runtime.example.dev",
        healthURL: "https://agent-runtime.example.dev/health",
        webSocketURL: "wss://agent-runtime.example.dev/stream",
        runtimeIdentity: nil,
        lastUpdatedText: "Yesterday, 7:18 PM",
        diagnostics: ["Health check has not completed for this profile."],
        isConnectable: true,
        disabledConnectReason: nil
    )

    let importedProfile = GlobalDiscoveryTarget(
        title: "Imported remote profile",
        sourceDescription: "Source: user-imported remote profile copy",
        profilePath: "/Users/robertsale/.codex/remote-profiles/staging-runtime.json",
        stateText: "Invalid",
        message: "The imported profile is missing a usable runtime URL.",
        baseURL: nil,
        healthURL: nil,
        webSocketURL: nil,
        runtimeIdentity: nil,
        lastUpdatedText: "Jun 24, 2026",
        diagnostics: ["Remote profile does not include a base URL."],
        isConnectable: false,
        disabledConnectReason: "Import or refresh a profile with a runtime URL before connecting."
    )

    let projectFilters = [
        GlobalProjectFilterOption(id: "all", label: "All projects", projectKey: nil, summary: "All runtime projects and unassigned sessions"),
        GlobalProjectFilterOption(id: "unassigned", label: "Unassigned", projectKey: nil, summary: "Sessions without a project"),
        GlobalProjectFilterOption(id: "codex-config", label: "Codex Config", projectKey: "codex-config", summary: "Only Codex Config sessions and project-scoped surfaces"),
        GlobalProjectFilterOption(id: "ezra", label: "Ezra", projectKey: "ezra", summary: "Only Ezra sessions and project-scoped surfaces")
    ]

    let modelOptions = GlobalModelOptionsState(
        availability: .loaded,
        models: [
            GlobalRuntimeModelOption(id: "gpt-5.4", label: "GPT-5.4", provider: "OpenAI", isDefault: true),
            GlobalRuntimeModelOption(id: "gpt-5.4-mini", label: "GPT-5.4 mini", provider: "OpenAI", isDefault: false),
            GlobalRuntimeModelOption(id: "local-runtime", label: "Local runtime default", provider: "Runtime", isDefault: false)
        ]
    )

    let diagnostics = [
        GlobalRuntimeDiagnostic(
            id: "icloud-health",
            title: "iCloud health pending",
            message: "Refresh the iCloud profile before using it as the active runtime.",
            tone: .warning
        ),
        GlobalRuntimeDiagnostic(
            id: "imported-profile-invalid",
            title: "Imported profile invalid",
            message: "Import a new remote profile or refresh the existing copy.",
            tone: .critical
        )
    ]

    GlobalSettingsSheet(
        connection: connection,
        localDiscovery: localDiscovery,
        iCloudProfile: iCloudProfile,
        importedProfile: importedProfile,
        projectFilters: projectFilters,
        initialProjectFilterId: "codex-config",
        modelOptions: modelOptions,
        diagnostics: diagnostics
    )
    .frame(width: 980, height: 760)
    .preferredColorScheme(.dark)
}

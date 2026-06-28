//
//  ProjectSettingsSheet.swift
//  robdex
//
//  Design surface placeholder generated from the Flutter Agent Runtime UI inventory.
//

import SwiftUI

#if os(macOS)
import AppKit
#endif

struct ProjectSettingsSheet: View {
    let project: ProjectSettingsProfile
    let roleOptions: [ProjectSettingsRoleOption]
    let modelOptions: [ProjectSettingsModelOption]
    let runtimeError: ProjectSettingsRuntimeError?
    let onCancel: () -> Void
    let onSave: (ProjectSettingsDraft) -> Void

    @State private var displayName: String
    @State private var defaultWorkdir: String
    @State private var defaultWorktreeRoot: String
    @State private var defaultRoleId: String
    @State private var defaultModelId: String
    @State private var submitted = false

    private var draft: ProjectSettingsDraft {
        ProjectSettingsDraft(
            projectKey: project.projectKey,
            displayName: displayName.trimmingCharacters(in: .whitespacesAndNewlines),
            defaultWorkdir: defaultWorkdir.trimmingCharacters(in: .whitespacesAndNewlines),
            defaultWorktreeRoot: defaultWorktreeRoot.trimmingCharacters(in: .whitespacesAndNewlines),
            defaultRoleId: defaultRoleId,
            defaultModel: defaultModelId
        )
    }

    private var originalDraft: ProjectSettingsDraft {
        ProjectSettingsDraft(
            projectKey: project.projectKey,
            displayName: project.displayName,
            defaultWorkdir: project.defaultWorkdir,
            defaultWorktreeRoot: project.defaultWorktreeRoot,
            defaultRoleId: project.defaultRoleId,
            defaultModel: project.defaultModel
        )
    }

    private var hasChanges: Bool {
        draft != originalDraft
    }

    private var validationMessages: [String] {
        var messages: [String] = []
        if draft.displayName.isEmpty {
            messages.append("Display name is required.")
        }
        if draft.defaultWorkdir.isEmpty {
            messages.append("Default workdir is required.")
        }
        if draft.defaultWorktreeRoot.isEmpty {
            messages.append("Default worktree root is required.")
        }
        if draft.defaultModel.isEmpty {
            messages.append("Default model is required.")
        }
        return messages
    }

    private var canSave: Bool {
        hasChanges && validationMessages.isEmpty
    }

    init(
        project: ProjectSettingsProfile,
        roleOptions: [ProjectSettingsRoleOption],
        modelOptions: [ProjectSettingsModelOption],
        runtimeError: ProjectSettingsRuntimeError? = nil,
        onCancel: @escaping () -> Void = {},
        onSave: @escaping (ProjectSettingsDraft) -> Void = { _ in }
    ) {
        self.project = project
        self.roleOptions = roleOptions
        self.modelOptions = modelOptions
        self.runtimeError = runtimeError
        self.onCancel = onCancel
        self.onSave = onSave
        _displayName = State(initialValue: project.displayName)
        _defaultWorkdir = State(initialValue: project.defaultWorkdir)
        _defaultWorktreeRoot = State(initialValue: project.defaultWorktreeRoot)
        _defaultRoleId = State(initialValue: project.defaultRoleId)
        _defaultModelId = State(initialValue: project.defaultModel)
    }

    var body: some View {
        VStack(spacing: 0) {
            ProjectSettingsHeader(
                canSave: canSave,
                hasChanges: hasChanges,
                onCancel: onCancel,
                onSave: submit
            )

            Divider().opacity(0.6)

            ScrollView {
                ViewThatFits(in: .horizontal) {
                    HStack(alignment: .top, spacing: 28) {
                        ProjectSettingsForm(
                            projectKey: project.projectKey,
                            displayName: $displayName,
                            defaultWorkdir: $defaultWorkdir,
                            defaultWorktreeRoot: $defaultWorktreeRoot,
                            defaultRoleId: $defaultRoleId,
                            defaultModelId: $defaultModelId,
                            roleOptions: roleOptions,
                            modelOptions: modelOptions,
                            validationMessages: submitted || hasChanges ? validationMessages : [],
                            runtimeError: runtimeError,
                            onUseWorkdirForWorktreeRoot: {
                                defaultWorktreeRoot = defaultWorkdir
                            }
                        )
                        .frame(maxWidth: 580, alignment: .topLeading)

                        ProjectSettingsSummary(
                            project: project,
                            draft: draft,
                            roleLabel: roleLabel(for: draft.defaultRoleId),
                            modelLabel: modelLabel(for: draft.defaultModel),
                            hasChanges: hasChanges
                        )
                        .frame(width: 330, alignment: .topLeading)
                    }

                    VStack(alignment: .leading, spacing: 24) {
                        ProjectSettingsForm(
                            projectKey: project.projectKey,
                            displayName: $displayName,
                            defaultWorkdir: $defaultWorkdir,
                            defaultWorktreeRoot: $defaultWorktreeRoot,
                            defaultRoleId: $defaultRoleId,
                            defaultModelId: $defaultModelId,
                            roleOptions: roleOptions,
                            modelOptions: modelOptions,
                            validationMessages: submitted || hasChanges ? validationMessages : [],
                            runtimeError: runtimeError,
                            onUseWorkdirForWorktreeRoot: {
                                defaultWorktreeRoot = defaultWorkdir
                            }
                        )

                        ProjectSettingsSummary(
                            project: project,
                            draft: draft,
                            roleLabel: roleLabel(for: draft.defaultRoleId),
                            modelLabel: modelLabel(for: draft.defaultModel),
                            hasChanges: hasChanges
                        )
                    }
                    .frame(maxWidth: 640, alignment: .topLeading)
                }
                .padding(24)
                .frame(maxWidth: 1020)
                .frame(maxWidth: .infinity)
            }
            .background(ProjectSettingsBackground())
        }
        .frame(minWidth: 700, minHeight: 660)
    }

    private func submit() {
        submitted = true
        guard canSave else {
            return
        }
        onSave(draft)
    }

    private func roleLabel(for id: String) -> String {
        if id.isEmpty {
            return "No default role"
        }
        return roleOptions.first(where: { $0.id == id })?.label ?? id
    }

    private func modelLabel(for id: String) -> String {
        modelOptions.first(where: { $0.id == id })?.label ?? "Not selected"
    }
}

private struct ProjectSettingsHeader: View {
    let canSave: Bool
    let hasChanges: Bool
    let onCancel: () -> Void
    let onSave: () -> Void

    var body: some View {
        HStack(alignment: .center, spacing: 16) {
            VStack(alignment: .leading, spacing: 4) {
                Text("Project Settings")
                    .font(.system(size: 22, weight: .semibold, design: .rounded))
                    .foregroundStyle(.primary)

                Text("Project identity, workspace defaults, and runtime defaults for new sessions.")
                    .font(.system(size: 13))
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }

            Spacer()

            if hasChanges {
                Text("Unsaved changes")
                    .font(.system(size: 12, weight: .medium))
                    .foregroundStyle(.secondary)
            }

            Button("Cancel") {
                onCancel()
            }
            .buttonStyle(.bordered)

            Button("Save") {
                onSave()
            }
            .buttonStyle(.borderedProminent)
            .disabled(!canSave)
            .help(canSave ? "Save project settings" : "Make a valid change before saving.")
        }
        .padding(.horizontal, 24)
        .padding(.vertical, 18)
        .background(.regularMaterial)
    }
}

private struct ProjectSettingsForm: View {
    let projectKey: String
    @Binding var displayName: String
    @Binding var defaultWorkdir: String
    @Binding var defaultWorktreeRoot: String
    @Binding var defaultRoleId: String
    @Binding var defaultModelId: String
    let roleOptions: [ProjectSettingsRoleOption]
    let modelOptions: [ProjectSettingsModelOption]
    let validationMessages: [String]
    let runtimeError: ProjectSettingsRuntimeError?
    let onUseWorkdirForWorktreeRoot: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 24) {
            ProjectSettingsSection(title: "Identity") {
                ProjectSettingsTextField(
                    title: "Display name",
                    text: $displayName,
                    prompt: "Codex Config",
                    help: "Human-readable project label shown in the runtime shell."
                )

                ProjectSettingsReadOnlyField(
                    title: "Project key",
                    value: projectKey,
                    help: "Stable runtime project key. It is not changed from this sheet."
                )
            }

            ProjectSettingsSection(title: "Workspace defaults") {
                ProjectSettingsPathField(
                    title: "Default workdir",
                    text: $defaultWorkdir,
                    prompt: "/Users/you/Code/my-ios-app",
                    help: "Default working directory for new sessions in this project."
                )

                ProjectSettingsPathField(
                    title: "Default worktree root",
                    text: $defaultWorktreeRoot,
                    prompt: "/Users/you/.codex/.worktrees/my-ios-app",
                    help: "Where new session worktree roots should be created for this project.",
                    copySourceTitle: "Use workdir",
                    onCopySource: onUseWorkdirForWorktreeRoot
                )
            }

            ProjectSettingsSection(title: "Runtime defaults") {
                VStack(alignment: .leading, spacing: 7) {
                    Text("Default role")
                        .font(.system(size: 13, weight: .semibold))
                        .foregroundStyle(.primary)

                    Picker("Default role", selection: $defaultRoleId) {
                        Text("No default role").tag("")
                        ForEach(roleOptions) { role in
                            Text(role.label).tag(role.id)
                        }
                    }
                    .labelsHidden()
                    .pickerStyle(.menu)

                    Text("Applied to new sessions unless a session chooses a different role.")
                        .font(.system(size: 12))
                        .foregroundStyle(.secondary)
                }

                VStack(alignment: .leading, spacing: 7) {
                    Text("Default model")
                        .font(.system(size: 13, weight: .semibold))
                        .foregroundStyle(.primary)

                    Picker("Default model", selection: $defaultModelId) {
                        ForEach(modelOptions) { model in
                            Text(model.label).tag(model.id)
                        }
                    }
                    .labelsHidden()
                    .pickerStyle(.menu)

                    Text("Model used for new sessions in this project unless overridden.")
                        .font(.system(size: 12))
                        .foregroundStyle(.secondary)
                }
            }

            if !validationMessages.isEmpty {
                ProjectSettingsValidationNotice(messages: validationMessages)
            }

            if let runtimeError {
                ProjectSettingsRuntimeErrorNotice(error: runtimeError)
            }
        }
    }
}

private struct ProjectSettingsSection<Content: View>: View {
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

            VStack(alignment: .leading, spacing: 16) {
                content
            }
        }
    }
}

private struct ProjectSettingsTextField: View {
    let title: String
    @Binding var text: String
    let prompt: String
    let help: String

    var body: some View {
        VStack(alignment: .leading, spacing: 7) {
            Text(title)
                .font(.system(size: 13, weight: .semibold))
                .foregroundStyle(.primary)

            TextField(prompt, text: $text)
                .textFieldStyle(.plain)
                .font(.system(size: 14))
                .padding(.horizontal, 11)
                .padding(.vertical, 10)
                .background(Color.primary.opacity(0.045))
                .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
                .overlay {
                    RoundedRectangle(cornerRadius: 10, style: .continuous)
                        .stroke(Color.primary.opacity(0.10), lineWidth: 1)
                }

            Text(help)
                .font(.system(size: 12))
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
        }
    }
}

private struct ProjectSettingsReadOnlyField: View {
    let title: String
    let value: String
    let help: String

    var body: some View {
        VStack(alignment: .leading, spacing: 7) {
            Text(title)
                .font(.system(size: 13, weight: .semibold))
                .foregroundStyle(.primary)

            Text(value)
                .font(.system(size: 14, design: .monospaced))
                .foregroundStyle(.secondary)
                .padding(.horizontal, 11)
                .padding(.vertical, 10)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(Color.primary.opacity(0.035))
                .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
                .overlay {
                    RoundedRectangle(cornerRadius: 10, style: .continuous)
                        .stroke(Color.primary.opacity(0.08), lineWidth: 1)
                }

            Text(help)
                .font(.system(size: 12))
                .foregroundStyle(.secondary)
        }
    }
}

private struct ProjectSettingsPathField: View {
    let title: String
    @Binding var text: String
    let prompt: String
    let help: String
    var copySourceTitle: String?
    var onCopySource: (() -> Void)?

    var body: some View {
        VStack(alignment: .leading, spacing: 7) {
            Text(title)
                .font(.system(size: 13, weight: .semibold))
                .foregroundStyle(.primary)

            HStack(spacing: 8) {
                TextField(prompt, text: $text)
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

                #if os(macOS)
                Button {
                    chooseDirectory()
                } label: {
                    Image(systemName: "folder")
                }
                .buttonStyle(.bordered)
                .help("Choose folder")
                #endif

                if let copySourceTitle, let onCopySource {
                    Button {
                        onCopySource()
                    } label: {
                        Image(systemName: "doc.on.clipboard")
                    }
                    .buttonStyle(.bordered)
                    .help(copySourceTitle)
                }
            }

            Text(help)
                .font(.system(size: 12))
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
        }
    }

    #if os(macOS)
    private func chooseDirectory() {
        let panel = NSOpenPanel()
        panel.canChooseFiles = false
        panel.canChooseDirectories = true
        panel.allowsMultipleSelection = false
        panel.canCreateDirectories = true
        panel.prompt = "Choose"

        if panel.runModal() == .OK, let url = panel.url {
            text = url.path
        }
    }
    #endif
}

private struct ProjectSettingsValidationNotice: View {
    let messages: [String]

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Before saving")
                .font(.system(size: 13, weight: .semibold))
                .foregroundStyle(Color(red: 0.98, green: 0.67, blue: 0.25))

            ForEach(messages, id: \.self) { message in
                HStack(alignment: .firstTextBaseline, spacing: 8) {
                    Image(systemName: "circle.fill")
                        .font(.system(size: 5, weight: .bold))
                        .foregroundStyle(Color(red: 0.98, green: 0.67, blue: 0.25))

                    Text(message)
                        .font(.system(size: 13))
                        .foregroundStyle(.secondary)
                }
            }
        }
        .padding(12)
        .background(Color(red: 0.130, green: 0.095, blue: 0.050))
        .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
    }
}

private struct ProjectSettingsRuntimeErrorNotice: View {
    let error: ProjectSettingsRuntimeError

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text("Runtime error")
                .font(.system(size: 13, weight: .semibold))
                .foregroundStyle(Color(red: 1.00, green: 0.42, blue: 0.38))

            Text(error.message)
                .font(.system(size: 13))
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)

            if let recovery = error.recovery {
                Text(recovery)
                    .font(.system(size: 12, weight: .medium))
                    .foregroundStyle(.secondary)
            }
        }
        .padding(12)
        .background(Color(red: 0.130, green: 0.055, blue: 0.055))
        .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
    }
}

private struct ProjectSettingsSummary: View {
    let project: ProjectSettingsProfile
    let draft: ProjectSettingsDraft
    let roleLabel: String
    let modelLabel: String
    let hasChanges: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 13) {
            VStack(alignment: .leading, spacing: 4) {
                Text("Project profile")
                    .font(.system(size: 16, weight: .semibold, design: .rounded))
                    .foregroundStyle(.primary)

                Text(hasChanges ? "Review changes before saving." : "These defaults apply to new sessions.")
                    .font(.system(size: 12))
                    .foregroundStyle(.secondary)
            }

            Divider().opacity(0.55)

            ProjectSettingsSummaryRow(label: "Project key", value: draft.projectKey, placeholder: "Not set", monospaced: true)
            ProjectSettingsSummaryRow(label: "Display name", value: draft.displayName, placeholder: "Not set")
            ProjectSettingsSummaryRow(label: "Workdir", value: draft.defaultWorkdir, placeholder: "Not set", monospaced: true)
            ProjectSettingsSummaryRow(label: "Worktree root", value: draft.defaultWorktreeRoot, placeholder: "Not set", monospaced: true)
            ProjectSettingsSummaryRow(label: "Default role", value: roleLabel, placeholder: "No default role")
            ProjectSettingsSummaryRow(label: "Default model", value: modelLabel, placeholder: "Not selected")

            if !project.lastUpdatedText.isEmpty {
                Divider().opacity(0.35)
                ProjectSettingsSummaryRow(label: "Last updated", value: project.lastUpdatedText, placeholder: "Not saved")
            }
        }
        .padding(16)
        .background(Color(red: 0.058, green: 0.078, blue: 0.105))
        .overlay {
            RoundedRectangle(cornerRadius: 16, style: .continuous)
                .stroke(Color.primary.opacity(0.10), lineWidth: 1)
        }
        .clipShape(RoundedRectangle(cornerRadius: 16, style: .continuous))
    }
}

private struct ProjectSettingsSummaryRow: View {
    let label: String
    let value: String
    let placeholder: String
    var monospaced: Bool = false

    private var displayValue: String {
        value.isEmpty ? placeholder : value
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 3) {
            Text(label)
                .font(.system(size: 11, weight: .semibold))
                .foregroundStyle(.tertiary)
                .textCase(.uppercase)
                .tracking(0.7)

            Text(displayValue)
                .font(.system(size: 13, weight: value.isEmpty ? .regular : .medium, design: monospaced ? .monospaced : .default))
                .foregroundStyle(value.isEmpty ? .tertiary : .secondary)
                .lineLimit(3)
                .fixedSize(horizontal: false, vertical: true)
        }
    }
}

private struct ProjectSettingsBackground: View {
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

struct ProjectSettingsProfile {
    let projectKey: String
    let displayName: String
    let defaultWorkdir: String
    let defaultWorktreeRoot: String
    let defaultRoleId: String
    let defaultModel: String
    let lastUpdatedText: String
}

struct ProjectSettingsDraft: Equatable {
    let projectKey: String
    let displayName: String
    let defaultWorkdir: String
    let defaultWorktreeRoot: String
    let defaultRoleId: String
    let defaultModel: String
}

struct ProjectSettingsRoleOption: Identifiable {
    let id: String
    let label: String
}

struct ProjectSettingsModelOption: Identifiable {
    let id: String
    let label: String
}

struct ProjectSettingsRuntimeError {
    let message: String
    let recovery: String?
}

#Preview(traits: .landscapeLeft) {
    let project = ProjectSettingsProfile(
        projectKey: "codex-config",
        displayName: "Codex Config",
        defaultWorkdir: "/Users/robertsale/.codex",
        defaultWorktreeRoot: "/Users/robertsale/.codex/.worktrees/codex-config",
        defaultRoleId: "runtime-allow",
        defaultModel: "gpt-5.4",
        lastUpdatedText: "Today, 10:18 AM"
    )

    let roles = [
        ProjectSettingsRoleOption(id: "runtime-allow", label: "Runtime allow"),
        ProjectSettingsRoleOption(id: "requirements-reviewer", label: "Requirements reviewer"),
        ProjectSettingsRoleOption(id: "project-progenitor", label: "Project progenitor")
    ]

    let models = [
        ProjectSettingsModelOption(id: "gpt-5.4", label: "GPT-5.4"),
        ProjectSettingsModelOption(id: "gpt-5.4-mini", label: "GPT-5.4 mini"),
        ProjectSettingsModelOption(id: "local-runtime", label: "Local runtime default")
    ]

    ProjectSettingsSheet(
        project: project,
        roleOptions: roles,
        modelOptions: models,
        runtimeError: nil
    )
    .frame(width: 940, height: 740)
    .preferredColorScheme(.dark)
}

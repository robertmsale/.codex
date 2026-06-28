//
//  CreateProjectSheet.swift
//  robdex
//
//  Design surface placeholder generated from the Flutter Agent Runtime UI inventory.
//

import SwiftUI

#if os(macOS)
import AppKit
#endif

struct CreateProjectSheet: View {
    let roleOptions: [ProjectRoleOption]
    let modelOptions: [ProjectModelOption]
    let allowNoDefaultRole: Bool
    let runtimeError: ProjectRuntimeError?
    let onCancel: () -> Void
    let onCreate: (CreateProjectDraft) -> Void

    @State private var displayName = ""
    @State private var defaultWorkdir = ""
    @State private var defaultWorktreeRoot = ""
    @State private var selectedRoleId = ""
    @State private var selectedModelId = ""

    private var draft: CreateProjectDraft {
        CreateProjectDraft(
            projectKey: synthesizedProjectKey(from: displayName),
            displayName: displayName.trimmingCharacters(in: .whitespacesAndNewlines),
            defaultWorkdir: defaultWorkdir.trimmingCharacters(in: .whitespacesAndNewlines),
            defaultWorktreeRoot: defaultWorktreeRoot.trimmingCharacters(in: .whitespacesAndNewlines),
            defaultRoleId: selectedRoleId.isEmpty ? nil : selectedRoleId,
            defaultModel: selectedModelId.trimmingCharacters(in: .whitespacesAndNewlines)
        )
    }

    private var validationMessages: [String] {
        var messages: [String] = []
        if draft.displayName.isEmpty {
            messages.append("Display name is required.")
        } else if draft.projectKey.isEmpty {
            messages.append("Display name must produce a project key.")
        }
        if draft.defaultWorkdir.isEmpty {
            messages.append("Default workdir is required.")
        }
        if draft.defaultWorktreeRoot.isEmpty {
            messages.append("Default worktree root is required.")
        }
        if !allowNoDefaultRole && draft.defaultRoleId == nil {
            messages.append("Default role is required.")
        }
        if draft.defaultModel.isEmpty {
            messages.append("Default model is required.")
        }
        return messages
    }

    private var canCreate: Bool {
        validationMessages.isEmpty
    }

    init(
        roleOptions: [ProjectRoleOption],
        modelOptions: [ProjectModelOption],
        allowNoDefaultRole: Bool = false,
        runtimeError: ProjectRuntimeError? = nil,
        onCancel: @escaping () -> Void = {},
        onCreate: @escaping (CreateProjectDraft) -> Void = { _ in }
    ) {
        self.roleOptions = roleOptions
        self.modelOptions = modelOptions
        self.allowNoDefaultRole = allowNoDefaultRole
        self.runtimeError = runtimeError
        self.onCancel = onCancel
        self.onCreate = onCreate
        _selectedRoleId = State(initialValue: roleOptions.first?.id ?? "")
        _selectedModelId = State(initialValue: modelOptions.first?.id ?? "")
    }

    var body: some View {
        VStack(spacing: 0) {
            CreateProjectHeader(
                canCreate: canCreate,
                onCancel: onCancel,
                onCreate: submit
            )

            Divider().opacity(0.6)

            ScrollView {
                ViewThatFits(in: .horizontal) {
                    HStack(alignment: .top, spacing: 28) {
                        CreateProjectFormFields(
                            projectKey: draft.projectKey,
                            displayName: $displayName,
                            defaultWorkdir: $defaultWorkdir,
                            defaultWorktreeRoot: $defaultWorktreeRoot,
                            selectedRoleId: $selectedRoleId,
                            selectedModelId: $selectedModelId,
                            roleOptions: roleOptions,
                            modelOptions: modelOptions,
                            allowNoDefaultRole: allowNoDefaultRole,
                            validationMessages: validationMessages,
                            runtimeError: runtimeError
                        )
                        .frame(maxWidth: 560, alignment: .topLeading)

                        ProjectCreationSummary(
                            draft: draft,
                            roleLabel: roleLabel(for: draft.defaultRoleId),
                            modelLabel: modelLabel(for: draft.defaultModel)
                        )
                        .frame(width: 320, alignment: .topLeading)
                    }

                    VStack(alignment: .leading, spacing: 24) {
                        CreateProjectFormFields(
                            projectKey: draft.projectKey,
                            displayName: $displayName,
                            defaultWorkdir: $defaultWorkdir,
                            defaultWorktreeRoot: $defaultWorktreeRoot,
                            selectedRoleId: $selectedRoleId,
                            selectedModelId: $selectedModelId,
                            roleOptions: roleOptions,
                            modelOptions: modelOptions,
                            allowNoDefaultRole: allowNoDefaultRole,
                            validationMessages: validationMessages,
                            runtimeError: runtimeError
                        )

                        ProjectCreationSummary(
                            draft: draft,
                            roleLabel: roleLabel(for: draft.defaultRoleId),
                            modelLabel: modelLabel(for: draft.defaultModel)
                        )
                    }
                }
                .padding(24)
                .frame(maxWidth: 980)
                .frame(maxWidth: .infinity)
            }
            .background(CreateProjectBackground())
        }
        .frame(minWidth: 680, minHeight: 620)
    }

    private func submit() {
        guard canCreate else {
            return
        }
        onCreate(draft)
    }

    private func synthesizedProjectKey(from displayName: String) -> String {
        var result = ""
        var lastWasDash = false

        for scalar in displayName.lowercased().unicodeScalars {
            let isLowercaseLetter = scalar.value >= 97 && scalar.value <= 122
            let isDigit = scalar.value >= 48 && scalar.value <= 57

            if isLowercaseLetter || isDigit {
                result.unicodeScalars.append(scalar)
                lastWasDash = false
            } else if !lastWasDash && !result.isEmpty {
                result.append("-")
                lastWasDash = true
            }
        }

        while result.last == "-" {
            result.removeLast()
        }

        return result
    }

    private func roleLabel(for id: String?) -> String {
        guard let id, !id.isEmpty else {
            return allowNoDefaultRole ? "No default" : "Not selected"
        }
        return roleOptions.first(where: { $0.id == id })?.label ?? id
    }

    private func modelLabel(for id: String) -> String {
        guard !id.isEmpty else {
            return "Not selected"
        }
        return modelOptions.first(where: { $0.id == id })?.label ?? id
    }
}

private struct CreateProjectHeader: View {
    let canCreate: Bool
    let onCancel: () -> Void
    let onCreate: () -> Void

    var body: some View {
        HStack(alignment: .center, spacing: 16) {
            VStack(alignment: .leading, spacing: 4) {
                Text("Create Project")
                    .font(.system(size: 22, weight: .semibold, design: .rounded))
                    .foregroundStyle(.primary)

                Text("Create an Agent Runtime project profile used as the default context for new sessions.")
                    .font(.system(size: 13))
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }

            Spacer()

            Button("Cancel") {
                onCancel()
            }
            .buttonStyle(.bordered)

            Button("Create") {
                onCreate()
            }
            .buttonStyle(.borderedProminent)
            .disabled(!canCreate)
        }
        .padding(.horizontal, 24)
        .padding(.vertical, 18)
        .background(.regularMaterial)
    }
}

private struct CreateProjectFormFields: View {
    let projectKey: String
    @Binding var displayName: String
    @Binding var defaultWorkdir: String
    @Binding var defaultWorktreeRoot: String
    @Binding var selectedRoleId: String
    @Binding var selectedModelId: String

    let roleOptions: [ProjectRoleOption]
    let modelOptions: [ProjectModelOption]
    let allowNoDefaultRole: Bool
    let validationMessages: [String]
    let runtimeError: ProjectRuntimeError?

    var body: some View {
        VStack(alignment: .leading, spacing: 24) {
            CreateProjectSection(title: "Identity") {
                ProjectTextField(
                    title: "Display name",
                    text: $displayName,
                    prompt: "Codex Config",
                    help: "Human-readable label shown in project lists."
                )

                GeneratedProjectKeyField(projectKey: projectKey)
            }

            CreateProjectSection(title: "Workspace defaults") {
                PathProjectField(
                    title: "Default workdir",
                    text: $defaultWorkdir,
                    prompt: "/Users/you/Code/my-ios-app",
                    help: "Default working directory for new sessions in this project."
                )

                PathProjectField(
                    title: "Default worktree root",
                    text: $defaultWorktreeRoot,
                    prompt: "/Users/you/.codex/.worktrees/my-ios-app",
                    help: "Where session worktrees or workspace roots should live.",
                    copySourceTitle: "Use workdir",
                    onCopySource: {
                        defaultWorktreeRoot = defaultWorkdir
                    }
                )
            }

            CreateProjectSection(title: "Runtime defaults") {
                VStack(alignment: .leading, spacing: 7) {
                    Text("Default role")
                        .font(.system(size: 13, weight: .semibold))
                        .foregroundStyle(.primary)

                    Picker("Default role", selection: $selectedRoleId) {
                        if allowNoDefaultRole {
                            Text("No default").tag("")
                        }
                        ForEach(roleOptions) { role in
                            Text(role.label).tag(role.id)
                        }
                    }
                    .labelsHidden()
                    .pickerStyle(.menu)

                    Text("Role used when creating sessions unless another role is selected.")
                        .font(.system(size: 12))
                        .foregroundStyle(.secondary)
                }

                VStack(alignment: .leading, spacing: 7) {
                    Text("Default model")
                        .font(.system(size: 13, weight: .semibold))
                        .foregroundStyle(.primary)

                    Picker("Default model", selection: $selectedModelId) {
                        ForEach(modelOptions) { model in
                            Text(model.label).tag(model.id)
                        }
                    }
                    .labelsHidden()
                    .pickerStyle(.menu)

                    Text("Model used when creating sessions unless another model is selected.")
                        .font(.system(size: 12))
                        .foregroundStyle(.secondary)
                }
            }

            if !validationMessages.isEmpty {
                ValidationNotice(messages: validationMessages)
            }

            if let runtimeError {
                RuntimeErrorNotice(error: runtimeError)
            }
        }
    }
}

private struct CreateProjectSection<Content: View>: View {
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

private struct ProjectTextField: View {
    let title: String
    @Binding var text: String
    let prompt: String
    let help: String
    var monospaced: Bool = false

    var body: some View {
        VStack(alignment: .leading, spacing: 7) {
            Text(title)
                .font(.system(size: 13, weight: .semibold))
                .foregroundStyle(.primary)

            TextField(prompt, text: $text)
                .textFieldStyle(.plain)
                .font(.system(size: 14, design: monospaced ? .monospaced : .default))
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

private struct GeneratedProjectKeyField: View {
    let projectKey: String

    var body: some View {
        VStack(alignment: .leading, spacing: 7) {
            Text("Project key")
                .font(.system(size: 13, weight: .semibold))
                .foregroundStyle(.primary)

            Text(projectKey.isEmpty ? "Generated from display name" : projectKey)
                .font(.system(size: 14, design: .monospaced))
                .foregroundStyle(projectKey.isEmpty ? .tertiary : .secondary)
                .padding(.horizontal, 11)
                .padding(.vertical, 10)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(Color.primary.opacity(0.035))
                .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
                .overlay {
                    RoundedRectangle(cornerRadius: 10, style: .continuous)
                        .stroke(Color.primary.opacity(0.08), lineWidth: 1)
                }

            Text("Generated as kebab case from the display name.")
                .font(.system(size: 12))
                .foregroundStyle(.secondary)
        }
    }
}

private struct PathProjectField: View {
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

private struct ValidationNotice: View {
    let messages: [String]

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Before creating")
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

private struct RuntimeErrorNotice: View {
    let error: ProjectRuntimeError

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

private struct ProjectCreationSummary: View {
    let draft: CreateProjectDraft
    let roleLabel: String
    let modelLabel: String

    var body: some View {
        VStack(alignment: .leading, spacing: 13) {
            VStack(alignment: .leading, spacing: 4) {
                Text("Summary")
                    .font(.system(size: 16, weight: .semibold, design: .rounded))
                    .foregroundStyle(.primary)

                Text("Confirm the defaults before creating the project.")
                    .font(.system(size: 12))
                    .foregroundStyle(.secondary)
            }

            Divider().opacity(0.55)

            SummaryRow(label: "Key", value: draft.projectKey, placeholder: "Not set")
            SummaryRow(label: "Name", value: draft.displayName, placeholder: "Not set")
            SummaryRow(label: "Workdir", value: draft.defaultWorkdir, placeholder: "Not set")
            SummaryRow(label: "Worktree root", value: draft.defaultWorktreeRoot, placeholder: "Not set")
            SummaryRow(label: "Role", value: roleLabel, placeholder: "Not selected")
            SummaryRow(label: "Model", value: modelLabel, placeholder: "Not selected")
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

private struct SummaryRow: View {
    let label: String
    let value: String
    let placeholder: String

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
                .font(.system(size: 13, weight: value.isEmpty ? .regular : .medium))
                .foregroundStyle(value.isEmpty ? .tertiary : .secondary)
                .lineLimit(3)
                .fixedSize(horizontal: false, vertical: true)
        }
    }
}

private struct CreateProjectBackground: View {
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

struct CreateProjectDraft {
    let projectKey: String
    let displayName: String
    let defaultWorkdir: String
    let defaultWorktreeRoot: String
    let defaultRoleId: String?
    let defaultModel: String
}

struct ProjectRoleOption: Identifiable {
    let id: String
    let label: String
}

struct ProjectModelOption: Identifiable {
    let id: String
    let label: String
}

struct ProjectRuntimeError {
    let message: String
    let recovery: String?
}

#Preview(traits: .landscapeLeft) {
    let roles = [
        ProjectRoleOption(id: "runtime-allow", label: "Runtime allow"),
        ProjectRoleOption(id: "requirements-reviewer", label: "Requirements reviewer"),
        ProjectRoleOption(id: "project-progenitor", label: "Project progenitor")
    ]

    let models = [
        ProjectModelOption(id: "gpt-5.4", label: "GPT-5.4"),
        ProjectModelOption(id: "gpt-5.4-mini", label: "GPT-5.4 mini"),
        ProjectModelOption(id: "local-runtime", label: "Local runtime default")
    ]

    CreateProjectSheet(
        roleOptions: roles,
        modelOptions: models,
        runtimeError: nil
    )
    .frame(width: 920, height: 720)
    .preferredColorScheme(.dark)
}

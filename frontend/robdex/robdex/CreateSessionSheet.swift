//
//  CreateSessionSheet.swift
//  robdex
//
//  Design surface placeholder generated from the Flutter Agent Runtime UI inventory.
//

import SwiftUI

#if os(macOS)
import AppKit
#endif

struct CreateSessionSheet: View {
    let projects: [SessionProjectOption]
    let roleOptions: [SessionRoleOption]
    let modelOptions: [SessionModelOption]
    let runtimeError: SessionRuntimeError?
    let onCancel: () -> Void
    let onCreate: (CreateSessionDraft) -> Void

    @State private var selectedProjectId = ""
    @State private var selectedRoleId = ""
    @State private var selectedModelId = ""
    @State private var workdir = ""
    @State private var worktreeRootOverride = ""
    @State private var worktreeRootManuallyEdited = false
    @State private var title = ""

    private var synthesizedName: String {
        slug(from: title)
    }

    private var synthesizedWorktreeRoot: String {
        guard !synthesizedName.isEmpty else {
            return selectedProject?.defaultWorktreeRoot ?? ""
        }
        return appendingSessionName(synthesizedName, to: selectedProject?.defaultWorktreeRoot ?? "")
    }

    private var effectiveWorktreeRoot: String {
        worktreeRootManuallyEdited ? worktreeRootOverride : synthesizedWorktreeRoot
    }

    private var draft: CreateSessionDraft {
        CreateSessionDraft(
            projectId: selectedProjectId,
            roleId: selectedRoleId,
            modelId: selectedModelId,
            workdir: workdir.trimmingCharacters(in: .whitespacesAndNewlines),
            worktreeRoot: effectiveWorktreeRoot.trimmingCharacters(in: .whitespacesAndNewlines),
            title: title.trimmingCharacters(in: .whitespacesAndNewlines),
            name: synthesizedName
        )
    }

    private var selectedProject: SessionProjectOption? {
        projects.first(where: { $0.id == selectedProjectId })
    }

    private var validationMessages: [String] {
        var messages: [String] = []
        if draft.title.isEmpty {
            messages.append("Session title is required.")
        } else if draft.name.isEmpty {
            messages.append("Session title must produce a stable name.")
        }
        if draft.projectId.isEmpty {
            messages.append("Project is required.")
        }
        if draft.roleId.isEmpty {
            messages.append("Role is required.")
        }
        if draft.modelId.isEmpty {
            messages.append("Model is required.")
        }
        if draft.workdir.isEmpty {
            messages.append("Workdir is required.")
        }
        if draft.worktreeRoot.isEmpty {
            messages.append("Worktree root is required.")
        }
        return messages
    }

    private var canCreate: Bool {
        validationMessages.isEmpty
    }

    init(
        projects: [SessionProjectOption],
        roleOptions: [SessionRoleOption],
        modelOptions: [SessionModelOption],
        runtimeError: SessionRuntimeError? = nil,
        onCancel: @escaping () -> Void = {},
        onCreate: @escaping (CreateSessionDraft) -> Void = { _ in }
    ) {
        self.projects = projects
        self.roleOptions = roleOptions
        self.modelOptions = modelOptions
        self.runtimeError = runtimeError
        self.onCancel = onCancel
        self.onCreate = onCreate
        _selectedProjectId = State(initialValue: projects.first?.id ?? "")
        _selectedRoleId = State(initialValue: roleOptions.first?.id ?? "")
        _selectedModelId = State(initialValue: modelOptions.first?.id ?? "")
        _workdir = State(initialValue: projects.first?.defaultWorkdir ?? "")
    }

    var body: some View {
        VStack(spacing: 0) {
            CreateSessionHeader(
                canCreate: canCreate,
                onCancel: onCancel,
                onCreate: submit
            )

            Divider().opacity(0.6)

            ScrollView {
                ViewThatFits(in: .horizontal) {
                    HStack(alignment: .top, spacing: 28) {
                        CreateSessionFormFields(
                            selectedProjectId: $selectedProjectId,
                            selectedRoleId: $selectedRoleId,
                            selectedModelId: $selectedModelId,
                            workdir: $workdir,
                            worktreeRootOverride: $worktreeRootOverride,
                            worktreeRootManuallyEdited: $worktreeRootManuallyEdited,
                            title: $title,
                            sessionName: synthesizedName,
                            synthesizedWorktreeRoot: synthesizedWorktreeRoot,
                            projects: projects,
                            roleOptions: roleOptions,
                            modelOptions: modelOptions,
                            validationMessages: validationMessages,
                            runtimeError: runtimeError,
                            onUseProjectDefaults: useProjectDefaults,
                            onCopyWorkdirToWorktreeRoot: {
                                worktreeRootOverride = workdir
                                worktreeRootManuallyEdited = true
                            }
                        )
                        .frame(maxWidth: 580, alignment: .topLeading)

                        SessionCreationSummary(
                            draft: draft,
                            projectLabel: projectLabel(for: draft.projectId),
                            roleLabel: roleLabel(for: draft.roleId),
                            modelLabel: modelLabel(for: draft.modelId)
                        )
                        .frame(width: 330, alignment: .topLeading)
                    }

                    VStack(alignment: .leading, spacing: 24) {
                        CreateSessionFormFields(
                            selectedProjectId: $selectedProjectId,
                            selectedRoleId: $selectedRoleId,
                            selectedModelId: $selectedModelId,
                            workdir: $workdir,
                            worktreeRootOverride: $worktreeRootOverride,
                            worktreeRootManuallyEdited: $worktreeRootManuallyEdited,
                            title: $title,
                            sessionName: synthesizedName,
                            synthesizedWorktreeRoot: synthesizedWorktreeRoot,
                            projects: projects,
                            roleOptions: roleOptions,
                            modelOptions: modelOptions,
                            validationMessages: validationMessages,
                            runtimeError: runtimeError,
                            onUseProjectDefaults: useProjectDefaults,
                            onCopyWorkdirToWorktreeRoot: {
                                worktreeRootOverride = workdir
                                worktreeRootManuallyEdited = true
                            }
                        )

                        SessionCreationSummary(
                            draft: draft,
                            projectLabel: projectLabel(for: draft.projectId),
                            roleLabel: roleLabel(for: draft.roleId),
                            modelLabel: modelLabel(for: draft.modelId)
                        )
                    }
                }
                .padding(24)
                .frame(maxWidth: 1020)
                .frame(maxWidth: .infinity)
            }
            .background(CreateSessionBackground())
        }
        .frame(minWidth: 700, minHeight: 660)
        .onChange(of: selectedProjectId) { _, _ in
            useProjectDefaults()
        }
    }

    private func submit() {
        guard canCreate else {
            return
        }
        onCreate(draft)
    }

    private func useProjectDefaults() {
        guard let selectedProject else {
            return
        }
        if !selectedProject.defaultRoleId.isEmpty {
            selectedRoleId = selectedProject.defaultRoleId
        }
        if !selectedProject.defaultModelId.isEmpty {
            selectedModelId = selectedProject.defaultModelId
        }
        workdir = selectedProject.defaultWorkdir
        worktreeRootManuallyEdited = false
        worktreeRootOverride = ""
    }

    private func projectLabel(for id: String) -> String {
        projects.first(where: { $0.id == id })?.label ?? "Not selected"
    }

    private func roleLabel(for id: String) -> String {
        roleOptions.first(where: { $0.id == id })?.label ?? "Not selected"
    }

    private func modelLabel(for id: String) -> String {
        modelOptions.first(where: { $0.id == id })?.label ?? "Not selected"
    }

    private func slug(from value: String) -> String {
        var result = ""
        var lastWasDash = false

        for scalar in value.lowercased().unicodeScalars {
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

    private func appendingSessionName(_ name: String, to root: String) -> String {
        let trimmedRoot = root.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedRoot.isEmpty else {
            return name
        }

        var base = trimmedRoot
        while base.last == "/" {
            base.removeLast()
        }

        guard !base.isEmpty else {
            return name
        }

        return "\(base)/\(name)"
    }
}

private struct CreateSessionHeader: View {
    let canCreate: Bool
    let onCancel: () -> Void
    let onCreate: () -> Void

    var body: some View {
        HStack(alignment: .center, spacing: 16) {
            VStack(alignment: .leading, spacing: 4) {
                Text("Create Session")
                    .font(.system(size: 22, weight: .semibold, design: .rounded))
                    .foregroundStyle(.primary)

                Text("Start an Agent Runtime session with project, role, model, and workspace defaults already set.")
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

private struct CreateSessionFormFields: View {
    @Binding var selectedProjectId: String
    @Binding var selectedRoleId: String
    @Binding var selectedModelId: String
    @Binding var workdir: String
    @Binding var worktreeRootOverride: String
    @Binding var worktreeRootManuallyEdited: Bool
    @Binding var title: String

    let sessionName: String
    let synthesizedWorktreeRoot: String
    let projects: [SessionProjectOption]
    let roleOptions: [SessionRoleOption]
    let modelOptions: [SessionModelOption]
    let validationMessages: [String]
    let runtimeError: SessionRuntimeError?
    let onUseProjectDefaults: () -> Void
    let onCopyWorkdirToWorktreeRoot: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 24) {
            CreateSessionSection(title: "Session") {
                SessionTextField(
                    title: "Title",
                    text: $title,
                    prompt: "Release notes cleanup",
                    help: "Human-readable session title."
                )

                GeneratedSessionNameField(name: sessionName)
            }

            CreateSessionSection(title: "Runtime context") {
                VStack(alignment: .leading, spacing: 7) {
                    Text("Project")
                        .font(.system(size: 13, weight: .semibold))
                        .foregroundStyle(.primary)

                    Picker("Project", selection: $selectedProjectId) {
                        ForEach(projects) { project in
                            Text(project.label).tag(project.id)
                        }
                    }
                    .labelsHidden()
                    .pickerStyle(.menu)

                    Text("Project supplies the starting defaults for new sessions.")
                        .font(.system(size: 12))
                        .foregroundStyle(.secondary)

                    Button("Use project defaults") {
                        onUseProjectDefaults()
                    }
                    .buttonStyle(.bordered)
                    .controlSize(.small)
                }

                VStack(alignment: .leading, spacing: 7) {
                    Text("Role")
                        .font(.system(size: 13, weight: .semibold))
                        .foregroundStyle(.primary)

                    Picker("Role", selection: $selectedRoleId) {
                        ForEach(roleOptions) { role in
                            Text(role.label).tag(role.id)
                        }
                    }
                    .labelsHidden()
                    .pickerStyle(.menu)

                    Text("Role controls how the runtime behaves in this session.")
                        .font(.system(size: 12))
                        .foregroundStyle(.secondary)
                }

                VStack(alignment: .leading, spacing: 7) {
                    Text("Model")
                        .font(.system(size: 13, weight: .semibold))
                        .foregroundStyle(.primary)

                    Picker("Model", selection: $selectedModelId) {
                        ForEach(modelOptions) { model in
                            Text(model.label).tag(model.id)
                        }
                    }
                    .labelsHidden()
                    .pickerStyle(.menu)

                    Text("Model used for the first turn unless changed later.")
                        .font(.system(size: 12))
                        .foregroundStyle(.secondary)
                }
            }

            CreateSessionSection(title: "Workspace") {
                SessionPathField(
                    title: "Workdir",
                    text: $workdir,
                    prompt: "/Users/you/Code/my-ios-app",
                    help: "Working directory where the session starts."
                )

                SessionPathField(
                    title: "Worktree root",
                    text: Binding(
                        get: { worktreeRootManuallyEdited ? worktreeRootOverride : synthesizedWorktreeRoot },
                        set: { newValue in
                            worktreeRootOverride = newValue
                            worktreeRootManuallyEdited = true
                        }
                    ),
                    prompt: "/Users/you/.codex/.worktrees/my-ios-app",
                    help: "Generated from the project worktree root and session name unless edited.",
                    copySourceTitle: "Use workdir",
                    onCopySource: onCopyWorkdirToWorktreeRoot
                )
            }

            if !validationMessages.isEmpty {
                SessionValidationNotice(messages: validationMessages)
            }

            if let runtimeError {
                SessionRuntimeErrorNotice(error: runtimeError)
            }
        }
    }
}

private struct CreateSessionSection<Content: View>: View {
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

private struct SessionTextField: View {
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

private struct GeneratedSessionNameField: View {
    let name: String

    var body: some View {
        VStack(alignment: .leading, spacing: 7) {
            Text("Name")
                .font(.system(size: 13, weight: .semibold))
                .foregroundStyle(.primary)

            Text(name.isEmpty ? "Generated from title" : name)
                .font(.system(size: 14, design: .monospaced))
                .foregroundStyle(name.isEmpty ? .tertiary : .secondary)
                .padding(.horizontal, 11)
                .padding(.vertical, 10)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(Color.primary.opacity(0.035))
                .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
                .overlay {
                    RoundedRectangle(cornerRadius: 10, style: .continuous)
                        .stroke(Color.primary.opacity(0.08), lineWidth: 1)
                }

            Text("Stable session name generated as kebab case from the title.")
                .font(.system(size: 12))
                .foregroundStyle(.secondary)
        }
    }
}

private struct SessionPathField: View {
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

private struct SessionValidationNotice: View {
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

private struct SessionRuntimeErrorNotice: View {
    let error: SessionRuntimeError

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

private struct SessionCreationSummary: View {
    let draft: CreateSessionDraft
    let projectLabel: String
    let roleLabel: String
    let modelLabel: String

    var body: some View {
        VStack(alignment: .leading, spacing: 13) {
            VStack(alignment: .leading, spacing: 4) {
                Text("Summary")
                    .font(.system(size: 16, weight: .semibold, design: .rounded))
                    .foregroundStyle(.primary)

                Text("Confirm the starting context before creating the session.")
                    .font(.system(size: 12))
                    .foregroundStyle(.secondary)
            }

            Divider().opacity(0.55)

            SessionSummaryRow(label: "Title", value: draft.title, placeholder: "Not set")
            SessionSummaryRow(label: "Name", value: draft.name, placeholder: "Generated from title")
            SessionSummaryRow(label: "Project", value: projectLabel, placeholder: "Not selected")
            SessionSummaryRow(label: "Role", value: roleLabel, placeholder: "Not selected")
            SessionSummaryRow(label: "Model", value: modelLabel, placeholder: "Not selected")
            SessionSummaryRow(label: "Workdir", value: draft.workdir, placeholder: "Not set")
            SessionSummaryRow(label: "Worktree root", value: draft.worktreeRoot, placeholder: "Not set")
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

private struct SessionSummaryRow: View {
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

private struct CreateSessionBackground: View {
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

struct CreateSessionDraft {
    let projectId: String
    let roleId: String
    let modelId: String
    let workdir: String
    let worktreeRoot: String
    let title: String
    let name: String
}

struct SessionProjectOption: Identifiable {
    let id: String
    let label: String
    let defaultWorkdir: String
    let defaultWorktreeRoot: String
    let defaultRoleId: String
    let defaultModelId: String
}

struct SessionRoleOption: Identifiable {
    let id: String
    let label: String
}

struct SessionModelOption: Identifiable {
    let id: String
    let label: String
}

struct SessionRuntimeError {
    let message: String
    let recovery: String?
}

#Preview(traits: .landscapeLeft) {
    let projects = [
        SessionProjectOption(
            id: "codex-config",
            label: "Codex Config",
            defaultWorkdir: "/Users/robertsale/.codex",
            defaultWorktreeRoot: "/Users/robertsale/.codex/.worktrees/codex-config",
            defaultRoleId: "runtime-allow",
            defaultModelId: "gpt-5.4"
        ),
        SessionProjectOption(
            id: "my-ios-app",
            label: "My iOS App",
            defaultWorkdir: "/Users/robertsale/Code/my-ios-app",
            defaultWorktreeRoot: "/Users/robertsale/.codex/.worktrees/my-ios-app",
            defaultRoleId: "project-progenitor",
            defaultModelId: "gpt-5.4-mini"
        ),
        SessionProjectOption(
            id: "__unassigned__",
            label: "Unassigned",
            defaultWorkdir: "/Users/robertsale/.codex",
            defaultWorktreeRoot: "/Users/robertsale/.codex/.worktrees/unassigned",
            defaultRoleId: "runtime-allow",
            defaultModelId: "gpt-5.4"
        )
    ]

    let roles = [
        SessionRoleOption(id: "runtime-allow", label: "Runtime allow"),
        SessionRoleOption(id: "requirements-reviewer", label: "Requirements reviewer"),
        SessionRoleOption(id: "project-progenitor", label: "Project progenitor")
    ]

    let models = [
        SessionModelOption(id: "gpt-5.4", label: "GPT-5.4"),
        SessionModelOption(id: "gpt-5.4-mini", label: "GPT-5.4 mini"),
        SessionModelOption(id: "local-runtime", label: "Local runtime default")
    ]

    CreateSessionSheet(
        projects: projects,
        roleOptions: roles,
        modelOptions: models,
        runtimeError: nil
    )
    .frame(width: 940, height: 740)
    .preferredColorScheme(.dark)
}
